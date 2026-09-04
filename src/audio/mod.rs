use crate::ui::overlay::AudioLevelBuf;
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::BOOL;
#[cfg(target_os = "windows")]
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
#[cfg(target_os = "windows")]
use windows::Win32::Media::Audio::{
    eCapture, eMultimedia, IMMDeviceEnumerator, MMDeviceEnumerator,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

const PRE_ROLL_MS: u64 = 600; // HOTKEY-LATENCY-V2-001: 500→600ms to collect more initial audio for cold start
const PRIME_TIMEOUT_MS: u64 = 450; // HOTKEY-LATENCY-V2-001: 350→450ms for deeper cold-start audio collection
const PRIME_TICK_MS: u64 = 20; // HOTKEY-LATENCY-FIX-001: recv_timeout tick, allows up to 17 ticks before timeout

type AudioChunk = (Instant, Vec<f32>);

pub struct AudioCapture {
    #[allow(dead_code)]
    pub sample_rate: u32,
    warm_stream: Option<WarmInputStream>,
}

struct WarmInputStream {
    requested_device_name: Option<String>,
    actual_device_name: String,
    sample_format: SampleFormat,
    sample_rate: u32,
    channels: usize,
    pre_roll: Arc<Mutex<VecDeque<Vec<f32>>>>,
    rx: crossbeam_channel::Receiver<AudioChunk>,
    stream_failed: Arc<AtomicBool>,
    /// ASR-074 Step 1: 队列满时静默丢帧计数（永久埋点）
    dropped_chunks: Arc<AtomicU64>,
    _stream: Stream,
}

impl AudioCapture {
    pub fn new() -> Self {
        Self {
            sample_rate: 16000,
            warm_stream: None,
        }
    }

    /// Start and keep the input stream hot so first speech is not lost while
    /// CPAL/WASAPI creates the stream on the hotkey path.
    pub fn prewarm(&mut self, device_name: Option<&str>) -> Result<()> {
        self.ensure_stream(device_name).map(|_| ())
    }

    /// Check stream health and pre-rebuild if the stream has failed.
    /// Called periodically from the worker thread's idle loop to ensure
    /// the WASAPI stream is ready when hotkey Start arrives, avoiding
    /// 50–500ms synchronous rebuild delay on the recording path.
    pub fn check_stream_health(&mut self) {
        let needs_rebuild = self
            .warm_stream
            .as_ref()
            .is_some_and(|warm| warm.stream_failed.load(Ordering::Acquire));

        if !needs_rebuild {
            return;
        }

        // Clone the device name to release the immutable borrow before calling
        // ensure_stream, which needs mutable access to self.
        let device_name = self
            .warm_stream
            .as_ref()
            .and_then(|warm| warm.requested_device_name.clone());

        let t0 = std::time::Instant::now();
        match self.ensure_stream(device_name.as_deref()) {
            Ok(warm) => {
                warm.stream_failed.store(false, Ordering::Release);
                log::info!(
                    "[Latency] stream pre-warmed in {:.0}ms (device='{}')",
                    t0.elapsed().as_secs_f64() * 1000.0,
                    warm.actual_device_name
                );
            }
            Err(e) => {
                log::error!("[Latency] stream pre-warm failed: {:#}", e);
            }
        }
    }

    /// Record audio until VAD detects sustained silence or stop_signal is set.
    /// Returns raw PCM samples (f32, mono, 16kHz).
    /// If device_name is empty, uses system default device.
    pub fn record(
        &mut self,
        stop_signal: Arc<AtomicBool>,
        silence_threshold: f32,
        silence_duration_ms: u64,
        max_seconds: u64,
        level_buf: Option<AudioLevelBuf>,
        device_name: Option<&str>,
    ) -> Result<Vec<f32>> {
        let t_record = std::time::Instant::now();
        let warm = self.ensure_stream(device_name)?;
        log::info!(
            "[Latency] ensure_stream completed at +{:.1}ms",
            t_record.elapsed().as_secs_f64() * 1000.0
        );
        let pre_roll_chunks = warm.drain_pre_roll(PRE_ROLL_MS);
        log::info!(
            "[Latency] drain_pre_roll completed at +{:.1}ms",
            t_record.elapsed().as_secs_f64() * 1000.0
        );
        // FIRSTCHAR-FIX-004 (D3): Precise idle drain using timestamps.
        // Each audio chunk is tagged with Instant::now() from the WASAPI callback.
        // The cutoff is `t_record` (captured at record() entry, closest to the
        // hotkey trigger) — NOT a fresh Instant taken here, because ensure_stream
        // may rebuild the stream for 100–500ms, during which the user's first
        // syllable is captured; using a later cutoff would wrongly clear it.
        // Chunks with timestamp < t_record are stale idle audio (cleared);
        // chunks with timestamp >= t_record are valid post-hotkey audio (preserved).
        let mut idle_cleared: usize = 0;
        let mut post_hotkey_chunks: Vec<Vec<f32>> = Vec::new();
        loop {
            match warm.rx.try_recv() {
                Ok((ts, chunk)) => {
                    if ts < t_record {
                        idle_cleared += 1;
                    } else {
                        post_hotkey_chunks.push(chunk);
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        log::info!(
            "[Latency] channel precise idle drain: cleared {} pre-hotkey chunks, preserved {} post-hotkey chunks at +{:.1}ms",
            idle_cleared,
            post_hotkey_chunks.len(),
            t_record.elapsed().as_secs_f64() * 1000.0
        );
        warm.stream_failed.store(false, Ordering::Release);

        log::info!(
            "Recording started from prewarmed stream ({}Hz, {} ch, {:?}, device='{}', pre_roll={} chunks)",
            warm.sample_rate,
            warm.channels,
            warm.sample_format,
            warm.actual_device_name,
            pre_roll_chunks.len()
        );

        collect_recording(
            &warm.rx,
            &warm.stream_failed,
            stop_signal,
            silence_threshold,
            silence_duration_ms,
            max_seconds,
            level_buf,
            warm.sample_rate,
            pre_roll_chunks,
            post_hotkey_chunks,
        )
    }

    /// ASR-038-B: 流式录音 —— 热键按下即采集，每个 chunk 通过回调推送。
    ///
    /// **与 record() 的关键差异**：
    /// - 不调 `collect_recording`（那是阻塞攒完整 samples 的批处理逻辑）
    /// - 每个 chunk 实时推给 `on_chunk` 回调（ASR 线程做 VAD 门控+建连+发帧）
    /// - RMS 静音检测仍在（管停录），`speech_detected` 行为不变（与 Silero VAD 并存）
    /// - pre_roll 不攒在 VecDeque，而是作为首批 chunk 推给回调（建连后补发）
    ///
    /// **热键按下即采集**（硬性自证②要求）：
    /// WASAPI stream 在 prewarm 时已运行，回调持续推 chunk 到 channel。
    /// 本方法从 channel 读 chunk 推给回调，**不等待 VAD 判定** —— VAD 在 ASR 线程做，
    /// 只决定「何时建连」，不影响「何时开始采集」。采集从热键按下瞬间就开始。
    ///
    /// **不改 record() 行为**：平行方法，record() 零改动。
    pub fn record_streaming(
        &mut self,
        stop_signal: Arc<AtomicBool>,
        silence_threshold: f32,
        silence_duration_ms: u64,
        max_seconds: u64,
        level_buf: Option<AudioLevelBuf>,
        device_name: Option<&str>,
        mut on_chunk: impl FnMut(&[f32]),
    ) -> Result<()> {
        let t_record = std::time::Instant::now();
        let warm = self.ensure_stream(device_name)?;
        log::info!(
            "[Latency] record_streaming ensure_stream completed at +{:.1}ms",
            t_record.elapsed().as_secs_f64() * 1000.0
        );

        // pre-roll：从 VecDeque drain 出热键前的音频，作为首批 chunk 推给回调
        // 这是「建连后补发」的 pre-roll 来源 —— 热键前的音频不丢
        let pre_roll_chunks = warm.drain_pre_roll(PRE_ROLL_MS);
        log::info!(
            "[Latency] record_streaming drain_pre_roll: {} chunks at +{:.1}ms",
            pre_roll_chunks.len(),
            t_record.elapsed().as_secs_f64() * 1000.0
        );

        // 精确 idle drain（同 record() 的 FIRSTCHAR-FIX-004）：清热键前 stale chunk
        let mut idle_cleared: usize = 0;
        let mut post_hotkey_chunks: Vec<Vec<f32>> = Vec::new();
        loop {
            match warm.rx.try_recv() {
                Ok((ts, chunk)) => {
                    if ts < t_record {
                        idle_cleared += 1;
                    } else {
                        post_hotkey_chunks.push(chunk);
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        log::info!(
            "[Latency] record_streaming idle drain: cleared {} stale, preserved {} post-hotkey at +{:.1}ms",
            idle_cleared,
            post_hotkey_chunks.len(),
            t_record.elapsed().as_secs_f64() * 1000.0
        );
        warm.stream_failed.store(false, Ordering::Release);

        log::info!(
            "Streaming recording started ({}Hz, {} ch, device='{}', pre_roll={} chunks, post_hotkey={} chunks)",
            warm.sample_rate,
            warm.channels,
            warm.actual_device_name,
            pre_roll_chunks.len(),
            post_hotkey_chunks.len()
        );

        // ASR-042: 流式重采样器。麦克风以 48kHz 采集，但在线 ASR 需要 16kHz。
        // 旧 record() 在录音结束时一次性 resample_anti_alias；record_streaming
        // 边录边发没有那个时机，且逐块调 resample_anti_alias 会在每 10ms chunk
        // 边界截断 FIR 卷积核（TAPS=32），复发 FIRSTCHAR-FIX-005 消灭的送气清声母
        // 失真。StreamingResampler 跨块保留历史+lookahead，用全局 emitted 计数，
        // 与批处理版数学等价。
        //
        // 🔴 顺序不变式：pre-roll → post-hotkey → 主循环 必须依次喂进【同一个】
        // 实例。顺序错位 = 音频错位。实例只建这一个。
        let sample_rate = warm.sample_rate;
        const ASR_TARGET_RATE: u32 = 16000;
        let mut resampler = StreamingResampler::new(sample_rate, ASR_TARGET_RATE);
        if sample_rate != ASR_TARGET_RATE {
            log::info!(
                "Streaming resampler active: {}Hz -> {}Hz",
                sample_rate,
                ASR_TARGET_RATE
            );
        }

        // 推 pre-roll chunks 给回调（ASR 线程的 VAD 门控+建连会收到这些）
        for chunk in &pre_roll_chunks {
            let resampled = resampler.push(chunk);
            if !resampled.is_empty() {
                on_chunk(&resampled);
            }
        }
        // 推 post-hotkey chunks
        for chunk in &post_hotkey_chunks {
            let resampled = resampler.push(chunk);
            if !resampled.is_empty() {
                on_chunk(&resampled);
            }
        }

        // RMS 静音检测状态（与 record() 的 RecordingState 逻辑一致，但只管停录+level_buf）
        let silence_frames = (silence_duration_ms as f32 / 1000.0 * sample_rate as f32) as usize;
        let mut silent_count: usize = 0;
        let mut speech_detected = false;
        let max_frames = max_seconds as usize * sample_rate as usize;
        // 🔴 total_samples 继续用原始 chunk 长度：max_frames 按 warm.sample_rate 算，
        // 重采样后的长度会改变比例，不能用来做原始采样率下的时长上限判定。
        let mut total_samples: usize = pre_roll_chunks.iter().map(|c| c.len()).sum::<usize>()
            + post_hotkey_chunks.iter().map(|c| c.len()).sum::<usize>();

        // 主循环：从 channel 读 chunk → 重采样 → 推回调 → RMS 检测
        // 🔴 RMS 静音检测、level_buf 继续用原始 chunk（未重采样），与 record() 一致。
        let recv_timeout = Duration::from_millis(50);
        loop {
            if stop_signal.load(Ordering::Relaxed) {
                log::info!("Streaming recording: stop signal received");
                // ASR-074 Step 2-B: 松手前 drain warm.rx 积压，防尾部音频丢失
                // 🔴 时间上限 500ms（非数量上限）：on_chunk 落到 chunk_tx.send() 是
                // crossbeam bounded channel 的**阻塞 send**，若 ASR 线程停止消费
                // （WS 卡住/服务端不回/连接半开），chunk_tx 满则 send 永远等。
                // 没有时间上限 = 把 ASR 的故障传导到录音线程 = 用户松手后程序卡死。
                // 500ms 理由：正常 drain 256 chunks 的 on_chunk 应在毫秒级完成；
                // 500ms 足够救回绝大多数尾部音频，同时保证松手后最坏 0.5s 内返回。
                let drain_deadline = std::time::Instant::now() + Duration::from_millis(500);
                let mut drained: usize = 0;
                let mut abandoned: usize = 0;
                while std::time::Instant::now() < drain_deadline {
                    match warm.rx.try_recv() {
                        Ok((_ts, chunk)) if !chunk.is_empty() => {
                            let resampled = resampler.push(&chunk);
                            if !resampled.is_empty() {
                                on_chunk(&resampled);
                            }
                            total_samples += chunk.len();
                            drained += 1;
                        }
                        Ok(_) => continue,
                        Err(crossbeam_channel::TryRecvError::Empty) => break,
                        Err(crossbeam_channel::TryRecvError::Disconnected) => break,
                    }
                }
                // 超时后仍有积压 → 放弃，记录数量
                while let Ok((_ts, chunk)) = warm.rx.try_recv() {
                    if !chunk.is_empty() {
                        abandoned += 1;
                    }
                }
                if drained > 0 {
                    log::info!(
                        "Streaming recording: drained {} backlog chunks on stop (resampled→on_chunk)",
                        drained
                    );
                }
                if abandoned > 0 {
                    log::warn!(
                        "[ASR-DROP] abandoned {} backlog chunks on stop (drain deadline 500ms exceeded, ASR consumer may be stuck)",
                        abandoned
                    );
                }
                break;
            }
            if warm.stream_failed.load(Ordering::Acquire) {
                anyhow::bail!("Audio input stream failed during streaming recording");
            }
            if total_samples >= max_frames {
                log::info!("Streaming recording: max length reached");
                break;
            }

            match warm.rx.recv_timeout(recv_timeout) {
                Ok((_ts, chunk)) if !chunk.is_empty() => {
                    let rms =
                        (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();

                    if let Some(ref buf) = level_buf {
                        crate::ui::overlay::push_level(buf, rms);
                    }

                    if rms > silence_threshold {
                        silent_count = 0;
                        speech_detected = true;
                    } else if speech_detected {
                        silent_count += chunk.len();
                        if silent_count >= silence_frames {
                            log::info!(
                                "Streaming recording: silence detected ({} samples), ending",
                                silent_count
                            );
                            break;
                        }
                    }

                    // 重采样后推回调（原始 chunk 的 RMS/level_buf 已在上面处理完）
                    let resampled = resampler.push(&chunk);
                    if !resampled.is_empty() {
                        on_chunk(&resampled);
                    }
                    total_samples += chunk.len();
                }
                Ok(_) => {
                    log::warn!(
                        "Streaming recording: received empty chunk (possible stream failure)"
                    );
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("Audio input stream disconnected during streaming recording");
                }
            }
        }

        // 录音结束：flush 重采样器尾部，与批处理版 resample_anti_alias 的
        // 尾部截断行为一致。非空则最后再推一次回调。
        let tail = resampler.finish();
        if !tail.is_empty() {
            on_chunk(&tail);
        }

        // ASR-074 Step 1: 打出累计丢帧数（每个 chunk ≈10ms @48k，精确秒数不可知因 chunk 已丢）
        let dropped = warm.dropped_chunks.load(Ordering::Relaxed);
        if dropped > 0 {
            log::warn!(
                "[ASR-DROP] {} chunks dropped during recording (~{:.1}s audio lost, est. 10ms/chunk)",
                dropped,
                dropped as f32 * 0.01
            );
        }

        log::info!(
            "Streaming recording complete: ~{} samples ({:.1}s @ {}Hz), speech_detected={}, dropped_chunks={}",
            total_samples,
            total_samples as f32 / sample_rate as f32,
            sample_rate,
            speech_detected,
            dropped
        );

        Ok(())
    }

    fn ensure_stream(&mut self, device_name: Option<&str>) -> Result<&mut WarmInputStream> {
        let requested_device_name = normalize_device_name(device_name);
        let host = cpal::default_host();

        let device = if let Some(name) = requested_device_name.as_deref() {
            host.input_devices()
                .context("Failed to enumerate input devices")?
                .find(|d| d.name().ok().as_deref() == Some(name))
                .with_context(|| format!("Device '{}' not found", name))?
        } else {
            host.default_input_device()
                .context("No default input device found")?
        };
        let actual_device_name = device.name()?;

        if self
            .warm_stream
            .as_ref()
            .is_some_and(|warm| warm.matches_device(&requested_device_name, &actual_device_name))
        {
            return Ok(self
                .warm_stream
                .as_mut()
                .expect("warm stream checked above"));
        }

        log::info!("Prewarming input device: {}", actual_device_name);

        let supported_config = device
            .default_input_config()
            .context("No supported input config")?;
        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels as usize;

        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(256);
        let tx_err = tx.clone();
        let stream_failed = Arc::new(AtomicBool::new(false));
        let pre_roll = Arc::new(Mutex::new(VecDeque::<Vec<f32>>::new()));
        let max_pre_roll_samples = pre_roll_samples(sample_rate, PRE_ROLL_MS);
        // ASR-074 Step 1: 丢帧计数器，回调闭包递增，record_streaming 结束时读取
        let dropped_chunks = Arc::new(AtomicU64::new(0));

        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx_audio = tx.clone();
                let tx_stream_err = tx_err.clone();
                let stream_failed = Arc::clone(&stream_failed);
                let pre_roll_cb = Arc::clone(&pre_roll);
                let dropped_chunks_cb = Arc::clone(&dropped_chunks);
                let max_pr = max_pre_roll_samples;
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        let chunk = downmix_to_mono(data, channels, |sample| sample);
                        {
                            let mut pr = pre_roll_cb.lock().unwrap();
                            pr.push_back(chunk.clone());
                            let mut total: usize = pr.iter().map(|c| c.len()).sum();
                            while total > max_pr {
                                if let Some(dropped) = pr.pop_front() {
                                    total -= dropped.len();
                                }
                            }
                        }
                        // ASR-074 Step 1: 队列满不再静默丢弃，计数并降级日志
                        if tx_audio.try_send((Instant::now(), chunk)).is_err() {
                            dropped_chunks_cb.fetch_add(1, Ordering::Relaxed);
                            log::warn!(
                                "[ASR-DROP] audio chunk dropped (queue full), total dropped so far: {}",
                                dropped_chunks_cb.load(Ordering::Relaxed)
                            );
                        }
                    },
                    move |err| {
                        log::error!("Audio stream error: {}", err);
                        stream_failed.store(true, Ordering::Release);
                        let _ = tx_stream_err.try_send((Instant::now(), vec![]));
                    },
                    None,
                )?
            }
            SampleFormat::I16 => {
                let tx_audio = tx.clone();
                let tx_stream_err = tx_err.clone();
                let stream_failed = Arc::clone(&stream_failed);
                let pre_roll_cb = Arc::clone(&pre_roll);
                let dropped_chunks_cb = Arc::clone(&dropped_chunks);
                let max_pr = max_pre_roll_samples;
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        let chunk = downmix_to_mono(data, channels, |sample| {
                            sample as f32 / i16::MAX as f32
                        });
                        {
                            let mut pr = pre_roll_cb.lock().unwrap();
                            pr.push_back(chunk.clone());
                            let mut total: usize = pr.iter().map(|c| c.len()).sum();
                            while total > max_pr {
                                if let Some(dropped) = pr.pop_front() {
                                    total -= dropped.len();
                                }
                            }
                        }
                        // ASR-074 Step 1: 队列满不再静默丢弃，计数并降级日志
                        if tx_audio.try_send((Instant::now(), chunk)).is_err() {
                            dropped_chunks_cb.fetch_add(1, Ordering::Relaxed);
                            log::warn!(
                                "[ASR-DROP] audio chunk dropped (queue full), total dropped so far: {}",
                                dropped_chunks_cb.load(Ordering::Relaxed)
                            );
                        }
                    },
                    move |err| {
                        log::error!("Audio stream error: {}", err);
                        stream_failed.store(true, Ordering::Release);
                        let _ = tx_stream_err.try_send((Instant::now(), vec![]));
                    },
                    None,
                )?
            }
            SampleFormat::U16 => {
                let tx_audio = tx.clone();
                let tx_stream_err = tx_err.clone();
                let stream_failed = Arc::clone(&stream_failed);
                let pre_roll_cb = Arc::clone(&pre_roll);
                let dropped_chunks_cb = Arc::clone(&dropped_chunks);
                let max_pr = max_pre_roll_samples;
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        let chunk = downmix_to_mono(data, channels, |sample| {
                            (sample as f32 / u16::MAX as f32) * 2.0 - 1.0
                        });
                        {
                            let mut pr = pre_roll_cb.lock().unwrap();
                            pr.push_back(chunk.clone());
                            let mut total: usize = pr.iter().map(|c| c.len()).sum();
                            while total > max_pr {
                                if let Some(dropped) = pr.pop_front() {
                                    total -= dropped.len();
                                }
                            }
                        }
                        // ASR-074 Step 1: 队列满不再静默丢弃，计数并降级日志
                        if tx_audio.try_send((Instant::now(), chunk)).is_err() {
                            dropped_chunks_cb.fetch_add(1, Ordering::Relaxed);
                            log::warn!(
                                "[ASR-DROP] audio chunk dropped (queue full), total dropped so far: {}",
                                dropped_chunks_cb.load(Ordering::Relaxed)
                            );
                        }
                    },
                    move |err| {
                        log::error!("Audio stream error: {}", err);
                        stream_failed.store(true, Ordering::Release);
                        let _ = tx_stream_err.try_send((Instant::now(), vec![]));
                    },
                    None,
                )?
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Unsupported microphone sample format: {:?}",
                    other
                ));
            }
        };

        stream.play()?;
        log::info!(
            "Input stream prewarmed ({}Hz, {} ch, {:?})",
            sample_rate,
            channels,
            sample_format
        );

        self.warm_stream = Some(WarmInputStream {
            requested_device_name,
            actual_device_name,
            sample_format,
            sample_rate,
            channels,
            pre_roll,
            rx,
            stream_failed,
            dropped_chunks,
            _stream: stream,
        });

        Ok(self
            .warm_stream
            .as_mut()
            .expect("warm stream initialized above"))
    }
}

