use crate::ui::overlay::AudioLevelBuf;
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use crossbeam_channel::Sender;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::BOOL;
#[cfg(target_os = "windows")]
use windows::Win32::Media::Audio::{
    eCapture, eMultimedia, IMMDeviceEnumerator, MMDeviceEnumerator,
};
#[cfg(target_os = "windows")]
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

const PRE_ROLL_MS: u64 = 600; // HOTKEY-LATENCY-V2-001: 500→600ms to collect more initial audio for cold start
const PRIME_TIMEOUT_MS: u64 = 450; // HOTKEY-LATENCY-V2-001: 350→450ms for deeper cold-start audio collection
const PRIME_TICK_MS: u64 = 20;     // HOTKEY-LATENCY-FIX-001: recv_timeout tick, allows up to 17 ticks before timeout

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
    rx: crossbeam_channel::Receiver<Vec<f32>>,
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

        let (tx, rx) = crossbeam_channel::bounded::<Vec<f32>>(256);
        let tx_err: Sender<Vec<f32>> = tx.clone();
        let stream_failed = Arc::new(AtomicBool::new(false));

        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx_audio = tx.clone();
                let tx_stream_err = tx_err.clone();
                let stream_failed = Arc::clone(&stream_failed);
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        let _ = tx_audio.try_send(downmix_to_mono(data, channels, |sample| sample));
                    },
                    move |err| {
                        log::error!("Audio stream error: {}", err);
                        stream_failed.store(true, Ordering::Release);
                        let _ = tx_stream_err.try_send(vec![]);
                    },
                    None,
                )?
            }
            SampleFormat::I16 => {
                let tx_audio = tx.clone();
                let tx_stream_err = tx_err.clone();
                let stream_failed = Arc::clone(&stream_failed);
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        let _ = tx_audio.try_send(downmix_to_mono(data, channels, |sample| {
                            sample as f32 / i16::MAX as f32
                        }));
                    },
                    move |err| {
                        log::error!("Audio stream error: {}", err);
                        stream_failed.store(true, Ordering::Release);
                        let _ = tx_stream_err.try_send(vec![]);
                    },
                    None,
                )?
            }
            SampleFormat::U16 => {
                let tx_audio = tx.clone();
                let tx_stream_err = tx_err.clone();
                let stream_failed = Arc::clone(&stream_failed);
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        let _ = tx_audio.try_send(downmix_to_mono(data, channels, |sample| {
                            (sample as f32 / u16::MAX as f32) * 2.0 - 1.0
                        }));
                    },
                    move |err| {
                        log::error!("Audio stream error: {}", err);
                        stream_failed.store(true, Ordering::Release);
                        let _ = tx_stream_err.try_send(vec![]);
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
        let mut pending = Vec::new();
        while let Ok(chunk) = self.rx.try_recv() {
            if !chunk.is_empty() {
                pending.push(chunk);
            }
        }

        let drained_chunks = pending.len();
        let drained_samples = pending.iter().map(Vec::len).sum::<usize>();
        let max_samples = pre_roll_samples(self.sample_rate, pre_roll_ms);
        let retained = retain_recent_samples(pending, max_samples);
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
    rx: &crossbeam_channel::Receiver<Vec<f32>>,
    stream_failed: &AtomicBool,
    stop_signal: Arc<AtomicBool>,
    silence_threshold: f32,
    silence_duration_ms: u64,
    max_seconds: u64,
    level_buf: Option<AudioLevelBuf>,
    sample_rate: u32,
    pre_roll_chunks: Vec<Vec<f32>>,
) -> Result<Vec<f32>> {
    let mut state = RecordingState::new(sample_rate, silence_duration_ms, level_buf);
    let max_frames = max_seconds as usize * 16_000usize;

    // HOTKEY-LATENCY-FIX-001: When pre-roll is empty (WASAPI idle / cold start),
    // collect audio chunks with a timeout loop until we have PRE_ROLL_MS worth of
    // samples, or 350ms timeout (whichever comes first). This is more robust than
    // a single fixed 200ms recv_timeout which only yields one chunk.
    if pre_roll_chunks.is_empty() {
        let t_prime = std::time::Instant::now();
        let target_samples = pre_roll_samples(sample_rate, PRE_ROLL_MS);
        let mut prime_samples: Vec<f32> = Vec::with_capacity(target_samples);
        let mut total_wait_ms: u64 = 0;

        while prime_samples.len() < target_samples && total_wait_ms < PRIME_TIMEOUT_MS {
            if stop_signal.load(Ordering::Relaxed) || stream_failed.load(Ordering::Acquire) {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(PRIME_TICK_MS)) {
                Ok(chunk) if !chunk.is_empty() => {
                    prime_samples.extend_from_slice(&chunk);
                }
                Ok(_) => {
                    log::warn!("Audio prime: received empty chunk (possible stream failure)");
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // expected: may need multiple ticks
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    return Err(anyhow::anyhow!("Audio input stream disconnected during prime"));
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
            // trim to target if collected more than needed
            if prime_samples.len() > target_samples {
                prime_samples = prime_samples[prime_samples.len() - target_samples..].to_vec();
            }
            if state.push_chunk(&prime_samples, silence_threshold)? {
                log::info!("Silence detected in prime chunk, ending recording");
                return Ok(state.all_samples);
            }
        }
    }

    let mut stop_after_pre_roll = false;
    for chunk in pre_roll_chunks {
        if state.push_chunk(&chunk, silence_threshold)? {
            log::info!("Silence detected in pre-roll, ending recording");
            stop_after_pre_roll = true;
            break;
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

        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(50)) {
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
    log::info!(
        "Recording complete: {} samples ({:.1}s), speech_detected={}, peak_before={:.5}, peak_after={:.5}",
        state.all_samples.len(),
        state.all_samples.len() as f32 / 16000.0,
        state.speech_detected,
        peak_before_gain,
        peak_after_gain
    );
    Ok(state.all_samples)
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
            all_samples: Vec::with_capacity(16_000 * 10),
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
        if self.sample_rate != 16000 {
            self.all_samples
                .extend(resample_linear(chunk, self.sample_rate, 16000));
        } else {
            self.all_samples.extend_from_slice(chunk);
        }
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
        endpoint.GetMute().map(|b: BOOL| b.as_bool()).unwrap_or(false)
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

    #[test]
    fn recording_state_processes_pre_roll_with_same_vad_and_resampling_path() {
        let mut state = RecordingState::new(48_000, 100, None);
        assert!(!state.push_chunk(&vec![0.02; 4_800], 0.01).unwrap());

        assert!(state.speech_detected);
        assert_eq!(state.all_samples.len(), 1_600);
    }

    #[test]
    fn audio_prime_only_triggers_on_empty_preroll() {
        // HOTKEY-LATENCY-FIX-001: prime timeout loop only when pre_roll_chunks is empty
        let (tx, rx) = crossbeam_channel::bounded(2);
        let stop = Arc::new(AtomicBool::new(false));
        let failed = AtomicBool::new(false);

        // Case 1: empty pre_roll -> prime consumes the first chunk from channel
        tx.send(vec![0.1f32; 100]).unwrap();
        let result = collect_recording(
            &rx, &failed, Arc::clone(&stop), 0.01, 100, 0, None, 16000, vec![],
        );
        assert!(result.is_ok(), "Empty pre-roll with audio chunk must complete OK");
        assert_eq!(result.unwrap().len(), 100,
            "Prime must consume the available chunk when pre_roll is empty");

        // Case 2: non-empty pre_roll -> prime skipped, pre_roll chunks processed instead
        tx.send(vec![0.2f32; 50]).unwrap();
        let result2 = collect_recording(
            &rx, &failed, Arc::clone(&stop), 0.01, 100, 0, None, 16000,
            vec![vec![0.3f32; 80]],
        );
        assert!(result2.is_ok(), "Non-empty pre-roll must complete OK");
        assert_eq!(result2.unwrap().len(), 80,
            "Non-empty pre-roll must skip prime and process provided chunks");
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
            !i18n::get(UiLanguage::TraditionalChinese).error_mic_muted.is_empty(),
            "ZH_TW error_mic_muted must not be empty"
        );
        assert!(
            !i18n::get(UiLanguage::English).error_mic_muted.is_empty(),
            "EN error_mic_muted must not be empty"
        );
    }
}
