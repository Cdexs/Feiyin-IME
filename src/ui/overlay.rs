use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Peak level with decay for waveform spectrum display.
/// Each bar tracks both current RMS and a decaying peak value.
#[derive(Debug, Clone)]
pub struct PeakLevel {
    pub current: f32,   // Current RMS value (0.0–1.0)
    pub peak: f32,      // Peak value (decays over time)
}

impl PeakLevel {
    /// Update with new RMS sample. Peak rises instantly, decays slowly.
    pub fn update(&mut self, rms: f32, decay_rate: f32) {
        self.current = rms.clamp(0.0, 1.0);
        // Peak rises immediately if current is higher, otherwise decays
        if self.current > self.peak {
            self.peak = self.current;
        } else {
            self.peak = (self.peak * (1.0 - decay_rate)).max(self.current);
        }
    }

    /// Get the value to use for bar height (the peak).
    pub fn display_value(&self) -> f32 {
        self.peak
    }
}

/// Shared audio level buffer with peak tracking for waveform display.
pub type AudioLevelBuf = Arc<Mutex<VecDeque<PeakLevel>>>;

pub fn new_audio_level_buf() -> AudioLevelBuf {
    Arc::new(Mutex::new(VecDeque::with_capacity(64)))
}

pub fn clear_levels(buf: &AudioLevelBuf) {
    if let Ok(mut q) = buf.lock() {
        q.clear();
    }
}

pub fn warmup_levels(buf: &AudioLevelBuf) {
    if let Ok(mut q) = buf.lock() {
        q.clear();
        for _ in 0..8 {
            q.push_back(PeakLevel {
                current: 0.03,
                peak: 0.03,
            });
        }
    }
}

/// Push an RMS sample from the audio thread. Peak tracking happens at render time.
pub fn push_level(buf: &AudioLevelBuf, rms: f32) {
    if let Ok(mut q) = buf.lock() {
        if q.len() >= 64 {
            q.pop_front();
        }
        q.push_back(PeakLevel {
            current: rms.clamp(0.0, 1.0),
            peak: rms.clamp(0.0, 1.0),
        });
    }
}

/// Recording / pipeline status shared by the Win32 overlay implementation.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayStatus {
    Recording,
    /// Waveform gravity-fall transition before showing Processing overlay.
    FallingToProcessing { message: String },
    Processing(String),
    /// Focus was lost; show text preview with copy button.
    FocusLost {
        text: String,
        copied: bool,
    },
    /// Error occurred; show error message briefly.
    Error(String),
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_audio_level_buf_is_empty() {
        let buf = new_audio_level_buf();
        let q = buf.lock().unwrap();
        assert!(q.is_empty(), "new buffer must be empty");
    }

    #[test]
    fn clear_levels_empties_buffer() {
        let buf = new_audio_level_buf();
        push_level(&buf, 0.5);
        push_level(&buf, 0.7);
        assert_eq!(buf.lock().unwrap().len(), 2);

        clear_levels(&buf);
        assert!(buf.lock().unwrap().is_empty(), "clear_levels must empty the buffer");
    }

    #[test]
    fn push_level_appends_and_caps_at_64() {
        let buf = new_audio_level_buf();
        for i in 1..=70 {
            push_level(&buf, i as f32 / 100.0);
        }
        let q = buf.lock().unwrap();
        assert_eq!(q.len(), 64, "buffer must cap at 64 entries");
        // oldest items (1..=6) should have been evicted
        assert!(
            q.iter().all(|lvl| lvl.current >= 0.07),
            "oldest entries should have been popped, remaining start from 7th"
        );
    }

    #[test]
    fn push_level_clamps_current_and_peak() {
        let buf = new_audio_level_buf();
        push_level(&buf, -0.5); // negative
        push_level(&buf, 1.5);  // > 1.0
        let q = buf.lock().unwrap();
        assert_eq!(q[0].current, 0.0, "negative RMS must be clamped to 0.0");
        assert_eq!(q[1].current, 1.0, "RMS > 1.0 must be clamped to 1.0");
        assert_eq!(q[1].peak, 1.0, "peak must also be clamped to 1.0");
    }

    /// HOTKEY-LATENCY-V2-001: Warm-up the overlay audio level buffer with 8 low-level bars
    /// so the waveform display is immediately populated on hotkey start.
    #[test]
    fn warmup_levels_populates_8_low_level_entries() {
        let buf = new_audio_level_buf();
        warmup_levels(&buf);
        let q = buf.lock().unwrap();
        assert_eq!(q.len(), 8, "warmup_levels must produce exactly 8 entries");
        assert!(
            q.iter().all(|lvl| lvl.current == 0.03 && lvl.peak == 0.03),
            "all warmup entries must have current=peak=0.03"
        );
    }

    #[test]
    fn warmup_levels_clears_existing_entries() {
        let buf = new_audio_level_buf();
        push_level(&buf, 0.9);
        push_level(&buf, 0.8);
        assert_eq!(buf.lock().unwrap().len(), 2);

        warmup_levels(&buf);
        let q = buf.lock().unwrap();
        assert_eq!(q.len(), 8, "warmup_levels must replace existing entries with 8 warmup entries");
        assert!(
            q.iter().all(|lvl| lvl.current == 0.03),
            "warmup must overwrite old high-level values"
        );
    }

    #[test]
    fn peak_level_update_and_decay() {
        let mut peak = PeakLevel { current: 0.0, peak: 0.0 };

        peak.update(0.5, 0.1);
        assert_eq!(peak.current, 0.5);
        assert_eq!(peak.peak, 0.5);

        peak.update(0.3, 0.1);
        // peak decays: 0.5 * 0.9 = 0.45, clamped to 0.3
        assert_eq!(peak.current, 0.3);
        assert!((peak.peak - 0.45).abs() < 0.001, "peak must decay when current < peak");
    }

    #[test]
    fn peak_level_display_value() {
        let mut peak = PeakLevel { current: 0.2, peak: 0.6 };
        assert_eq!(peak.display_value(), 0.6, "display_value must return the peak value");

        peak.update(0.8, 0.1);
        assert_eq!(peak.display_value(), 0.8, "display_value must update to new higher peak");
    }
}