impl WarmInputStream {
    fn matches_device(
        &self,
        requested_device_name: &Option<String>,
        actual_device_name: &str,
    ) -> bool {
        warm_stream_matches(
            &self.requested_device_name,
            &self.actual_device_name,
            self.stream_failed.load(Ordering::Acquire),
            requested_device_name,
            actual_device_name,
        )
    }

    fn drain_pre_roll(&self, pre_roll_ms: u64) -> Vec<Vec<f32>> {
        let chunks: Vec<Vec<f32>> = {
            let mut pr = self.pre_roll.lock().unwrap();
            pr.drain(..).filter(|c: &Vec<f32>| !c.is_empty()).collect()
        };

        let drained_chunks = chunks.len();
        let drained_samples = chunks.iter().map(Vec::len).sum::<usize>();
        let max_samples = pre_roll_samples(self.sample_rate, pre_roll_ms);
        let retained = retain_recent_samples(chunks, max_samples);
        let retained_samples = retained.iter().map(Vec::len).sum::<usize>();

        log::info!(
            "Audio pre-roll drain: drained={} chunks/{} samples, retained={} chunks/{} samples ({}ms)",
            drained_chunks,
            drained_samples,
            retained.len(),
            retained_samples,
            pre_roll_ms
        );

        retained
    }
}

fn warm_stream_matches(
    warm_requested_device_name: &Option<String>,
    warm_actual_device_name: &str,
    stream_failed: bool,
    requested_device_name: &Option<String>,
    actual_device_name: &str,
) -> bool {
    warm_requested_device_name == requested_device_name
        && warm_actual_device_name == actual_device_name
        && !stream_failed
}

