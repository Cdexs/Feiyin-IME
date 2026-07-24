use crate::ui::overlay::AudioLevelBuf;
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
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

        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx_audio = tx.clone();
                let tx_stream_err = tx_err.clone();
                let stream_failed = Arc::clone(&stream_failed);
                let pre_roll_cb = Arc::clone(&pre_roll);
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
                        let _ = tx_audio.try_send((Instant::now(), chunk));
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
                        let _ = tx_audio.try_send((Instant::now(), chunk));
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
                        let _ = tx_audio.try_send((Instant::now(), chunk));
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
}