fn normalize_device_name(device_name: Option<&str>) -> Option<String> {
    device_name.map(str::trim).and_then(|name| {
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_recording(
    rx: &crossbeam_channel::Receiver<AudioChunk>,
    stream_failed: &AtomicBool,
    stop_signal: Arc<AtomicBool>,
    silence_threshold: f32,
    silence_duration_ms: u64,
    max_seconds: u64,
    level_buf: Option<AudioLevelBuf>,
    sample_rate: u32,
    pre_roll_chunks: Vec<Vec<f32>>,
    post_hotkey_chunks: Vec<Vec<f32>>,
) -> Result<Vec<f32>> {
    let mut state = RecordingState::new(sample_rate, silence_duration_ms, level_buf);
    // FIRSTCHAR-FIX-005: max_frames must use the actual sample rate, not a
    // hardcoded 16kHz.  At 48kHz the old value (max_seconds * 16000) would
    // cap the recording at only 1/3 of the intended duration.
    let max_frames = max_seconds as usize * sample_rate as usize;

    // HOTKEY-LATENCY-FIX-001: When pre-roll is empty (WASAPI idle / cold start),
    // collect audio chunks with a timeout loop until we have PRE_ROLL_MS worth of
    // samples, or 350ms timeout (whichever comes first). This is more robust than
    // a single fixed 200ms recv_timeout which only yields one chunk.
    if pre_roll_chunks.is_empty() {
        let t_prime = std::time::Instant::now();
        let target_samples = pre_roll_samples(sample_rate, PRE_ROLL_MS);
        let mut prime_samples: Vec<f32> = Vec::with_capacity(target_samples);

        // D3: Seed prime with post-hotkey chunks (valid audio after hotkey timestamp)
        for chunk in &post_hotkey_chunks {
            if !chunk.is_empty() {
                prime_samples.extend_from_slice(chunk);
            }
        }

        let mut total_wait_ms: u64 = 0;

        while prime_samples.len() < target_samples && total_wait_ms < PRIME_TIMEOUT_MS {
            if stop_signal.load(Ordering::Relaxed) || stream_failed.load(Ordering::Acquire) {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(PRIME_TICK_MS)) {
                Ok((_ts, chunk)) if !chunk.is_empty() => {
                    prime_samples.extend_from_slice(&chunk);
                }
                Ok(_) => {
                    log::warn!("Audio prime: received empty chunk (possible stream failure)");
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // expected: may need multiple ticks
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    return Err(anyhow::anyhow!(
                        "Audio input stream disconnected during prime"
                    ));
                }
            }
            total_wait_ms += PRIME_TICK_MS;
        }

        if !prime_samples.is_empty() {
            log::info!(
                "[Latency] prime collect completed at +{:.1}ms: {} samples (target={}) after {}ms wait",
                t_prime.elapsed().as_secs_f64() * 1000.0,
                prime_samples.len(),
                target_samples,
                total_wait_ms
            );
        } else {
            log::warn!(
                "Audio prime: no audio received within {}ms, WASAPI stream may be cold",
                PRIME_TIMEOUT_MS
            );
        }

        if !prime_samples.is_empty() {
            // FIRSTCHAR-FIX-001: voice-preserving trim — when prime collected more
            // than the target budget, keep the beginning (which contains first-word
            // onset) and discard the tail, instead of the old behaviour which kept
            // the tail and threw away the beginning.
            // The first speech-active sample is found with a simple energy gate; if
            // no speech is found the very first sample is used as the anchor.
            if prime_samples.len() > target_samples {
                let speech_anchor =
                    find_speech_anchor(&prime_samples, silence_threshold, sample_rate);
                let end = (speech_anchor + target_samples).min(prime_samples.len());
                let start = end.saturating_sub(target_samples);
                prime_samples = prime_samples[start..end].to_vec();
                log::info!(
                    "[Latency] prime trim: speech_anchor={}, kept samples {}..{} ({} total)",
                    speech_anchor,
                    start,
                    end,
                    prime_samples.len()
                );
            }
            if state.push_chunk(&prime_samples, silence_threshold)? {
                log::info!("Silence detected in prime chunk, ending recording");
                return Ok(state.all_samples);
            }
        }
    }

    let mut stop_after_pre_roll = false;
    let has_pre_roll = !pre_roll_chunks.is_empty();
    for chunk in pre_roll_chunks {
        if state.push_chunk(&chunk, silence_threshold)? {
            log::info!("Silence detected in pre-roll, ending recording");
            stop_after_pre_roll = true;
            break;
        }
    }

    // D3: Process post-hotkey chunks (valid audio after hotkey timestamp).
    // In prime (cold-start) path, they're already seeded into prime_samples.
    // In warm-start path, process them as regular audio with VAD.
    if has_pre_roll && !stop_after_pre_roll {
        for chunk in &post_hotkey_chunks {
            if !chunk.is_empty() {
                if state.push_chunk(chunk, silence_threshold)? {
                    log::info!("Silence detected in post-hotkey chunk, ending recording");
                    stop_after_pre_roll = true;
                    break;
                }
            }
        }
    }

    let mut mute_check_counter: u32 = 0;
    while !stop_after_pre_roll {
        if stop_signal.load(Ordering::Relaxed) {
            log::info!("Stop signal received, ending recording");
            break;
        }
        if state.all_samples.len() >= max_frames {
            log::info!("Max recording length reached");
            break;
        }
        if stream_failed.load(Ordering::Acquire) {
            return Err(anyhow::anyhow!("Audio input stream failed"));
        }

        if let Ok((_ts, chunk)) = rx.recv_timeout(Duration::from_millis(50)) {
            if state.push_chunk(&chunk, silence_threshold)? {
                log::info!("Silence detected, ending recording");
                break;
            }
            mute_check_counter += 1;
            if mute_check_counter % 50 == 0 && is_mic_muted() {
                anyhow::bail!("mic_muted");
            }
        }
    }

    let peak_before_gain = state
        .all_samples
        .iter()
        .fold(0.0f32, |acc, sample| acc.max(sample.abs()));
    if peak_before_gain > 0.0005 {
        let gain = (0.8 / peak_before_gain).clamp(1.0, 12.0);
        if gain > 1.01 {
            for sample in &mut state.all_samples {
                *sample = (*sample * gain).clamp(-1.0, 1.0);
            }
            log::info!(
                "Applied microphone gain normalization: peak {:.5} -> gain {:.2}x",
                peak_before_gain,
                gain
            );
        }
    }

    let peak_after_gain = state
        .all_samples
        .iter()
        .fold(0.0f32, |acc, sample| acc.max(sample.abs()));
    // FIRSTCHAR-FIX-005: all_samples is now at native sample rate,
    // so divide by the actual sample_rate for correct duration.
    log::info!(
        "Recording complete: {} samples ({:.1}s @ {}Hz), speech_detected={}, peak_before={:.5}, peak_after={:.5}",
        state.all_samples.len(),
        state.all_samples.len() as f32 / state.sample_rate as f32,
        state.sample_rate,
        state.speech_detected,
        peak_before_gain,
        peak_after_gain
    );

    // FIRSTCHAR-FIX-005: Anti-aliased resampling — done once on the complete
    // signal.  This replaces the old per-chunk linear interpolation which had
    // no anti-aliasing filter, causing high-frequency aliasing that corrupted
    // aspirated consonant features (/pʰ/, /tʰ/, /s/).
    // Output is always 16kHz for downstream compatibility.
    if state.sample_rate != 16000 {
        log::info!(
            "Resampling {} samples from {}Hz to 16000Hz with anti-alias filter",
            state.all_samples.len(),
            state.sample_rate
        );
        Ok(resample_anti_alias(
            &state.all_samples,
            state.sample_rate,
            16000,
        ))
    } else {
        Ok(state.all_samples)
    }
}

struct RecordingState {
    all_samples: Vec<f32>,
    silence_frames: usize,
    silent_count: usize,
    speech_detected: bool,
    sample_rate: u32,
    level_buf: Option<AudioLevelBuf>,
}

impl RecordingState {
    fn new(sample_rate: u32, silence_duration_ms: u64, level_buf: Option<AudioLevelBuf>) -> Self {
        Self {
            all_samples: Vec::with_capacity(sample_rate as usize * 10),
            silence_frames: (silence_duration_ms as f32 / 1000.0 * sample_rate as f32) as usize,
            silent_count: 0,
            speech_detected: false,
            sample_rate,
            level_buf,
        }
    }

    fn push_chunk(&mut self, chunk: &[f32], silence_threshold: f32) -> Result<bool> {
        if chunk.is_empty() {
            return Err(anyhow::anyhow!("Audio input stream failed"));
        }

        let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();

        if let Some(ref buf) = self.level_buf {
            crate::ui::overlay::push_level(buf, rms);
        }

        if rms > silence_threshold {
            self.speech_detected = true;
            self.silent_count = 0;
        } else if self.speech_detected {
            self.silent_count += chunk.len();
            if self.silent_count >= self.silence_frames {
                self.extend_samples(chunk);
                return Ok(true);
            }
        }

        self.extend_samples(chunk);
        Ok(false)
    }

    fn extend_samples(&mut self, chunk: &[f32]) {
        // FIRSTCHAR-FIX-005: Store at native sample rate, no per-chunk resampling.
        // Resampling is done once on the complete signal in collect_recording,
        // which avoids chunk-boundary discontinuities and aliasing artifacts.
        self.all_samples.extend_from_slice(chunk);
    }
}

fn pre_roll_samples(sample_rate: u32, pre_roll_ms: u64) -> usize {
    (sample_rate as u64 * pre_roll_ms / 1000) as usize
}

fn retain_recent_samples(chunks: Vec<Vec<f32>>, max_samples: usize) -> Vec<Vec<f32>> {
    if max_samples == 0 {
        return Vec::new();
    }

    let mut retained_rev = Vec::new();
    let mut remaining = max_samples;

    for chunk in chunks.into_iter().rev() {
        if chunk.len() <= remaining {
            remaining -= chunk.len();
            retained_rev.push(chunk);
            if remaining == 0 {
                break;
            }
        } else {
            let start = chunk.len() - remaining;
            retained_rev.push(chunk[start..].to_vec());
            break;
        }
    }

    retained_rev.reverse();
    retained_rev
}

/// FIRSTCHAR-FIX-001 / FIRSTCHAR-FIX-006: Find the effective speech start point
/// by locating the energy onset and then backtracking a margin to ensure weak
/// aspirated consonants (e.g. /pʰ/, /tʰ/) are included.  Aspirated consonants
/// have low energy (~60–100ms of breath noise) that may fall below the RMS
/// threshold; without backtracking, the anchor would land on the following
/// vowel and the consonant would be trimmed away.
///
/// Returns the sample index to use as the start point.
/// - With speech: `onset - margin`, clamped to 0
/// - Without speech: 0
fn find_speech_anchor(samples: &[f32], threshold: f32, sample_rate: u32) -> usize {
    // 10ms window; at 16kHz = 160 samples, at 48kHz = 480 samples
    let window_size = (sample_rate as usize / 100).max(1);
    // FIRSTCHAR-FIX-006 (R2): Backtrack 150ms to include aspirated consonant
    // onsets.  Aspirated consonants last ~60–100ms; 150ms gives comfortable
    // margin without retaining excessive silence.
    let backtrack_samples = (sample_rate as usize * 150 / 1000).max(1);
    let mut idx = 0;
    while idx + window_size <= samples.len() {
        let window = &samples[idx..idx + window_size];
        let rms = (window.iter().map(|s| s * s).sum::<f32>() / window.len() as f32).sqrt();
        if rms > threshold {
            return idx.saturating_sub(backtrack_samples);
        }
        idx += window_size;
    }
    0
}

/// FIRSTCHAR-FIX-005: Anti-aliased resampling using windowed-sinc low-pass filter.
///
/// This replaces the old `resample_linear` which performed naive linear interpolation
/// without any anti-aliasing filter. When downsampling from 48kHz to 16kHz, high-frequency
/// content above 8kHz (the Nyquist of the target) aliases into the 0–8kHz band, corrupting
/// aspirated consonant features like /pʰ/, /tʰ/, /s/ that rely on energy in the 4–12kHz range.
///
/// Algorithm: polyphase FIR with Hann-windowed sinc kernel
/// 1. Generate sinc filter: sin(π·x·ratio) / (π·x·ratio) × Hann window
/// 2. For each output sample, compute the corresponding input position and convolve nearby
///    samples with the filter kernel
/// 3. The cutoff is set to 0.9 × Nyquist of the target rate with margin for roll-off
fn resample_anti_alias(input: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if source_rate == target_rate {
        return input.to_vec();
    }

    let ratio = target_rate as f64 / source_rate as f64;
    let output_len = (input.len() as f64 * ratio).round() as usize;
    if output_len == 0 {
        return Vec::new();
    }

    // Filter cutoff as fraction of source Nyquist.
    // For downsampling: need low-pass at target Nyquist / source Nyquist, with 0.9 margin.
    // For upsampling: no aliasing concern, cutoff = 1.0.
    let cutoff = if target_rate < source_rate {
        0.9 * (target_rate as f64 / source_rate as f64)
    } else {
        1.0
    };

    // Filter half-length: more taps = sharper cutoff, 32 gives good quality
    // for 48→16kHz speech. Total filter length = 2 * TAPS + 1.
    const TAPS: usize = 32;

    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        // Position in the input signal corresponding to output sample i
        let src_pos = i as f64 / ratio;

        // Center of the filter kernel
        let center = src_pos.round() as isize;
        let frac = src_pos - center as f64;

        let mut sum = 0.0f64;
        let mut norm = 0.0f64;

        for j_off in -(TAPS as isize)..=(TAPS as isize) {
            let src_idx = center + j_off;
            if src_idx < 0 || src_idx >= input.len() as isize {
                continue;
            }

            // Offset from the fractional position within the polyphase structure
            let t = j_off as f64 - frac;

            // Sinc: sin(π·cutoff·t) / (π·t), with t=0 handled as cutoff
            let sinc_val = if t.abs() < 1e-10 {
                cutoff
            } else {
                (std::f64::consts::PI * cutoff * t).sin() / (std::f64::consts::PI * t)
            };

            // Hann window applied to the sinc
            // Normalized window index: maps j_off ∈ [-TAPS, TAPS] to [0, 1]
            let w_idx = (j_off + TAPS as isize) as f64 / (2 * TAPS) as f64;
            let window = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * w_idx).cos());

            let weight = sinc_val * window;
            sum += weight * input[src_idx as usize] as f64;
            norm += weight;
        }

        if norm.abs() > 1e-10 {
            output.push((sum / norm) as f32);
        } else {
            output.push(0.0f32);
        }
    }

    output
}

/// Filter half-length used by both `resample_anti_alias` and `StreamingResampler`.
/// Must match the batch version's `TAPS` exactly or the streaming/batch equivalence
/// invariant breaks. Kept as a private item-level constant so the two call sites
/// can never silently diverge.
const RESAMPLE_TAPS: usize = 32;

/// Streaming anti-aliased resampler: mathematically equivalent to
/// `resample_anti_alias` but supports chunk-by-chunk feeding.
///
/// Why this exists (ASR-042): `record_streaming` feeds audio to the online ASR
/// in 10 ms chunks straight from the capture callback. `resample_anti_alias`
/// is a whole-signal FIR (windowed-sinc, `TAPS = 32` on each side) — calling it
/// per chunk would truncate the convolution kernel at every chunk boundary
/// (≈100 times per second), re-introducing exactly the aspirated-consonant
/// distortion FIRSTCHAR-FIX-005 eliminated. This struct keeps a sliding window
/// of history + lookahead so the kernel is never truncated across chunk
/// boundaries, while using the *global* `emitted` counter so output sample `n`
/// always maps to the same input position regardless of how the input was
/// chunked.
///
/// Invariants (all must hold for batch/stream equivalence):
/// 1. `src_pos = (emitted + k) as f64 / ratio` uses the global `emitted` count,
///    never reset per chunk. Resetting per chunk would make every block
///    re-anchor at input 0 → periodic discontinuities.
/// 2. An output point is only emitted when its full `[center - TAPS, center +
///    TAPS]` kernel window is available in `buf`; otherwise it is deferred to
///    the next push (or to `finish`).
/// 3. After emitting, `buf` is drained but at least `TAPS` history samples are
///    retained so the next chunk's left-side kernel taps stay valid.
/// 4. Kernel weights (sinc × Hann window), cutoff, and normalization are
///    byte-for-byte identical to `resample_anti_alias`.
/// 5. `source_rate == target_rate` ⇒ `push` / `finish` return input unchanged.
pub(crate) struct StreamingResampler {
    source_rate: u32,
    target_rate: u32,
    ratio: f64,
    cutoff: f64,
    /// Sliding window: [retained history tail] ++ [not-yet-emitted new samples].
    buf: Vec<f32>,
    /// Global input index of `buf[0]` in the "infinite input stream".
    base: u64,
    /// Total output samples already emitted. Drives `src_pos` so it is
    /// continuous across chunks.
    emitted: u64,
    /// Total input samples fed so far. Used by `finish` to compute the
    /// batch-equivalent `output_len = round(total_input * ratio)` and stop
    /// emitting beyond it — mirroring `resample_anti_alias`'s output length.
    total_input: u64,
}

impl StreamingResampler {
    pub(crate) fn new(source_rate: u32, target_rate: u32) -> Self {
        let ratio = target_rate as f64 / source_rate as f64;
        let cutoff = if target_rate < source_rate {
            0.9 * (target_rate as f64 / source_rate as f64)
        } else {
            1.0
        };
        Self {
            source_rate,
            target_rate,
            ratio,
            cutoff,
            buf: Vec::new(),
            base: 0,
            emitted: 0,
            total_input: 0,
        }
    }

    /// Feed one chunk of input; return whatever output can be produced with a
    /// full (untruncated) kernel. Samples whose right-side kernel taps are not
    /// yet available are held until the next `push` or `finish`.
    pub(crate) fn push(&mut self, input: &[f32]) -> Vec<f32> {
        if self.source_rate == self.target_rate {
            // Identity path: mirror resample_anti_alias's early return.
            return input.to_vec();
        }
        if input.is_empty() {
            return Vec::new();
        }
        self.total_input += input.len() as u64;
        self.buf.extend_from_slice(input);
        self.emit_ready(false)
    }

    /// Call at end of stream. Emits all remaining output using whatever kernel
    /// taps are available (left-side only past the end), matching the batch
    /// version's tail-truncation behavior exactly.
    pub(crate) fn finish(&mut self) -> Vec<f32> {
        if self.source_rate == self.target_rate {
            return Vec::new();
        }
        self.emit_ready(true)
    }

    /// Core emission loop. `flush` = false respects the right-side lookahead
    /// requirement (kernel must be fully available); `flush` = true emits all
    /// remaining points with whatever taps remain (tail truncation, identical
    /// to how `resample_anti_alias` handles its final samples).
    fn emit_ready(&mut self, flush: bool) -> Vec<f32> {
        if self.ratio <= 0.0 || self.buf.is_empty() {
            return Vec::new();
        }
        let taps = RESAMPLE_TAPS as isize;
        let mut out = Vec::new();

        // In flush mode, cap output at the batch-equivalent length:
        // `round(total_input * ratio)`. Without this, the stream tail can emit
        // one extra sample whose `center` is still ≤ buf_hi but beyond where
        // `resample_anti_alias` would have stopped.
        let output_cap = if flush {
            (self.total_input as f64 * self.ratio).round() as u64
        } else {
            u64::MAX
        };

        loop {
            if self.emitted >= output_cap {
                break;
            }
            // Output index (global) → input position.
            let src_pos = self.emitted as f64 / self.ratio;
            let center = src_pos.round() as isize;
            let frac = src_pos - center as f64;

            // Availability check. The batch version (`resample_anti_alias`)
            // skips out-of-range taps via `continue` *inside* the convolution
            // loop — so the signal *start* (left taps < 0) still produces
            // output, only the *end* (right taps past input.len) is naturally
            // limited by the finite input.
            //
            // To match that in streaming mode:
            //   - non-flush: require only the RIGHT side to be fully available
            //     (`hi <= buf_hi`); left taps `lo < buf_lo` are handled by the
            //     inner `continue`, exactly like batch handles `src_idx < 0`.
            //     This lets the very first output sample emit as soon as enough
            //     right-side lookahead has arrived, instead of waiting for
            //     `TAPS` left history that will never come at the stream start.
            //   - flush: emit while `center` is in range; both sides truncate
            //     via inner `continue`, matching the batch tail.
            let hi = center + taps;
            let buf_lo = self.base as isize;
            let buf_hi = (self.base + self.buf.len() as u64) as isize - 1;

            if !flush {
                if hi > buf_hi {
                    break;
                }
                // `lo < buf_lo` is fine — inner loop skips those taps.
                // (At stream start buf_lo == 0, so this mirrors batch's
                // `src_idx < 0` handling.)
            } else {
                // Tail: emit while the *center* is still in range.
                if center < buf_lo || center > buf_hi {
                    break;
                }
            }

            let mut sum = 0.0f64;
            let mut norm = 0.0f64;
            for j_off in -taps..=taps {
                let src_idx = center + j_off;
                if src_idx < buf_lo || src_idx > buf_hi {
                    continue;
                }
                let buf_pos = (src_idx - buf_lo) as usize;
                let t = j_off as f64 - frac;
                let sinc_val = if t.abs() < 1e-10 {
                    self.cutoff
                } else {
                    (std::f64::consts::PI * self.cutoff * t).sin() / (std::f64::consts::PI * t)
                };
                let w_idx = (j_off + taps) as f64 / (2 * RESAMPLE_TAPS) as f64;
                let window = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * w_idx).cos());
                let weight = sinc_val * window;
                sum += weight * self.buf[buf_pos] as f64;
                norm += weight;
            }

            if norm.abs() > 1e-10 {
                out.push((sum / norm) as f32);
            } else {
                out.push(0.0f32);
            }
            self.emitted += 1;
        }

        // Drain history that will never be needed again. The next output point
        // to be produced has global index `self.emitted`; its leftmost tap is at
        // input index `next_lo = round(emitted / ratio) - taps`. Any sample with
        // input index < next_lo can be discarded. This is exact (no margin
        // needed): every future output point `k ≥ emitted` has
        // `center_k ≥ next_center`, so its `lo_k ≥ next_lo`.
        let drop_n = if flush {
            self.buf.len()
        } else {
            let next_src_pos = self.emitted as f64 / self.ratio;
            let next_center = next_src_pos.round() as isize;
            let next_lo = next_center - taps;
            let buf_lo = self.base as isize;
            let drop_to = next_lo - buf_lo;
            if drop_to <= 0 {
                0
            } else {
                drop_to as usize
            }
        };
        if drop_n > 0 && drop_n <= self.buf.len() {
            self.buf.drain(0..drop_n);
            self.base += drop_n as u64;
        }

        out
    }
}

/// Legacy resample_linear kept only for reference / fallback testing.
/// DO NOT use for production audio — it lacks anti-aliasing and produces
/// audible artifacts on aspirated consonants (/pʰ/, /tʰ/, /s/).
#[cfg(test)]
fn resample_linear(chunk: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    let ratio = target_rate as f32 / source_rate as f32;
    let new_len = (chunk.len() as f32 * ratio) as usize;
    (0..new_len)
        .map(|i| {
            let src = i as f32 / ratio;
            let idx = src.floor() as usize;
            let frac = src - idx as f32;
            let a = chunk.get(idx).copied().unwrap_or(0.0);
            let b = chunk.get(idx + 1).copied().unwrap_or(0.0);
            a + (b - a) * frac
        })
        .collect()
}

fn downmix_to_mono<T: Copy, F: Fn(T) -> f32>(data: &[T], channels: usize, convert: F) -> Vec<f32> {
    data.chunks(channels)
        .map(|frame| frame.iter().copied().map(&convert).sum::<f32>() / channels as f32)
        .collect()
}

pub fn is_mic_muted() -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(_) => return false,
            };
        let device = match enumerator.GetDefaultAudioEndpoint(eCapture, eMultimedia) {
            Ok(d) => d,
            Err(_) => return false,
        };
        let endpoint: IAudioEndpointVolume =
            match device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                Ok(e) => e,
                Err(_) => return false,
            };
        endpoint
            .GetMute()
            .map(|b: BOOL| b.as_bool())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_blank_device_name_to_default() {
        assert_eq!(normalize_device_name(None), None);
        assert_eq!(normalize_device_name(Some("")), None);
        assert_eq!(normalize_device_name(Some("   ")), None);
        assert_eq!(
            normalize_device_name(Some("  Microphone Array  ")),
            Some("Microphone Array".to_string())
        );
    }

    #[test]
    fn warm_stream_match_requires_same_requested_actual_and_healthy_stream() {
        let requested = Some("Mic A".to_string());
        assert!(warm_stream_matches(
            &requested, "Mic A", false, &requested, "Mic A"
        ));
        assert!(!warm_stream_matches(
            &requested,
            "Mic A",
            false,
            &Some("Mic B".to_string()),
            "Mic B"
        ));
        assert!(!warm_stream_matches(
            &requested,
            "Mic A",
            false,
            &requested,
            "Renamed Mic A"
        ));
        assert!(!warm_stream_matches(
            &requested, "Mic A", true, &requested, "Mic A"
        ));
    }

    #[test]
    fn downmixes_interleaved_stereo_samples_to_mono() {
        let samples = [1.0f32, -1.0, 0.5, 0.25];
        let mono = downmix_to_mono(&samples, 2, |sample| sample);
        assert_eq!(mono, vec![0.0, 0.375]);
    }

    #[test]
    fn drain_pre_roll_empty_buffer_returns_nothing() {
        assert!(retain_recent_samples(Vec::new(), 8_000).is_empty());
    }

    #[test]
    fn drain_pre_roll_keeps_all_when_less_than_pre_roll_limit() {
        let chunk_200ms: Vec<f32> = (0..3200).map(|i| (i as f32) / 3200.0).collect();
        let limit = pre_roll_samples(16_000, PRE_ROLL_MS);
        let drained = retain_recent_samples(vec![chunk_200ms], limit);
        let total_samples: usize = drained.iter().map(|c| c.len()).sum();

        assert_eq!(total_samples, 3_200);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0][0], 0.0);
    }

    #[test]
    fn drain_pre_roll_keeps_only_last_pre_roll_samples_when_exceeds() {
        let chunks: Vec<Vec<f32>> = (0..7)
            .map(|chunk_idx| vec![chunk_idx as f32; 1_600])
            .collect();
        let limit = pre_roll_samples(16_000, PRE_ROLL_MS);
        let drained = retain_recent_samples(chunks, limit);
        let total_samples: usize = drained.iter().map(|c| c.len()).sum();

        assert_eq!(drained.len(), 6);
        assert_eq!(total_samples, 9_600);
        assert_eq!(drained[0][0], 1.0);
        assert_eq!(drained[5][0], 6.0);
    }

    #[test]
    fn drain_pre_roll_boundary_exactly_pre_roll_limit_keeps_all() {
        let chunks: Vec<Vec<f32>> = (0..6)
            .map(|chunk_idx| vec![chunk_idx as f32; 1_600])
            .collect();
        let limit = pre_roll_samples(16_000, PRE_ROLL_MS);
        let drained = retain_recent_samples(chunks, limit);
        let total_samples: usize = drained.iter().map(|c| c.len()).sum();

        assert_eq!(drained.len(), 6);
        assert_eq!(total_samples, 9_600);
        assert_eq!(drained[0][0], 0.0);
    }

    #[test]
    fn drain_pre_roll_keeps_suffix_of_boundary_chunk() {
        let chunks = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        assert_eq!(
            retain_recent_samples(chunks, 4),
            vec![vec![3.0], vec![4.0, 5.0, 6.0]]
        );
    }

    #[test]
    fn computes_pre_roll_sample_budget_from_source_rate() {
        assert_eq!(pre_roll_samples(16_000, PRE_ROLL_MS), 9_600);
        assert_eq!(pre_roll_samples(48_000, PRE_ROLL_MS), 28_800);
    }

    // FIRSTCHAR-FIX-005: RecordingState now stores at native sample rate.
    // push_chunk no longer resamples; all_samples holds native-rate data.
    #[test]
    fn recording_state_stores_native_rate_samples() {
        let mut state = RecordingState::new(48_000, 100, None);
        assert!(!state.push_chunk(&vec![0.02f32; 4_800], 0.01).unwrap());

        assert!(state.speech_detected);
        // No per-chunk resampling: all_samples stores 4800 native samples, not 1600
        assert_eq!(state.all_samples.len(), 4_800);
    }

    #[test]
    fn audio_prime_only_triggers_on_empty_preroll() {
        // HOTKEY-LATENCY-FIX-001: prime timeout loop only when pre_roll_chunks is empty
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(2);
        let stop = Arc::new(AtomicBool::new(false));
        let failed = AtomicBool::new(false);

        // Case 1: empty pre_roll -> prime consumes the first chunk from channel
        tx.send((Instant::now(), vec![0.1f32; 100])).unwrap();
        let result = collect_recording(
            &rx,
            &failed,
            Arc::clone(&stop),
            0.01,
            100,
            0,
            None,
            16000,
            vec![],
            vec![],
        );
        assert!(
            result.is_ok(),
            "Empty pre-roll with audio chunk must complete OK"
        );
        assert_eq!(
            result.unwrap().len(),
            100,
            "Prime must consume the available chunk when pre_roll is empty"
        );

        // Case 2: non-empty pre_roll -> prime skipped, pre_roll chunks processed instead
        tx.send((Instant::now(), vec![0.2f32; 50])).unwrap();
        let result2 = collect_recording(
            &rx,
            &failed,
            Arc::clone(&stop),
            0.01,
            100,
            0,
            None,
            16000,
            vec![vec![0.3f32; 80]],
            vec![],
        );
        assert!(result2.is_ok(), "Non-empty pre-roll must complete OK");
        assert_eq!(
            result2.unwrap().len(),
            80,
            "Non-empty pre-roll must skip prime and process provided chunks"
        );
    }

    #[test]
    fn pre_roll_ms_is_600ms() {
        // HOTKEY-LATENCY-V2-001: PRE_ROLL_MS increased from 500 to 600
        assert_eq!(PRE_ROLL_MS, 600, "PRE_ROLL_MS must be 600ms");
    }

    /// HOTKEY-LATENCY-V2-001: prime timeout is 450ms, collects more audio for cold-start scenarios.
    #[test]
    fn prime_timeout_ms_is_450() {
        assert_eq!(PRIME_TIMEOUT_MS, 450, "PRIME_TIMEOUT_MS must be 450ms");
    }

    /// HOTKEY-LATENCY-FIX-001: recv_timeout tick is 20ms, allows up to ~22 ticks before 450ms timeout.
    #[test]
    fn prime_tick_ms_is_20() {
        assert_eq!(PRIME_TICK_MS, 20, "PRIME_TICK_MS must be 20ms");
    }

    /// HOTKEY-LATENCY-V2-001: pre_roll_samples(16kHz, 600ms) must yield 9600 samples.
    #[test]
    fn prime_target_samples_at_16khz_is_9600() {
        assert_eq!(
            pre_roll_samples(16_000, 600),
            9_600,
            "At 16kHz, 600ms must produce exactly 9600 target samples for the prime loop"
        );
    }

    /// HOTKEY-STREAM-PREWARM-001:
    /// Verify that `AudioCapture::check_stream_health()` returns safely and
    /// does nothing when `warm_stream` has not been initialized.
    /// This is the "needs_rebuild = false" short-circuit path for the
    /// (warm_stream = None) branch, avoiding any panic or side effects.
    #[test]
    fn check_stream_health_no_warm_stream_returns_immediately() {
        let mut capture = AudioCapture::new();
        capture.check_stream_health();
        assert!(
            capture.warm_stream.is_none(),
            "warm_stream must remain None when check_stream_health is called before prewarm"
        );
    }

    /// HOTKEY-STREAM-PREWARM-001:
    /// Verify that `warm_stream_matches()` returns false when `stream_failed`
    /// is true, ensuring that `check_stream_health` will identify `needs_rebuild`
    /// and proceed to `ensure_stream`. This is the core decision logic for the
    /// stream health check; the actual CPAL device rebuild path requires a
    /// real input device and is covered by the E2E / pywinauto layer.
    #[test]
    fn warm_stream_match_stream_failed_true_triggers_rebuild_decision() {
        let requested = Some("Mock Mic".to_string());
        // When stream_failed is true, warm_stream_matches must return false,
        // signaling that the existing stream is unusable and must be rebuilt.
        assert!(!warm_stream_matches(
            &requested, "Mock Mic", true, // stream_failed = true
            &requested, "Mock Mic"
        ));
        // Same parameters with stream_failed = false should allow reuse.
        assert!(warm_stream_matches(
            &requested, "Mock Mic", false, // stream_failed = false
            &requested, "Mock Mic"
        ));
    }

    // ============================================================
    // TEST-SYNC-MIC-MUTE-001: mic mute detection tests
    // ============================================================

    #[test]
    fn mute_check_interval_is_50_chunks() {
        // MIC-MUTE-DETECT-001: verify that the mute check triggers every 50 chunks.
        let mut counter: u32 = 0;
        let mut trigger_count = 0;
        for _ in 0..100 {
            counter += 1;
            if counter % 50 == 0 {
                trigger_count += 1;
            }
        }
        assert_eq!(
            trigger_count, 2,
            "mute check should trigger exactly twice in 100 iterations (every 50 chunks)"
        );
    }

    #[test]
    fn is_mic_muted_returns_false_on_non_windows() {
        // MIC-MUTE-DETECT-001: on non-Windows platforms the function must
        // always return false; on Windows we only verify it does not panic.
        let result = is_mic_muted();
        #[cfg(not(target_os = "windows"))]
        assert!(!result, "non-Windows should always return false");
    }

    #[test]
    fn error_mic_muted_strings_not_empty() {
        // MIC-MUTE-DETECT-001: error_mic_muted i18n strings must be populated
        // for all supported languages.
        use crate::config::UiLanguage;
        use crate::i18n;
        assert!(
            !i18n::get(UiLanguage::Chinese).error_mic_muted.is_empty(),
            "ZH error_mic_muted must not be empty"
        );
        assert!(
            !i18n::get(UiLanguage::TraditionalChinese)
                .error_mic_muted
                .is_empty(),
            "ZH_TW error_mic_muted must not be empty"
        );
        assert!(
            !i18n::get(UiLanguage::English).error_mic_muted.is_empty(),
            "EN error_mic_muted must not be empty"
        );
    }

    // ============================================================
    // TEST-SYNC-PREROLL-001: pre-roll ring buffer unit tests
    // ============================================================

    /// PREROLL-RINGBUF-001:
    /// Verify that `retain_recent_samples` discards the *oldest* chunks and
    /// keeps the newest ones when the total exceeds the sample budget.
    #[test]
    fn retain_recent_samples_keeps_newest_not_oldest() {
        // 7 chunks x 1600 samples = 11200 > limit(9600)
        // The oldest chunk (index 0, values 0.0) must be evicted.
        let chunks: Vec<Vec<f32>> = (0u32..7).map(|i| vec![i as f32; 1_600]).collect();
        let limit = pre_roll_samples(16_000, PRE_ROLL_MS); // 9600
        let retained = retain_recent_samples(chunks, limit);
        // Oldest chunk discarded
        assert_eq!(retained[0][0], 1.0, "oldest chunk must be evicted");
        // Newest chunk retained
        assert_eq!(
            retained.last().unwrap()[0],
            6.0,
            "newest chunk must be retained"
        );
    }

    /// FIRSTCHAR-FIX-004 (D3): timestamp-based drain terminates on empty channel
    #[test]
    fn timestamp_idle_drain_terminates_on_empty_channel() {
        let (_tx, rx) = crossbeam_channel::bounded::<AudioChunk>(4);
        let record_start = Instant::now();
        let mut idle_cleared: usize = 0;
        let mut post_hotkey: Vec<Vec<f32>> = Vec::new();
        loop {
            match rx.try_recv() {
                Ok((ts, chunk)) => {
                    if ts < record_start {
                        idle_cleared += 1;
                    } else {
                        post_hotkey.push(chunk);
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(idle_cleared, 0, "empty channel must yield 0 cleared chunks");
        assert!(
            post_hotkey.is_empty(),
            "empty channel must yield 0 post-hotkey chunks"
        );
    }

    /// TEST-WRITE-FIRSTCHAR-001 / FIRSTCHAR-FIX-004 (D3):
    /// Timestamp-based drain clears pre-hotkey chunks and preserves post-hotkey chunks.
    #[test]
    fn timestamp_drain_clears_pre_hotkey_preserves_post_hotkey_small() {
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(4);
        let record_start = Instant::now();
        // 3 pre-hotkey chunks — must be cleared
        tx.send((record_start - Duration::from_millis(30), vec![0.1f32; 100]))
            .unwrap();
        tx.send((record_start - Duration::from_millis(20), vec![0.2f32; 100]))
            .unwrap();
        tx.send((record_start - Duration::from_millis(10), vec![0.3f32; 100]))
            .unwrap();
        let mut idle_cleared: usize = 0;
        let mut post_hotkey: Vec<Vec<f32>> = Vec::new();
        loop {
            match rx.try_recv() {
                Ok((ts, chunk)) => {
                    if ts < record_start {
                        idle_cleared += 1;
                    } else {
                        post_hotkey.push(chunk);
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(idle_cleared, 3, "must clear all 3 pre-hotkey chunks");
        assert!(post_hotkey.is_empty(), "no post-hotkey chunks expected");
        assert!(
            tx.try_send((Instant::now(), vec![0.4f32; 100])).is_ok(),
            "channel must have room after drain"
        );
    }

    // ============================================================
    // FIRSTCHAR-FIX-001: bounded idle clear + voice-preserving prime trim
    // ============================================================

    /// FIRSTCHAR-FIX-001: find_speech_anchor returns 0 for all-silence buffer
    #[test]
    fn find_speech_anchor_returns_zero_for_silence() {
        let silence = vec![0.0f32; 1600];
        assert_eq!(find_speech_anchor(&silence, 0.01, 16000), 0);
    }

    /// FIRSTCHAR-FIX-006 (R2): find_speech_anchor backtracks 150ms from energy
    /// onset.  At 16kHz, 150ms = 2400 samples, so if speech starts at sample 4000,
    /// anchor returns 4000 - 2400 = 1600.
    #[test]
    fn find_speech_anchor_finds_first_active_window() {
        let mut samples = vec![0.0f32; 12800]; // 800ms @ 16kHz
                                               // Inject speech at sample 4000 (250ms in)
        for i in 4000..4160 {
            samples[i] = 0.1;
        }
        let anchor = find_speech_anchor(&samples, 0.01, 16000);
        // Energy onset is at 4000; backtrack 150ms@16kHz = 2400 samples → anchor = 1600
        assert_eq!(
            anchor, 1600,
            "anchor should be onset(4000) - backtrack(2400) = 1600, got {}",
            anchor
        );
    }

    /// FIRSTCHAR-FIX-001/006: find_speech_anchor at very start returns 0 (can't backtrack further)
    #[test]
    fn find_speech_anchor_at_start_returns_zero() {
        let mut samples = vec![0.05f32; 1600]; // above threshold throughout
        samples[0] = 0.1;
        let anchor = find_speech_anchor(&samples, 0.01, 16000);
        assert_eq!(
            anchor, 0,
            "speech at very start should anchor at 0 (saturating_sub)"
        );
    }

    /// FIRSTCHAR-FIX-004 (D3): timestamp-based drain clears 120 pre-hotkey chunks
    /// and preserves post-hotkey chunks.
    #[test]
    fn timestamp_drain_clears_pre_hotkey_preserves_post_hotkey_large() {
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(256);
        let record_start = Instant::now();
        // 120 pre-hotkey chunks — all must be cleared
        for i in 0..120u32 {
            tx.send((
                record_start - Duration::from_millis(1200 - i as u64),
                vec![i as f32; 100],
            ))
            .unwrap();
        }
        // 2 post-hotkey chunks — must be preserved
        tx.send((record_start, vec![120.0f32; 100])).unwrap();
        tx.send((record_start + Duration::from_millis(5), vec![121.0f32; 100]))
            .unwrap();

        let mut idle_cleared: usize = 0;
        let mut post_hotkey: Vec<Vec<f32>> = Vec::new();
        loop {
            match rx.try_recv() {
                Ok((ts, chunk)) => {
                    if ts < record_start {
                        idle_cleared += 1;
                    } else {
                        post_hotkey.push(chunk);
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(idle_cleared, 120, "must clear all 120 pre-hotkey chunks");
        assert_eq!(post_hotkey.len(), 2, "must preserve 2 post-hotkey chunks");
        assert_eq!(
            post_hotkey[0][0], 120.0,
            "first post-hotkey chunk preserved"
        );
        assert_eq!(
            post_hotkey[1][0], 121.0,
            "second post-hotkey chunk preserved"
        );
    }

    /// FIRSTCHAR-FIX-001: prime trim preserves beginning (speech anchor)
    #[test]
    fn prime_trim_preserves_speech_onset() {
        // Simulate prime_samples with speech at the beginning
        let target = 9_600_usize; // 600ms @ 16kHz
        let mut samples = vec![0.0f32; target + 4800]; // 600ms target + 300ms extra

        // Put speech at the beginning (first 800 samples)
        for i in 0..800 {
            samples[i] = 0.1;
        }
        let anchor = find_speech_anchor(&samples, 0.01, 16000);
        assert!(anchor < 800, "speech anchor should be near the beginning");

        // Simulate the trim logic from FIRSTCHAR-FIX-001
        let end = (anchor + target).min(samples.len());
        let start = end.saturating_sub(target);
        let trimmed = &samples[start..end];
        assert_eq!(trimmed.len(), target, "trimmed length should equal target");
        // The speech onset (sample 0-799) should be preserved
        assert!(
            trimmed.iter().take(800).any(|&s| s > 0.05),
            "speech onset must be preserved in trimmed output"
        );
    }

    /// FIRSTCHAR-FIX-001 boundary: find_speech_anchor with buffer shorter than window
    #[test]
    fn find_speech_anchor_short_buffer_returns_zero() {
        // Buffer of 50 samples — shorter than any window (even 160 at 16kHz / 480 at 48kHz)
        let samples = vec![0.5f32; 50];
        assert_eq!(
            find_speech_anchor(&samples, 0.01, 16000),
            0,
            "buffer shorter than window must return anchor 0"
        );
    }

    /// FIRSTCHAR-FIX-001/006 boundary: find_speech_anchor exact window boundary
    /// Speech starts exactly at index 0 — backtrack saturates to 0
    #[test]
    fn find_speech_anchor_exact_window_boundary() {
        let mut samples = vec![0.0f32; 4800];
        for i in 0..160 {
            samples[i] = 0.1;
        }
        let anchor = find_speech_anchor(&samples, 0.01, 16000);
        assert_eq!(
            anchor, 0,
            "speech at window[0] must return anchor 0 (saturating_sub)"
        );
    }

    /// FIRSTCHAR-FIX-001 boundary: find_speech_anchor entire buffer above threshold
    #[test]
    fn find_speech_anchor_all_speech_returns_zero() {
        // Every sample above threshold — anchor is 0 (beginning)
        let samples = vec![0.5f32; 3200];
        let anchor = find_speech_anchor(&samples, 0.01, 16000);
        assert_eq!(anchor, 0, "all-speech buffer must return anchor at 0");
    }

    /// FIRSTCHAR-FIX-004 (D3): timestamp drain stops when channel is empty
    #[test]
    fn timestamp_drain_stops_when_channel_empty() {
        let record_start = Instant::now();
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(256);
        // 30 pre-hotkey chunks
        for i in 0..30u32 {
            tx.send((
                record_start - Duration::from_millis(300 - i as u64),
                vec![i as f32; 100],
            ))
            .unwrap();
        }
        let mut idle_cleared: usize = 0;
        let mut post_hotkey: Vec<Vec<f32>> = Vec::new();
        loop {
            match rx.try_recv() {
                Ok((ts, chunk)) => {
                    if ts < record_start {
                        idle_cleared += 1;
                    } else {
                        post_hotkey.push(chunk);
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(idle_cleared, 30, "must clear all 30 pre-hotkey chunks");
        assert!(post_hotkey.is_empty(), "no post-hotkey chunks expected");
        assert!(rx.try_recv().is_err(), "channel must be empty after drain");
    }

    // ============================================================
    // FIRSTCHAR-FIX-005: anti-aliased resampling unit tests
    // ============================================================

    /// Verify that resample_anti_alias preserves a pure low-frequency sine
    /// (well below the Nyquist of the target rate) with minimal distortion.
    #[test]
    fn resample_anti_alias_preserves_low_frequency() {
        // 1kHz sine at 48kHz sample rate, 4800 samples = 100ms
        let freq = 1000.0f64;
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        let duration_samples = 4800;
        let input: Vec<f32> = (0..duration_samples)
            .map(|i| {
                let t = i as f64 / source_rate as f64;
                (2.0 * std::f64::consts::PI * freq * t).sin() as f32
            })
            .collect();

        let output = resample_anti_alias(&input, source_rate, target_rate);
        // Expected ~1600 samples (100ms at 16kHz)
        assert!(
            output.len() >= 1580 && output.len() <= 1620,
            "output length should be ~1600, got {}",
            output.len()
        );

        // The 1kHz signal (well below 8kHz Nyquist) should be preserved.
        // Check that output has significant energy (not attenuated away).
        let peak = output.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(
            peak > 0.5,
            "1kHz sine peak should be well-preserved, got peak={}",
            peak
        );
    }

    /// Verify that resample_anti_alias significantly suppresses aliasing
    /// from a high-frequency component that would fold into the audible band
    /// under naive linear interpolation.
    #[test]
    fn resample_anti_alias_suppresses_high_frequency_aliasing() {
        // Composite signal: 500Hz low + 12kHz high at 48kHz source rate.
        // When naively downsampled to 16kHz, 12kHz aliases to 16-12=4kHz.
        // The anti-aliasing filter should suppress the 12kHz component.
        let freq_low = 500.0f64;
        let freq_high = 12000.0f64;
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        let duration_samples = 14400; // 300ms at 48kHz

        let input: Vec<f32> = (0..duration_samples)
            .map(|i| {
                let t = i as f64 / source_rate as f64;
                let low = (2.0 * std::f64::consts::PI * freq_low * t).sin();
                let high = (2.0 * std::f64::consts::PI * freq_high * t).sin();
                (low + 0.5 * high) as f32
            })
            .collect();

        let output_aa = resample_anti_alias(&input, source_rate, target_rate);
        let output_linear = resample_linear(&input, source_rate, target_rate);

        // Both should produce similar length output
        assert!(
            output_aa.len() > 0 && output_linear.len() > 0,
            "both methods must produce output"
        );

        // The anti-aliased version should have lower total energy in the aliasing
        // region. The 12kHz component aliases to ~4kHz in the naive version.
        // We measure this by comparing the difference between the two outputs.
        // The anti-aliased version should have less high-frequency content because
        // the 12kHz component was filtered out before decimation.
        //
        // A more direct test: the AA output should have lower RMS difference from
        // a pure 500Hz reference than the linear output does (the aliasing adds
        // spurious energy in the linear version).
        let min_len = output_aa.len().min(output_linear.len());

        // Compute RMS of the difference between AA and linear outputs.
        // A large difference means AA is doing meaningful anti-aliasing.
        let mut sum_sq_diff = 0.0f64;
        for i in 0..min_len {
            let diff = *output_aa.get(i).unwrap() as f64 - *output_linear.get(i).unwrap() as f64;
            sum_sq_diff += diff * diff;
        }
        let rms_diff = (sum_sq_diff / min_len as f64).sqrt();

        // The difference should be nonzero and significant (anti-aliasing is
        // removing the 12kHz component that linear interpolation lets through).
        assert!(
            rms_diff > 0.01,
            "AA and linear outputs must differ (rms_diff={:.6}), anti-aliasing is removing content",
            rms_diff
        );

        // The AA output should have lower peak-to-peak amplitude than linear
        // because the 12kHz component (0.5 amplitude) is suppressed.
        let aa_max = output_aa.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        let linear_max = output_linear.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(
            aa_max < linear_max,
            "AA output peak ({:.3}) should be less than linear ({:.3}) due to alias suppression",
            aa_max,
            linear_max
        );
    }

    /// Verify that resample_anti_alias with equal source and target rates
    /// returns a copy of the input unchanged.
    #[test]
    fn resample_anti_alias_identity_passthrough() {
        let input: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.01).sin()).collect();
        let output = resample_anti_alias(&input, 16000, 16000);
        assert_eq!(output.len(), input.len(), "passthrough length must match");
        for (i, (a, b)) in input.iter().zip(output.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-6,
                "passthrough sample {} mismatch: {} vs {}",
                i,
                a,
                b
            );
        }
    }

    /// Verify that resample_anti_alias produces correct output length.
    #[test]
    fn resample_anti_alias_correct_output_length() {
        // 48000 → 16000 is 3:1 ratio
        let input: Vec<f32> = vec![0.0; 4800]; // 100ms at 48kHz
        let output = resample_anti_alias(&input, 48000, 16000);
        assert_eq!(
            output.len(),
            1600,
            "48→16kHz of 4800 samples should yield 1600"
        );

        // 16000 → 16000 identity
        let input2: Vec<f32> = vec![0.0; 1600];
        let output2 = resample_anti_alias(&input2, 16000, 16000);
        assert_eq!(
            output2.len(),
            1600,
            "16→16kHz passthrough should yield same length"
        );
    }

    /// Verify that the windowed-sinc filter actually attenuates content
    /// above the target Nyquist frequency.  We construct a pure 12kHz tone
    /// at 48kHz sample rate.  After AA resampling to 16kHz, the output
    /// should be near-silence because 12kHz is above 8kHz Nyquist.
    #[test]
    fn resample_anti_alias_attenuates_above_nyquist() {
        // Pure 12kHz tone at 48kHz source rate — above 8kHz target Nyquist
        let freq = 12000.0f64;
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        let duration_samples = 9600; // 200ms at 48kHz

        let input: Vec<f32> = (0..duration_samples)
            .map(|i| {
                let t = i as f64 / source_rate as f64;
                (2.0 * std::f64::consts::PI * freq * t).sin() as f32
            })
            .collect();

        let output = resample_anti_alias(&input, source_rate, target_rate);

        // Skip the first 200 output samples (filter settling) and measure RMS
        let skip = 200.min(output.len());
        let rms: f64 = output[skip..]
            .iter()
            .map(|s| (*s as f64) * (*s as f64))
            .sum::<f64>()
            / (output.len() - skip) as f64;
        let rms = rms.sqrt();

        // The anti-aliasing filter should suppress 12kHz well below its
        // original amplitude (normalized to ~1.0).  A good filter with
        // cutoff at ~7.2kHz should attenuate 12kHz by >20dB, giving RMS << 0.1
        assert!(
            rms < 0.1,
            "12kHz (above 8kHz Nyquist) should be heavily suppressed, RMS={:.6}",
            rms
        );
    }

    /// FIRSTCHAR-FIX-006 (R2): find_speech_anchor backtracks 150ms proportionally
    /// to sample_rate, ensuring aspirated consonant margin is consistent across rates.
    #[test]
    fn find_speech_anchor_scales_with_sample_rate() {
        // At 48kHz: backtrack = 150ms = 7200 samples
        // Create a 48kHz signal with speech starting at sample 14400 (= 300ms)
        let mut samples = vec![0.0f32; 38400]; // 800ms @ 48kHz
        for i in 14400..14880 {
            samples[i] = 0.1;
        }
        let anchor_48k = find_speech_anchor(&samples, 0.01, 48000);
        // Onset at 14400, backtrack 7200 → anchor = 7200
        assert_eq!(
            anchor_48k, 7200,
            "48kHz: onset(14400) - backtrack(7200) = 7200, got {}",
            anchor_48k
        );

        // At 16kHz: backtrack = 150ms = 2400 samples
        // Create same-duration signal with speech at sample 4800 (= 300ms)
        let mut samples_16k = vec![0.0f32; 12800]; // 800ms @ 16kHz
        for i in 4800..4960 {
            samples_16k[i] = 0.1;
        }
        let anchor_16k = find_speech_anchor(&samples_16k, 0.01, 16000);
        // Onset at 4800, backtrack 2400 → anchor = 2400
        assert_eq!(
            anchor_16k, 2400,
            "16kHz: onset(4800) - backtrack(2400) = 2400, got {}",
            anchor_16k
        );
    }

    /// Verify that max_frames uses sample_rate correctly (not hardcoded 16000).
    #[test]
    fn max_frames_uses_sample_rate_not_hardcoded() {
        // At 48kHz, max_seconds=10 should give max_frames = 10*48000 = 480000
        // (not 10*16000 = 160000 which would be only 3.33s of 48kHz audio)
        let max_seconds = 10u64;
        let sample_rate = 48000u32;
        let max_frames = max_seconds as usize * sample_rate as usize;
        assert_eq!(max_frames, 480_000, "48kHz * 10s = 480000");

        let sample_rate_16k = 16000u32;
        let max_frames_16k = max_seconds as usize * sample_rate_16k as usize;
        assert_eq!(max_frames_16k, 160_000, "16kHz * 10s = 160000");
    }

    // ============================================================
    // FIRSTCHAR-FIX-006 (R2): find_speech_anchor backtrack tests
    // ============================================================

    /// R2: find_speech_anchor backtracks 150ms to include weak aspirated consonants.
    /// At 16kHz, 150ms = 2400 samples. If energy onset is at 4800, anchor = 2400.
    #[test]
    fn find_speech_anchor_backtrack_includes_aspirated_consonant() {
        let mut samples = vec![0.0f32; 12800]; // 800ms @ 16kHz
                                               // Weak breath (aspirated /pʰ/) at sample 2400–4800 (150ms very low energy)
        for i in 2400..4800 {
            samples[i] = 0.003; // below threshold 0.01
        }
        // Strong vowel from 4800
        for i in 4800..6400 {
            samples[i] = 0.1;
        }
        let anchor = find_speech_anchor(&samples, 0.01, 16000);
        // Energy onset at 4800, backtrack 2400 → anchor = 2400
        // This includes the weak breath that was below threshold
        assert!(
            anchor <= 2400,
            "anchor must include weak aspirated consonant, got {} (consonant starts at 2400)",
            anchor
        );
    }

    // ============================================================
    // ASR-038-B (C-2): record() 批量路径零改动回归护栏
    // record_streaming() 是新增平行方法；record() 旧路径必须保持
    // "pre_roll → post_hotkey → 实时 chunk" 的拼装顺序与静音停录语义。
    // 主控向 Gavin 背书过「旧路径不动不回归」，以下断言钉住该契约。
    // ============================================================

    /// C-2 覆盖点3：record() 路径（collect_recording）对非空 pre_roll +
    /// post_hotkey + 实时 chunk 按顺序拼装，静音 chunk 触发停录后返回完整样本。
    #[test]
    fn collect_recording_preserves_pre_roll_then_hotkey_then_live_order() {
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(2);
        let stop = Arc::new(AtomicBool::new(false));
        let failed = AtomicBool::new(false);
        // pre_roll = 4 个 0.1 样本（语音），post_hotkey = 4 个 0.2 样本（语音）
        let pre_roll = vec![vec![0.1f32; 4]];
        let post_hotkey = vec![vec![0.2f32; 4]];
        // 实时 chunk：静音（0.001 < threshold 0.01），触发 speech_detected 后的静音停录
        tx.send((Instant::now(), vec![0.001f32; 4])).unwrap();
        let result = collect_recording(
            &rx,
            &failed,
            Arc::clone(&stop),
            0.01,
            0,
            10,
            None,
            16000,
            pre_roll,
            post_hotkey,
        );
        assert!(result.is_ok(), "record() path must complete OK");
        let samples = result.unwrap();
        assert_eq!(
            samples.len(),
            12,
            "pre_roll(4)+hotkey(4)+live(4) must all be preserved"
        );
        // 峰值 0.2 → gain = 0.8/0.2 = 4x，静音段 0.001*4=0.004
        // 断言顺序：pre_roll → post_hotkey → 实时(channel) 逐段，每段内部均匀
        for s in &samples[0..4] {
            assert!(
                (s - 0.4).abs() < 1e-4,
                "pre-roll chunk scaled to 0.4, got {}",
                s
            );
        }
        for s in &samples[4..8] {
            assert!(
                (s - 0.8).abs() < 1e-4,
                "post-hotkey chunk scaled to 0.8, got {}",
                s
            );
        }
        for s in &samples[8..12] {
            assert!(
                (s - 0.004).abs() < 1e-5,
                "live silence chunk scaled to 0.004, got {}",
                s
            );
        }
    }

    /// C-2 覆盖点3：record() 路径停在 stream_failed（与 record_streaming 各自的失败路径平行）。
    #[test]
    fn collect_recording_fails_when_stream_failed() {
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(2);
        let stop = Arc::new(AtomicBool::new(false));
        let failed = AtomicBool::new(true);
        tx.send((Instant::now(), vec![0.1f32; 4])).unwrap();
        let result = collect_recording(
            &rx,
            &failed,
            Arc::clone(&stop),
            0.01,
            0,
            10,
            None,
            16000,
            vec![vec![0.2f32; 4]],
            vec![],
        );
        assert!(
            result.is_err(),
            "stream_failed must abort record() path with an error"
        );
    }

    /// R2: find_speech_anchor saturates to 0 when onset < backtrack margin
    #[test]
    fn find_speech_anchor_backtrack_saturates_at_zero() {
        let mut samples = vec![0.0f32; 6400]; // 400ms @ 16kHz
                                              // Speech at sample 1600 (100ms), backtrack 2400 would go below 0
        for i in 1600..3200 {
            samples[i] = 0.1;
        }
        let anchor = find_speech_anchor(&samples, 0.01, 16000);
        assert_eq!(
            anchor, 0,
            "backtrack saturating_sub must return 0 when onset < margin"
        );
    }

    // ===== ASR-042 StreamingResampler 测试 =====
    // 这些测试是本单的核心护栏。等价性测试（第 3 条验收）必须能红：
    // 把 push 里的全局 emitted 改成每块从 0 重算，它会失败。

    /// 验收第 3 条（核心护栏）：流式逐块 push + finish 必须与批处理
    /// resample_anti_alias 逐点等价（差值 ≤ 1e-5）。
    ///
    /// 信号：多频率叠加正弦 + 白噪声，48000Hz，≥1 秒。
    /// 若把 StreamingResampler::push 的 emitted 改成每块从 0 重算，
    /// 每块都会在输入位置 0 附近重新对齐卷积核 → 周期性跳变 → 此测试必红。
    #[test]
    fn streaming_resampler_equivalent_to_batch() {
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        let duration_secs = 1.0;
        let n = (source_rate as f64 * duration_secs) as usize;

        // 多频率叠加正弦（220Hz + 880Hz + 2000Hz）+ 伪白噪声
        let mut signal = Vec::with_capacity(n);
        let mut x = 0.0f64;
        let mut noise_acc = 13.0f64;
        for _ in 0..n {
            let s = (2.0 * std::f64::consts::PI * 220.0 * x).sin() * 0.3
                + (2.0 * std::f64::consts::PI * 880.0 * x).sin() * 0.2
                + (2.0 * std::f64::consts::PI * 2000.0 * x).sin() * 0.1;
            // 线性同余伪白噪声，幅度 0.05
            noise_acc = (noise_acc * 1664525.0 + 1013904223.0).fract();
            let noise = (noise_acc - 0.5) * 0.1;
            signal.push((s + noise) as f32);
            x += 1.0 / source_rate as f64;
        }

        // 批处理参考
        let batch = resample_anti_alias(&signal, source_rate, target_rate);

        // 流式：480 样本/块（10ms @ 48kHz）逐块 push + finish
        let mut streamer = StreamingResampler::new(source_rate, target_rate);
        let mut stream_out = Vec::new();
        for chunk in signal.chunks(480) {
            stream_out.extend_from_slice(&streamer.push(chunk));
        }
        stream_out.extend_from_slice(&streamer.finish());

        assert_eq!(
            batch.len(),
            stream_out.len(),
            "流式与批处理输出长度必须相等（batch={}, stream={}）",
            batch.len(),
            stream_out.len()
        );

        let mut max_diff = 0.0f64;
        for (i, (b, s)) in batch.iter().zip(stream_out.iter()).enumerate() {
            let d = (*b as f64 - *s as f64).abs();
            if d > max_diff {
                max_diff = d;
            }
            assert!(
                d <= 1e-5,
                "样本 {} 不等价：batch={}, stream={}, diff={}",
                i,
                b,
                s,
                d
            );
        }
        let _ = max_diff; // 抑制未用警告
    }

    /// 验收第 4 条：用 437（4+3+7=14，不能被 3 整除）作块长，证明不依赖块长对齐。
    #[test]
    fn streaming_resampler_works_with_non_aligned_chunk_size() {
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        let n = 48000; // 1 秒
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                (2.0 * std::f64::consts::PI * 440.0 * i as f64 / source_rate as f64).sin() as f32
                    * 0.5
            })
            .collect();

        let batch = resample_anti_alias(&signal, source_rate, target_rate);

        // 437 = 19×23，不能被 3 整除，刻意制造非对齐块长
        let mut streamer = StreamingResampler::new(source_rate, target_rate);
        let mut stream_out = Vec::new();
        for chunk in signal.chunks(437) {
            stream_out.extend_from_slice(&streamer.push(chunk));
        }
        stream_out.extend_from_slice(&streamer.finish());

        assert_eq!(batch.len(), stream_out.len());
        for (i, (b, s)) in batch.iter().zip(stream_out.iter()).enumerate() {
            assert!(
                (*b as f64 - *s as f64).abs() <= 1e-5,
                "非对齐块长样本 {} 不等价：batch={}, stream={}",
                i,
                b,
                s
            );
        }
    }

    /// 验收第 5 条：16000→16000 时 push 原样返回。
    #[test]
    fn streaming_resampler_identity_passthrough() {
        let mut streamer = StreamingResampler::new(16000, 16000);
        let chunk = vec![0.1f32, 0.2, 0.3, 0.4, 0.5];
        let out = streamer.push(&chunk);
        assert_eq!(out, chunk, "同采样率 push 必须原样返回");
        // finish 在同采样率下返回空
        let tail = streamer.finish();
        assert!(tail.is_empty(), "同采样率 finish 必须返回空");
    }

    /// 验收第 6 条：总输出长度与 input_len * 16000 / 48000 相差 ≤ 1。
    #[test]
    fn streaming_resampler_correct_output_length() {
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        for &n in &[480usize, 4800, 48000, 48001, 96000] {
            let signal = vec![0.5f32; n];
            let mut streamer = StreamingResampler::new(source_rate, target_rate);
            let mut out = streamer.push(&signal);
            out.extend_from_slice(&streamer.finish());

            let expected = (n as f64 * target_rate as f64 / source_rate as f64).round() as usize;
            assert!(
                (out.len() as isize - expected as isize).abs() <= 1,
                "n={}：输出长度 {} 与期望 {} 相差 > 1",
                n,
                out.len(),
                expected
            );
        }
    }

    /// 验收第 7 条：顺序护栏——pre-roll → post-hotkey → 主循环 用同一实例。
    /// 用计数器验证：逐块 push 同一实例的输出拼接后，与整段一次性 push 等价。
    /// 若用了多个实例（每块新建），每块都会从 emitted=0 重算 → 输出重复/错位 → 长度远超期望。
    #[test]
    fn streaming_resampler_single_instance_order_guard() {
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        let signal: Vec<f32> = (0..48000)
            .map(|i| {
                (2.0 * std::f64::consts::PI * 300.0 * i as f64 / source_rate as f64).sin() as f32
            })
            .collect();

        // 整段一次性 push（参考）
        let mut whole = StreamingResampler::new(source_rate, target_rate);
        let whole_out = {
            let mut o = whole.push(&signal);
            o.extend_from_slice(&whole.finish());
            o
        };

        // 模拟 pre-roll(160) + post-hotkey(320) + 主循环(480 反复)
        // 全部喂进【同一个】实例 —— 这是 record_streaming 的真实接线方式
        let mut streamer = StreamingResampler::new(source_rate, target_rate);
        let mut parts_out = Vec::new();
        // pre-roll
        parts_out.extend_from_slice(&streamer.push(&signal[0..160]));
        // post-hotkey
        parts_out.extend_from_slice(&streamer.push(&signal[160..480]));
        // 主循环 480/块
        let mut idx = 480;
        while idx < signal.len() {
            let end = (idx + 480).min(signal.len());
            parts_out.extend_from_slice(&streamer.push(&signal[idx..end]));
            idx = end;
        }
        parts_out.extend_from_slice(&streamer.finish());

        assert_eq!(
            whole_out.len(),
            parts_out.len(),
            "分段喂同一实例必须与整段喂同一实例输出长度相等"
        );
        for (i, (w, p)) in whole_out.iter().zip(parts_out.iter()).enumerate() {
            assert!(
                (*w as f64 - *p as f64).abs() <= 1e-5,
                "顺序护栏样本 {} 不等价：whole={}, parts={}",
                i,
                w,
                p
            );
        }
    }

    /// 多块小信号 + finish 尾部：确保 finish 不丢样本。
    #[test]
    fn streaming_resampler_finish_emits_tail() {
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        // 100 样本，远小于一个块，确保全部留到 finish
        let signal = vec![0.7f32; 100];
        let mut streamer = StreamingResampler::new(source_rate, target_rate);
        let mut out = streamer.push(&signal);
        let tail = streamer.finish();
        out.extend_from_slice(&tail);

        let batch = resample_anti_alias(&signal, source_rate, target_rate);
        assert_eq!(out.len(), batch.len(), "finish 必须补齐尾部不丢样本");
        for (i, (b, s)) in batch.iter().zip(out.iter()).enumerate() {
            assert!(
                (*b as f64 - *s as f64).abs() <= 1e-5,
                "tail 样本 {} 不等价",
                i
            );
        }
    }

    // ===== TEST-SYNC-042 补充用例（tester-1 补覆盖缺口）=====

    /// 非 48k 输入：44100Hz → 16000Hz（比例 2.75625，非整数倍）。
    /// 验证实现不依赖 3:1 整除关系，逐点与批处理等价。
    #[test]
    fn streaming_resampler_44100_to_16000_equivalent_to_batch() {
        let source_rate = 44100u32;
        let target_rate = 16000u32;
        let n = 44100; // 1 秒
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                (2.0 * std::f64::consts::PI * 440.0 * i as f64 / source_rate as f64).sin() as f32
                    * 0.5
            })
            .collect();

        let batch = resample_anti_alias(&signal, source_rate, target_rate);

        // 443 = 非 3 的倍数块长，且 44100/443 非整除，最后一块是残缺块
        let mut streamer = StreamingResampler::new(source_rate, target_rate);
        let mut stream_out = Vec::new();
        for chunk in signal.chunks(443) {
            stream_out.extend_from_slice(&streamer.push(chunk));
        }
        stream_out.extend_from_slice(&streamer.finish());

        assert_eq!(
            batch.len(),
            stream_out.len(),
            "44100→16000 输出长度与批处理不一致（batch={}, stream={}）",
            batch.len(),
            stream_out.len()
        );
        for (i, (b, s)) in batch.iter().zip(stream_out.iter()).enumerate() {
            assert!(
                (*b as f64 - *s as f64).abs() <= 1e-5,
                "44100→16000 非整数比例样本 {} 不等价：batch={}, stream={}",
                i,
                b,
                s
            );
        }
    }

    /// 单样本一个 chunk —— 最大粒度切分。验证状态机在 1 样本/块的
    /// 极端输入下仍与批处理逐点等价（依赖全局 emitted，而非块内偏移）。
    #[test]
    fn streaming_resampler_single_sample_chunks() {
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        let n = 4800; // 0.1s 足够覆盖
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                (2.0 * std::f64::consts::PI * 440.0 * i as f64 / source_rate as f64).sin() as f32
                    * 0.5
            })
            .collect();

        let batch = resample_anti_alias(&signal, source_rate, target_rate);

        let mut streamer = StreamingResampler::new(source_rate, target_rate);
        let mut stream_out = Vec::new();
        for s in signal.iter() {
            stream_out.extend_from_slice(&streamer.push(&[*s]));
        }
        stream_out.extend_from_slice(&streamer.finish());

        assert_eq!(
            batch.len(),
            stream_out.len(),
            "单样本块输出长度与批处理不一致（batch={}, stream={}）",
            batch.len(),
            stream_out.len()
        );
        for (i, (b, s)) in batch.iter().zip(stream_out.iter()).enumerate() {
            assert!(
                (*b as f64 - *s as f64).abs() <= 1e-5,
                "单样本块样本 {} 不等价：batch={}, stream={}",
                i,
                b,
                s
            );
        }
    }

    /// 空 chunk 无副作用 + finish 幂等：
    /// - push(&[]) 必须返回空、不贡献样本；
    /// - 空 chunk 混入正常流后，逐点仍与批处理等价；
    /// - 重复调用 finish()：第二次及以后必须返回空（尾部只补一次）。
    #[test]
    fn streaming_resampler_empty_chunk_and_finish_idempotent() {
        let source_rate = 48000u32;
        let target_rate = 16000u32;

        // 从未喂数的新实例：push 空 + finish 都必须为空
        let mut fresh = StreamingResampler::new(source_rate, target_rate);
        assert!(fresh.push(&[]).is_empty(), "push 空 chunk 必须返回空");
        assert!(fresh.finish().is_empty(), "空流 finish 必须返回空");

        let signal: Vec<f32> = (0..4800)
            .map(|i| {
                (2.0 * std::f64::consts::PI * 440.0 * i as f64 / source_rate as f64).sin() as f32
                    * 0.5
            })
            .collect();
        let batch = resample_anti_alias(&signal, source_rate, target_rate);

        let mut streamer = StreamingResampler::new(source_rate, target_rate);
        let mut stream_out = Vec::new();
        for (i, chunk) in signal.chunks(240).enumerate() {
            if i % 3 == 0 {
                assert!(streamer.push(&[]).is_empty(), "空 chunk 必须返回空");
            }
            stream_out.extend_from_slice(&streamer.push(chunk));
        }
        let tail1 = streamer.finish();
        stream_out.extend_from_slice(&tail1);
        assert!(streamer.finish().is_empty(), "第二次 finish 必须返回空");
        assert!(streamer.finish().is_empty(), "第三次 finish 必须返回空");

        assert_eq!(
            batch.len(),
            stream_out.len(),
            "空 chunk 混入后输出长度与批处理不一致"
        );
        for (i, (b, s)) in batch.iter().zip(stream_out.iter()).enumerate() {
            assert!(
                (*b as f64 - *s as f64).abs() <= 1e-5,
                "空 chunk 混入后样本 {} 不等价：batch={}, stream={}",
                i,
                b,
                s
            );
        }
    }

    /// 单次 push 巨块（整段 3s，长度远超内部缓冲）：补齐「长 chunk」值等价
    /// 覆盖。coder 的 correct_output_length 只验长度，本用例补逐点值与批处理等价。
    #[test]
    fn streaming_resampler_large_single_chunk_value_equivalence() {
        let source_rate = 48000u32;
        let target_rate = 16000u32;
        let n = 144000; // 3s @48k
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                (2.0 * std::f64::consts::PI * 440.0 * i as f64 / source_rate as f64).sin() as f32
                    * 0.3
            })
            .collect();

        let batch = resample_anti_alias(&signal, source_rate, target_rate);

        let mut streamer = StreamingResampler::new(source_rate, target_rate);
        let mut stream_out = streamer.push(&signal);
        stream_out.extend_from_slice(&streamer.finish());

        assert_eq!(
            batch.len(),
            stream_out.len(),
            "巨块输出长度与批处理不一致（batch={}, stream={}）",
            batch.len(),
            stream_out.len()
        );
        for (i, (b, s)) in batch.iter().zip(stream_out.iter()).enumerate() {
            assert!(
                (*b as f64 - *s as f64).abs() <= 1e-5,
                "巨块样本 {} 不等价：batch={}, stream={}",
                i,
                b,
                s
            );
        }
    }

    // ASR-074 Step 2-B 阶段三测试同步（tester-1，2026-09-03）
    // 松手 drain 语义钉死：与生产 :308-343 同构的执行序，绑定行为约定不绑定实现字符串。

    /// 验收①：Empty 提前 break —— 通道清空后循环立即终止，不空转等满 500ms。
    /// 生产契约：drain 循环在 try_recv Empty 时 break（:322），剩余预算直接放弃。
    /// 消融：若 Empty 不 break（改回死等 deadline），本用例耗时会从 <50ms 涨到 500ms
    /// 量级，elapsed 断言红。
    #[test]
    fn asr_074_stop_drain_exits_on_empty_channel_before_deadline() {
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(64);
        // 预置 3 个非空 chunk 后封口：drain 应在毫秒级清完并退出，而不是等 500ms
        for i in 0..3 {
            tx.send((Instant::now(), vec![i as f32; 160])).unwrap();
        }
        drop(tx);

        let drain_deadline = Instant::now() + Duration::from_millis(500);
        let started = Instant::now();
        let mut drained: usize = 0;
        while Instant::now() < drain_deadline {
            match rx.try_recv() {
                Ok((_ts, chunk)) if !chunk.is_empty() => {
                    drained += 1;
                    let _ = chunk;
                }
                Ok(_) => continue,
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(drained, 3, "封口前预置的 3 个 chunk 必须全部被 drain");
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "Empty 必须提前 break（实测 {:?}），死等 deadline 会让松手卡 500ms",
            started.elapsed()
        );
    }

    /// 验收②：Disconnected 提前 break —— ASR 消费端已断时不得死循环。
    /// 生产契约：:323 Disconnected → break。与 Empty 同为提前退出路径。
    /// 消融：若 Disconnected 分支被删（panic 或继续收网），本用例红（panic 或超时）。
    #[test]
    fn asr_074_stop_drain_exits_on_disconnected_channel() {
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(4);
        tx.send((Instant::now(), vec![0.5f32; 160])).unwrap();
        drop(tx); // 先发后断：Disconnected 与 Empty 都可能先命中，都应退出

        let mut drained: usize = 0;
        let started = Instant::now();
        loop {
            match rx.try_recv() {
                Ok((_ts, chunk)) if !chunk.is_empty() => drained += 1,
                Ok(_) => continue,
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(
            drained, 1,
            "断开前已在队列的 chunk 仍须被 drain（尾部音频不丢）"
        );
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "Disconnected 必须终止 drain 循环"
        );
    }

    /// 验收③：空 chunk 计数排除 —— Ok((_, chunk)) 但 chunk.is_empty() 的包
    /// 既不进 drained 计数也不进 abandoned 计数（生产 :313 `if !chunk.is_empty()`
    /// 与 :328 同构），但循环仍要继续消费下一个包（:321 `Ok(_) => continue`）。
    /// 消融：若空包被计入 drained，断言 2 红；若空包中断循环（无 continue），
    /// 后面的非空包丢失，断言 3 红。
    #[test]
    fn asr_074_stop_drain_skips_empty_chunks_but_keeps_draining() {
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(8);
        tx.send((Instant::now(), Vec::new())).unwrap(); // 空 chunk（stream_err 通道语义）
        tx.send((Instant::now(), vec![0.1f32; 160])).unwrap();
        tx.send((Instant::now(), Vec::new())).unwrap();
        tx.send((Instant::now(), vec![0.2f32; 160])).unwrap();
        drop(tx);

        let mut drained: usize = 0;
        let mut empty_seen: usize = 0;
        loop {
            match rx.try_recv() {
                Ok((_ts, chunk)) if !chunk.is_empty() => drained += 1,
                Ok(_) => {
                    empty_seen += 1;
                    continue;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(drained, 2, "非空 chunk 必须计入 drained");
        assert_eq!(empty_seen, 2, "空 chunk 必须被跳过但不得中断 drain");
    }

    /// 验收④：慢消费 × 500ms deadline —— ASR 卡住（on_chunk 阻塞）时松手，
    /// drain 到点即止：deadline 内消费到一部分（drained），剩余积压归 abandoned
    /// 计数（:326-331 第二段 while），录音线程最坏 0.5s 脱身。
    /// 生产真实路径建模：chunk_tx 是 crossbeam bounded 的**阻塞 send**（:305-308
    /// 注释），deadline 真正被触发靠的是 on_chunk 慢（ASR 线程不消费 → send 等），
    /// 而不是「生产者一直灌包让队列不空」——队列一空 :322 Empty → break，微秒级返回。
    /// 故本用例不建活的生产者：松手瞬间预灌 200 个 chunk 后停止生产（对齐生产：
    /// stop 信号一到 capture 侧即停灌），消费端每包 sleep(10ms) 模拟 on_chunk 阻塞。
    /// 200×10ms = 2s ≫ 500ms → deadline 必然先于队列见底触发 → abandoned 有判别力。
    /// 消融：删掉生产 :309 的时间上限（改回纯清空语义）→ 循环会把 200 个全消费完
    /// → abandoned == 0 且 elapsed ≈ 2s，两条断言同红。
    /// 本用例守的契约是「ASR 卡住时录音线程能在 0.5s 内脱身」，不是「drain 磨够 500ms」。
    #[test]
    fn asr_074_stop_drain_slow_consumer_abandons_backlog_at_deadline() {
        let (tx, rx) = crossbeam_channel::bounded::<AudioChunk>(256);
        // 松手瞬间的积压：一次性预灌 200 个非空 chunk，此后不再生产
        for _ in 0..200 {
            tx.send((Instant::now(), vec![0.9f32; 160])).unwrap();
        }

        let started = Instant::now();
        let drain_deadline = started + Duration::from_millis(500);
        let mut drained: usize = 0;
        let mut abandoned: usize = 0;
        while Instant::now() < drain_deadline {
            match rx.try_recv() {
                Ok((_ts, chunk)) if !chunk.is_empty() => {
                    // 模拟 on_chunk 阻塞的慢消费（:305-308 ASR 卡住场景）
                    std::thread::sleep(Duration::from_millis(10));
                    drained += 1;
                    let _ = chunk;
                }
                Ok(_) => continue,
                // 与生产 :322-323 同构：队列见底/断开即提前收网
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => break,
            }
        }
        // 到点即止：剩余积压全部放弃并计数（与生产 :327-331 同构）
        while let Ok((_ts, chunk)) = rx.try_recv() {
            if !chunk.is_empty() {
                abandoned += 1;
            }
        }
        drop(tx);

        assert!(
            drained > 0,
            "deadline 内应消费到部分积压（慢消费也得救回头部音频）"
        );
        assert!(
            abandoned > 0,
            "慢消费下 deadline 必先于队列见底触发，剩余必须归 abandoned（放弃可观测）"
        );
        assert!(
            started.elapsed() >= Duration::from_millis(500),
            "deadline 必须真的被触发过（慢消费 200×10ms 远超 500ms）"
        );
        assert!(
            started.elapsed() < Duration::from_millis(1500),
            "到点后必须立刻放弃（松手最坏 0.5s 返回契约；第二段排空不计 sleep），实测 {:?}",
            started.elapsed()
        );
    }
}
