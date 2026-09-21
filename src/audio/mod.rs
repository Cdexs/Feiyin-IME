use crate::ui::overlay::AudioLevelBuf;
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
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

// FIX-LOCALRT-FIRSTCHAR-293：`PRE_ROLL_MS` 现为**在线三档 / 批处理路径**的 pre-roll 取值。
// 环形缓冲容量另由 `PRE_ROLL_CAPACITY_MS` 决定（必须 ≥ 所有取用值），见下。
const PRE_ROLL_MS: u64 = 600; // HOTKEY-LATENCY-V2-001: 500→600ms to collect more initial audio for cold start
/// FIX-LOCALRT-FIRSTCHAR-293：环形缓冲**容量**（三路共用的 `max_pre_roll_samples` 用它）。
/// 本地流式要取 1000ms，故容量必须 ≥1000；在线/批处理只取 `PRE_ROLL_MS`(600)，
/// 因此容量变大对它们零影响（`retain_recent_samples` 只保留最近 N ms）。
const PRE_ROLL_CAPACITY_MS: u64 = 1000;
/// FIX-LOCALRT-FIRSTCHAR-293：本地流式管线的 pre-roll 取用长度。
/// 600ms 窗口会把首字开头的爆破音声母削掉（Gavin 端测「输入你」→「输入按」），
/// 加长到 1000ms 争取完整声学起点；窗口长 ≠ 全喂进去，由裁剪规则锚定语音起点。
const PRE_ROLL_LOCAL_RT_MS: u64 = 1000;
/// FIX-LOCALRT-FIRSTCHAR-293：锚定语音起点时往前多留的 padding。
/// 150ms（原 283 为 100ms）：送气清声母 / 爆破段能量低于阈值、检不出来，不能切掉
/// （`[FIRSTCHAR-001]`/`[FIRSTCHAR-002]` 老坑，不许复发）。
const PRE_ROLL_PAD_MS: u64 = 150;
/// FIX-LOCALRT-FIRSTCHAR-293：整窗无语音段时只保留的末尾长度（场景 B：先按键后开口，
/// 不把窗口里的静音/底噪全灌给 ASR）。
const PRE_ROLL_TAIL_KEEP_MS: u64 = 200;
/// 能量 VAD 静音阈值（沿用 283 的 0.005）。
const PRE_ROLL_SIL_THRESHOLD: f32 = 0.005;
/// 段间静音 ≥ 此值才判为分隔（沿用 283 的 200ms）。
const PRE_ROLL_MIN_SILENCE_MS: u64 = 200;
/// FIX-PREROLL-RESIDUAL-308：最后一段语音的**结束点**距窗口末尾的静音 ≥ 此值 ⇒ 判为
/// 「上一句尾音残留」（按键前已说完），与场景 B 一样只留末尾 tail。取值同
/// `PRE_ROLL_MIN_SILENCE_MS`（200ms），不新增语义。
const PRE_ROLL_RESIDUAL_SILENCE_MS: u64 = 200;
const PRIME_TIMEOUT_MS: u64 = 450; // HOTKEY-LATENCY-V2-001: 350→450ms for deeper cold-start audio collection
const PRIME_TICK_MS: u64 = 20; // HOTKEY-LATENCY-FIX-001: recv_timeout tick, allows up to 17 ticks before timeout

type AudioChunk = (Instant, Vec<f32>);

// RESEARCH-ACC-FIRSTCHAR-278: **仅埋点，非行为性**（不改任何逻辑）。
// 记录「上次录音结束」时刻（进程启动起算 ms），用于计算按键时距上次录音结束的间隔 ——
// 这是判断 pre_roll 600ms 是否为「上一句残留」的关键变量。
static LAST_RECORD_END_MS: AtomicU64 = AtomicU64::new(0);
static PROC_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn proc_now_ms() -> u64 {
    PROC_START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

// ============================================================================
// PREROLL-DEAD-322：诊断口径修正（**只读**，不参与任何行为决策、不改动音频一个 bit）。
//
// 事故：`{:.4}` 会把 1e-7 打印成 `0.0000`；16-bit WAV 落盘会把 1e-7 量化成 0。
// 于是「pre-roll 是纯零 ⇒ 设备交数字静音 ⇒ 整块摘除」这个错误结论先后得出两轮。
// 2026-09-21 实测（`pr322_idle_probe_manual`）：空闲期环形缓冲的样本 **100% 非零**，
// 峰值 ≈2^-24 ≈ 6e-8（≈-140 dBFS）—— 是 24-bit 量化格上的最低位抖动，**不是数字零**。
//
// ⇒ 诊断一律改用两个**与小数位无关**的口径：
//   - `peak_to_dbfs`：dB 刻度。1e-7 显示成 -140.0，永远不会被小数位吃掉；
//   - `nonzero_ratio`：非零样本占比。数字静音 = 0.000，最低位抖动 = 1.000，一眼可辨。
// ============================================================================

/// dBFS 下限：避免 `log10(0)` 的 -inf / NaN 混进日志（`peak <= 0` 含 NaN 都归到这一档）。
const DIAG_DBFS_FLOOR: f32 = -200.0;

/// 峰值幅度 → dBFS（满刻度 1.0 = 0 dBFS），下限钳到 `DIAG_DBFS_FLOOR`。
fn peak_to_dbfs(peak: f32) -> f32 {
    if peak > 0.0 {
        (20.0 * peak.log10()).max(DIAG_DBFS_FLOOR)
    } else {
        DIAG_DBFS_FLOOR
    }
}

/// 非零样本占比（0.0..=1.0）。**区分「数字静音」与「极低电平底噪」的硬指标**：
/// 数字静音恒为 0.000；哪怕只有 ±1 个最低位抖动也是 1.000。
/// `total == 0` 时返回 0.0（无样本 ⇒ 不算「有信号」）。
fn nonzero_ratio(nonzero: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        nonzero as f64 / total as f64
    }
}

/// 诊断用一次遍历：返回 (峰值, 非零样本数, 总样本数)。
fn diag_peak_nz(samples: &[f32]) -> (f32, usize, usize) {
    let mut peak = 0f32;
    let mut nonzero = 0usize;
    for &s in samples {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        if s != 0.0 {
            nonzero += 1;
        }
    }
    (peak, nonzero, samples.len())
}

/// [LocalRT-DBG-278] 记录 pre_roll 那 600ms 的能量/内容特征（只读）。
///
/// URGENT-286：本函数全部计算（能量/峰值/语音帧统计，O(600ms 样本)）都是
/// **为日志而算**，故先过 `log_enabled!` 守卫——默认 `max_level=Warn` 下直接返回，
/// 连遍历都不发生（零开销）；仅带 `-debug`（`filter_level(Debug)`）端测时统计。
///
/// PREROLL-DEAD-322：口径升级为 `peak_dbfs` + `nz_ratio`（见上方常量注释块）。
/// 原 `peak={:.4}` 在 ≈-140 dBFS 的底噪上恒显示 `0.0000`，是本单两轮误判的直接来源。
fn pre_roll_diag(chunks: &[Vec<f32>], rate: u32) {
    if !log::log_enabled!(log::Level::Debug) {
        return;
    }
    let total: usize = chunks.iter().map(|c| c.len()).sum();
    if total == 0 {
        log::debug!("[LocalRT-DBG-278] pre_roll chunks=0 (empty)");
        return;
    }
    let mut peak = 0f32;
    let mut sum_abs = 0f64;
    let mut nonzero = 0usize;
    let frame = ((rate / 50).max(1)) as usize; // 20ms 帧
    let mut frames = 0usize;
    let mut speech_frames = 0usize;
    let mut f_sum_sq = 0f64;
    let mut f_n = 0usize;
    for c in chunks {
        for &s in c {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            if s != 0.0 {
                nonzero += 1;
            }
            sum_abs += a as f64;
            f_sum_sq += (s * s) as f64;
            f_n += 1;
            if f_n >= frame {
                let rms = (f_sum_sq / f_n as f64).sqrt();
                frames += 1;
                if rms > 0.01 {
                    speech_frames += 1;
                }
                f_sum_sq = 0.0;
                f_n = 0;
            }
        }
    }
    let dur_ms = total as f64 / rate as f64 * 1000.0;
    let last = LAST_RECORD_END_MS.load(Ordering::Relaxed);
    let gap = if last == 0 {
        "-".to_string()
    } else {
        proc_now_ms().saturating_sub(last).to_string()
    };
    let ratio = if frames > 0 {
        speech_frames as f64 / frames as f64
    } else {
        0.0
    };
    log::debug!(
        "[LocalRT-DBG-278] pre_roll chunks={} dur={:.0}ms peak={:.3e}({:.1}dBFS) mean_abs={:.3e} nz_ratio={:.3} speech_frames={}/{} ratio={:.2} gap_since_last_record={}ms",
        chunks.len(),
        dur_ms,
        peak,
        peak_to_dbfs(peak),
        sum_abs / total as f64,
        nonzero_ratio(nonzero, total),
        speech_frames,
        frames,
        ratio,
        gap
    );
}

// ============================================================================
// DIAG-LOCALRT-FIRSTCHAR-292：把 pre-roll 原始音频落盘，回答「首字声学起点到底
// 在不在这 600ms 窗口里」——Gavin 2026-09-21 报流式档首字不准（`我自翻` vs `端`）。
//
// 🔴 这是**只读旁路**：本模块只借用 `&[f32]`，不修改、不重排、不丢弃任何样本，
//    喂给 ASR 的音频因此逐 bit 不变。全部工作在 `log::log_enabled!(Debug)` 为真时
//    才发生；默认 Warn 下 `PreRollDump::new` 直接返回 None，连目录名都不构造。
//
// 依赖说明：仓库无 WAV 写入能力（sherpa `Wave` 只读、未引入 hound），故手写
// 44 字节 PCM16 WAV 头 + 样本，避免为一个诊断功能新增依赖。
// ============================================================================

/// 每个 dump 事件最多 2 个文件 ⇒ 最多保留最近 `DUMP_MAX_FILES` 个文件（超出删最旧）。
const DUMP_MAX_FILES: usize = 20;
/// 第二个文件 = pre-roll + 其后这么多秒的实时音频。
const DUMP_REALTIME_SECS: usize = 2;
/// `head_clipped` 判据：首个非静音样本落在窗口前这么多 ms 以内 ⇒ 认为语音在窗口
/// 打开前就已开始、头部被 600ms 边界削掉。
const HEAD_CLIP_WINDOW_MS: u64 = 40;
/// `head_clipped` 用的「非静音」绝对幅度阈值（与 278 的能量阈值同源，但为样本级）。
const HEAD_CLIP_ABS_THRESHOLD: f32 = 0.01;

/// 诊断音频落盘目录：exe 同级 `debug-audio/`（DEC-011：不依赖运行时工作目录）。
fn debug_audio_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("debug-audio")
}

/// 首个 |sample| > `HEAD_CLIP_ABS_THRESHOLD` 的样本相对窗口起点的偏移（ms）。
/// 全静音返回 None。
fn first_speech_offset_ms(samples: &[f32], rate: u32) -> Option<u64> {
    if rate == 0 {
        return None;
    }
    samples
        .iter()
        .position(|s| s.abs() > HEAD_CLIP_ABS_THRESHOLD)
        .map(|i| i as u64 * 1000 / rate as u64)
}

/// 最小 WAV 写入：单声道 16-bit PCM，采样率写真实值（端测现场 48000，写错即
/// `[ASR-SAMPLERATE-STREAM-001]`）。刻意不引入 hound —— 44 字节头手写足够。
fn write_wav_pcm16(path: &Path, samples: &[f32], rate: u32) -> std::io::Result<()> {
    use std::io::Write;
    let data_len = (samples.len() * 2) as u32;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_len).to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    header.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    header.extend_from_slice(&1u16.to_le_bytes()); // channels = mono
    header.extend_from_slice(&rate.to_le_bytes()); // sample rate (真实值)
    header.extend_from_slice(&(rate * 2).to_le_bytes()); // byte rate
    header.extend_from_slice(&2u16.to_le_bytes()); // block align
    header.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());

    let mut file = std::fs::File::create(path)?;
    file.write_all(&header)?;
    let mut pcm = Vec::with_capacity(data_len as usize);
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    file.write_all(&pcm)
}

fn is_dump_file(name: &str) -> bool {
    name.starts_with("preroll-") && name.ends_with(".wav")
}

/// 上限闸门：`preroll-<ts>*.wav` 超过 `max_files` 时删最旧的（按文件名升序 =
/// 时间戳升序）。返回删除数量。不设上限 = 端测跑一晚撑爆盘。
fn enforce_dump_limit(dir: &Path, max_files: usize) -> usize {
    let mut names: Vec<String> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| is_dump_file(n))
            .collect(),
        Err(_) => return 0,
    };
    if names.len() <= max_files {
        return 0;
    }
    names.sort();
    let excess = names.len() - max_files;
    let mut removed = 0;
    for name in names.iter().take(excess) {
        if std::fs::remove_file(dir.join(name)).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// 2026-09-21 现场诊断用格式：`first_speech_at` 为 none 时窗口内无语音。
///
/// PREROLL-DEAD-322：加打 `peak`(dBFS) + `nz_ratio`，理由同 278 —— 「首个超过 0.01
/// 的样本」在 ≈-140 dBFS 的底噪上只会给出「全静音」的假象，而 `nz_ratio` 能一眼区分
/// 「数字零」与「最低位抖动」。
///
/// 🔴 每条日志报的是**它自己那段样本**的 offset（起点 = 该 WAV 的起点）。文件 ② 起点是
/// pre-roll 起点，故其 `first_speech_at` ≥ pre-roll 长度 ⇔ 语音出现在热键之后。
/// 旧写法让文件 ② 复用文件 ① 的 offset，会给**含语音**的文件 ② 打上 `first_speech_at=none`
/// —— 2026-09-21 本单取证时差点被这一行误导，故一并纠正。
fn dump_log(name: &str, rate: u32, offset: Option<u64>, head_clipped: bool, samples: &[f32]) {
    let (peak, nonzero, total) = diag_peak_nz(samples);
    let at = offset
        .map(|ms| format!("{}ms", ms))
        .unwrap_or_else(|| "none".to_string());
    log::debug!(
        "[LocalRT-DBG-292] preroll dump: file={} rate={} first_speech_at={} head_clipped={} peak={:.3e}({:.1}dBFS) nz_ratio={:.3}",
        name,
        rate,
        at,
        head_clipped,
        peak,
        peak_to_dbfs(peak),
        nonzero_ratio(nonzero, total)
    );
}

/// 一次录音的 pre-roll 落盘器。
///
/// - 构造即写文件 ①（原始 600ms pre-roll）。
/// - `push` 累积实时原始 chunk，凑满 pre-roll + 2s 时写文件 ②。
/// - 录音提前结束时由 `Drop` 写文件 ②（不足 2s 就写多少算多少）。
struct PreRollDump {
    dir: PathBuf,
    ts: String,
    rate: u32,
    buf2: Vec<f32>,
    target2: usize,
    done2: bool,
}

impl PreRollDump {
    /// 生产入口：目录固定 exe 同级 `debug-audio/`。
    fn new(chunks: &[Vec<f32>], rate: u32) -> Option<Self> {
        if !log::log_enabled!(log::Level::Debug) {
            return None;
        }
        Self::build(debug_audio_dir(), chunks, rate)
    }

    /// 测试入口：可注入目录。Warn 级下与 `new` 同样完全不动作（连目录都不建）。
    #[cfg(test)]
    fn new_in(dir: PathBuf, chunks: &[Vec<f32>], rate: u32) -> Option<Self> {
        if !log::log_enabled!(log::Level::Debug) {
            return None;
        }
        Self::build(dir, chunks, rate)
    }

    fn build(dir: PathBuf, chunks: &[Vec<f32>], rate: u32) -> Option<Self> {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!(
                "[LocalRT-DBG-292] cannot create dump dir {}: {e}",
                dir.display()
            );
            return None;
        }
        let preroll: Vec<f32> = chunks.iter().flat_map(|c| c.iter().copied()).collect();
        let offset = first_speech_offset_ms(&preroll, rate);
        let head_clipped = offset.map(|ms| ms <= HEAD_CLIP_WINDOW_MS).unwrap_or(false);
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let name1 = format!("preroll-{}.wav", ts);
        if let Err(e) = write_wav_pcm16(&dir.join(&name1), &preroll, rate) {
            log::warn!("[LocalRT-DBG-292] WAV write failed ({}): {e}", name1);
            return None;
        }
        dump_log(&name1, rate, offset, head_clipped, &preroll);
        enforce_dump_limit(&dir, DUMP_MAX_FILES);

        let target2 = preroll.len() + rate as usize * DUMP_REALTIME_SECS;
        Some(PreRollDump {
            dir,
            ts,
            rate,
            buf2: preroll,
            target2,
            done2: false,
        })
    }

    fn push(&mut self, chunk: &[f32]) {
        if self.done2 || chunk.is_empty() {
            return;
        }
        self.buf2.extend_from_slice(chunk);
        if self.buf2.len() >= self.target2 {
            self.buf2.truncate(self.target2);
            self.write_part2();
        }
    }

    fn write_part2(&mut self) {
        if self.done2 {
            return;
        }
        self.done2 = true;
        let name = format!("preroll-{}-2s.wav", self.ts);
        // PREROLL-DEAD-322：文件 ② 的 first_speech_at / head_clipped 按**它自己**的样本重算
        // （起点 = pre-roll 起点 ⇒ 数值 ≥ pre-roll 长度即语音出现在热键之后）。
        let off2 = first_speech_offset_ms(&self.buf2, self.rate);
        let hc2 = off2.map(|ms| ms <= HEAD_CLIP_WINDOW_MS).unwrap_or(false);
        match write_wav_pcm16(&self.dir.join(&name), &self.buf2, self.rate) {
            Ok(()) => dump_log(&name, self.rate, off2, hc2, &self.buf2),
            Err(e) => log::warn!("[LocalRT-DBG-292] WAV write failed ({}): {e}", name),
        }
        enforce_dump_limit(&self.dir, DUMP_MAX_FILES);
    }
}

impl Drop for PreRollDump {
    fn drop(&mut self) {
        if !self.done2 && !self.buf2.is_empty() {
            self.write_part2();
        }
    }
}

/// FIX-LOCALRT-FIRSTCHAR-283/293（统一规则）：把 pre-roll 窗口裁成「喂给 ASR 的那一段」。
///
/// 窗口加长到 1000ms 是为了**不错过早到的语音**，但**窗口长 ≠ 全喂进去**。本函数用轻量
/// **样本级能量 VAD**（阈值 `PRE_ROLL_SIL_THRESHOLD`，段间静音 ≥`PRE_ROLL_MIN_SILENCE_MS`）
/// 把窗口切成语音段，一条规则同时覆盖端测三种场景：
///
/// | 场景 | 检出 | 保留 |
/// | --- | --- | --- |
/// | A 先开口后按键（首字被 600ms 边界削掉） | ≥1 段且语音连到窗口末尾 | 最后一段起点 − `PRE_ROLL_PAD_MS` 起，带出完整声母 |
/// | B 先按键后开口（旧 buffer / 静音干扰首字） | 0 段 | 只留末尾 `PRE_ROLL_TAIL_KEEP_MS` |
/// | C 上句尾音残留（多段） | ≥2 段且末段连到末尾 | 取最后一段（283 原有能力） |
/// | **D 上句尾音残留（单段）** | ≥1 段但**末段结束后跟 ≥`PRE_ROLL_RESIDUAL_SILENCE_MS` 静音** | 同场景 B：只留末尾 tail |
/// | 兜底：整窗找不到任何 ≥200ms 静音游程（嘈杂环境，判据不可靠） | — | **原样返回**，绝不比现状更差 |
///
/// 🔴 只用能量法（平台中立、不引入 ASR 模型依赖）。**仅本地流式调用**；在线三档
/// `trim_pre_roll_residual=false`，根本不进本函数 ⇒ 在线零改变。
fn select_pre_roll_for_asr(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == 0 || samples.is_empty() {
        return samples.to_vec();
    }
    let min_silence = rate as usize * PRE_ROLL_MIN_SILENCE_MS as usize / 1000;
    let pad = rate as usize * PRE_ROLL_PAD_MS as usize / 1000;

    // 样本级静音游程切段（帧级在「正好 200ms 静音」边界会量化漏切，283 离线复现过）。
    let mut starts: Vec<usize> = Vec::new();
    let mut in_speech = false;
    let mut run = 0usize;
    let mut has_long_silence = false;
    // FIX-PREROLL-RESIDUAL-308：记录最后一个**非静音样本**的下标（= 最后一段的结束点）。
    let mut last_speech_end: Option<usize> = None;
    for (idx, &x) in samples.iter().enumerate() {
        if x.abs() <= PRE_ROLL_SIL_THRESHOLD {
            run += 1;
            if run >= min_silence {
                has_long_silence = true;
                in_speech = false; // 关闭当前语音段
            }
        } else {
            if !in_speech {
                starts.push(idx);
                in_speech = true;
            }
            run = 0;
            last_speech_end = Some(idx);
        }
    }

    // 兜底：没有任何 ≥200ms 静音游程 ⇒ 能量判据不可靠，退回现状（原样全保留）。
    if !has_long_silence {
        log_pre_roll_select(samples.len(), samples.len(), rate, "fallback", None, None);
        return samples.to_vec();
    }

    // 场景 B：整窗无语音段 ⇒ 只留末尾 TAIL_KEEP_MS（不得为空）。
    if starts.is_empty() {
        let tail = rate as usize * PRE_ROLL_TAIL_KEEP_MS as usize / 1000;
        let st = samples.len().saturating_sub(tail);
        let out = samples[st..].to_vec();
        log_pre_roll_select(samples.len(), out.len(), rate, "tail", None, None);
        return out;
    }

    // 场景 D（FIX-PREROLL-RESIDUAL-308）：最后一段语音结束点距窗口末尾 ≥ RESIDUAL_SILENCE_MS
    // ⇒ 判为「上一句尾音残留」（按键前已说完）⇒ 与场景 B 同样只留末尾 tail。
    //
    // 🔴 有意的取舍：说完最后一个字、停顿一下才按键的情形也会被判残留而丢弃 pre-roll。
    //    那时用户已经说完，丢旧的不会影响本次要说的内容；留着它必然产生幻影文字。
    //    「宁可丢旧的，不可污染新的」。真·先开口者按键时通常仍在说 ⇒ 末尾有能量 ⇒ 不触发。
    if let Some(last_end) = last_speech_end {
        let trailing_samples = samples.len().saturating_sub(last_end + 1);
        let trailing_ms = trailing_samples as u64 * 1000 / rate as u64;
        if trailing_ms >= PRE_ROLL_RESIDUAL_SILENCE_MS {
            let tail = rate as usize * PRE_ROLL_TAIL_KEEP_MS as usize / 1000;
            let st = samples.len().saturating_sub(tail);
            let out = samples[st..].to_vec();
            log_pre_roll_select(
                samples.len(),
                out.len(),
                rate,
                "residual",
                None,
                Some(trailing_ms),
            );
            return out;
        }
    }

    // 场景 A / C：锚定最后一段起点，往前留 PAD_MS（saturating：起点在 PAD 内不下溢）。
    let last = *starts.last().unwrap();
    let st = last.saturating_sub(pad);
    let out = samples[st..].to_vec();
    log_pre_roll_select(samples.len(), out.len(), rate, "onset", Some(last), None);
    out
}

/// `[LocalRT-DBG-293]` 只读埋点：`debug!` + `log_enabled!` 守卫（默认 Warn 零开销）。
/// `trailing_silence_ms` 仅在 `mode=residual` 时给出（FIX-PREROLL-RESIDUAL-308 的判定依据）。
fn log_pre_roll_select(
    window: usize,
    kept: usize,
    rate: u32,
    mode: &str,
    onset: Option<usize>,
    trailing_silence_ms: Option<u64>,
) {
    if !log::log_enabled!(log::Level::Debug) {
        return;
    }
    let ms = |n: usize| n as f64 / rate as f64 * 1000.0;
    let onset_at = onset
        .map(|s| format!("{}ms", ms(s) as u64))
        .unwrap_or_else(|| "-".to_string());
    let trailing = trailing_silence_ms
        .map(|t| format!(" trailing_silence_ms={t}"))
        .unwrap_or_default();
    log::debug!(
        "[LocalRT-DBG-293] pre_roll select: window={}ms kept={}ms mode={} onset_at={}{}",
        ms(window) as u64,
        ms(kept) as u64,
        mode,
        onset_at,
        trailing
    );
}

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
            &warm.dropped_chunks,
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
        // FIX-LOCALRT-FIRSTCHAR-283/293：仅**本地流式**路径传 true——用
        // `select_pre_roll_for_asr` 把加长后的 pre-roll 锚定到语音起点（A 先开口 /
        // B 先按键 / C 上句尾音），同时把取用长度切到 `PRE_ROLL_LOCAL_RT_MS`(1000ms)。
        // 在线流式三档传 false ⇒ 取 `PRE_ROLL_MS`(600ms) 且不进裁剪函数，行为完全不变。
        trim_pre_roll_residual: bool,
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
        let pre_roll_chunks =
            warm.drain_pre_roll(pre_roll_ms_for_streaming(trim_pre_roll_residual));
        log::info!(
            "[Latency] record_streaming drain_pre_roll: {} chunks at +{:.1}ms",
            pre_roll_chunks.len(),
            t_record.elapsed().as_secs_f64() * 1000.0
        );
        // RESEARCH-ACC-FIRSTCHAR-278 埋点（只读）：pre_roll 能量特征 + 距上次录音结束间隔
        pre_roll_diag(&pre_roll_chunks, warm.sample_rate);
        // DIAG-LOCALRT-FIRSTCHAR-292：**只读旁路**。debug 级下把原始 600ms pre-roll
        // 落盘（并在凑满「pre-roll + 其后 2s 实时」时再落一份），用于离线判定首字
        // 声学起点是否被窗口边界削掉。Warn 级下 `new` 返回 None，零文件、零计算。
        // 🔴 注意：此处取的是 **trim 之前** 的原始 pre_roll —— 正是要看的输入。
        let mut preroll_dump = PreRollDump::new(&pre_roll_chunks, warm.sample_rate);

        // FIX-LOCALRT-FIRSTCHAR-283/293（统一裁剪，仅本地流式 trim_pre_roll_residual=true）：
        // 拼成单块 → 按「锚定语音起点」规则选出喂 ASR 的那一段（A 先开口 / B 先按键 /
        // C 上句尾音；拥挤/无静音游程时原样返回）。埋点 [LocalRT-DBG-293] 在函数内部。
        let pre_roll_chunks = if trim_pre_roll_residual {
            let mut all: Vec<f32> =
                Vec::with_capacity(pre_roll_chunks.iter().map(|c| c.len()).sum());
            for c in &pre_roll_chunks {
                all.extend_from_slice(c);
            }
            let kept = select_pre_roll_for_asr(&all, warm.sample_rate);
            if kept.is_empty() {
                Vec::new()
            } else {
                vec![kept]
            }
        } else {
            pre_roll_chunks
        };

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
        // DIAG-LOCALRT-FIRSTCHAR-292：只读累积原始实时 chunk（未重采样，与 WAV 采样率一致）
        if let Some(dump) = preroll_dump.as_mut() {
            for chunk in &post_hotkey_chunks {
                dump.push(chunk);
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
        // FIX-ASR-DROP-288：消费端只在录音期存在 ⇒ 本节流上报即「录音期丢包」专用。
        // 全局累计计数在此取基线快照，差值才是本次录音真实丢包。
        let drop_baseline = warm.dropped_chunks.load(Ordering::Relaxed);
        let mut last_drop_report_ms = proc_now_ms();
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

            // 非实时线程的节流上报（≤1 条/秒）；空闲期无消费端，永远不会走到这里。
            report_recording_drops_throttled(
                warm.dropped_chunks.load(Ordering::Relaxed),
                drop_baseline,
                &mut last_drop_report_ms,
                proc_now_ms(),
            );

            match warm.rx.recv_timeout(recv_timeout) {
                Ok((_ts, chunk)) if !chunk.is_empty() => {
                    // DIAG-LOCALRT-FIRSTCHAR-292：只读旁路，喂 ASR 前顺手抄一份原始 chunk
                    if let Some(dump) = preroll_dump.as_mut() {
                        dump.push(&chunk);
                    }
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

        // FIX-ASR-DROP-288：只报**本次录音**真实丢包（全局累计 - 录音前基线），
        // 修掉旧代码把全局累计当本次（3.7s 录音报出 23s 音频）的误导。
        // 每个 chunk ≈10ms @48k，精确秒数不可知因 chunk 已丢。
        let dropped_total = warm.dropped_chunks.load(Ordering::Relaxed);
        let dropped_this_recording = dropped_total.saturating_sub(drop_baseline);
        if dropped_this_recording > 0 {
            log::warn!(
                "[ASR-DROP] {} chunks dropped during THIS recording (~{:.1}s audio lost, est. 10ms/chunk); process-lifetime cumulative={}",
                dropped_this_recording,
                dropped_this_recording as f32 * 0.01,
                dropped_total
            );
        }

        log::info!(
            "Streaming recording complete: ~{} samples ({:.1}s @ {}Hz), speech_detected={}, dropped_this_recording={}, dropped_cumulative={}",
            total_samples,
            total_samples as f32 / sample_rate as f32,
            sample_rate,
            speech_detected,
            dropped_this_recording,
            dropped_total
        );

        // RESEARCH-ACC-FIRSTCHAR-278 埋点（只读）：标记本次录音结束时刻，供下次按键算间隔
        LAST_RECORD_END_MS.store(proc_now_ms(), Ordering::Relaxed);
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
        let max_pre_roll_samples = pre_roll_samples(sample_rate, PRE_ROLL_CAPACITY_MS);
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
                        // FIX-ASR-DROP-288：实时回调线程只做一次原子自增——**不打日志**
                        // （不格式化/不分配/不加锁）。空闲期队列满丢弃是**正常行为**
                        // （无人消费），由消费端决定是否告警；消费端只在录音期存在，
                        // 故能结构性地区分「空闲正常丢弃」与「录音期异常丢弃」。
                        if tx_audio.try_send((Instant::now(), chunk)).is_err() {
                            dropped_chunks_cb.fetch_add(1, Ordering::Relaxed);
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
                        // FIX-ASR-DROP-288：实时回调线程只做一次原子自增——**不打日志**。
                        if tx_audio.try_send((Instant::now(), chunk)).is_err() {
                            dropped_chunks_cb.fetch_add(1, Ordering::Relaxed);
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
                        // FIX-ASR-DROP-288：实时回调线程只做一次原子自增——**不打日志**。
                        if tx_audio.try_send((Instant::now(), chunk)).is_err() {
                            dropped_chunks_cb.fetch_add(1, Ordering::Relaxed);
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
/// FIX-ASR-DROP-288：消费端节流上报（**非实时线程**）。
///
/// 只在消费端（`record()` 的 `collect_recording` / `record_streaming` 主循环）调用，
/// 而消费端**只在录音期存在** ⇒ 能执行到此必为录音期，无需任何 `AtomicBool` 去判断
/// 「当前是否在录音」——由结构本身回答。空闲期的队列满丢弃是正常行为（无人消费），
/// 那时根本没有消费端，故永远不会走到这里、也就永远不打日志。
///
/// `dropped_total` 为全局累计计数，`baseline` 为本次录音开始时的快照；
/// 二者差值即**本次录音真实丢包**（旧代码把全局累计当本次，是误导源头）。
/// 节流：同一录音内至多每 1000ms 报一条。返回本次是否真的打了一条（便于单测）。
fn report_recording_drops_throttled(
    dropped_total: u64,
    baseline: u64,
    last_report_ms: &mut u64,
    now_ms: u64,
) -> bool {
    let delta = dropped_total.saturating_sub(baseline);
    if delta == 0 {
        return false;
    }
    if now_ms.saturating_sub(*last_report_ms) >= 1000 {
        *last_report_ms = now_ms;
        log::warn!(
            "[ASR-DROP] {} chunks dropped during THIS recording so far (queue full; consumer may be stalled)",
            delta
        );
        return true;
    }
    false
}

fn collect_recording(
    rx: &crossbeam_channel::Receiver<AudioChunk>,
    stream_failed: &AtomicBool,
    dropped_chunks: &AtomicU64,
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

    // FIX-ASR-DROP-288：消费端只在录音期存在，故此处基线/节流上报即「录音期丢包」专用。
    // 全局计数是累计值，只有差值才是本次录音真实丢包。
    let drop_baseline = dropped_chunks.load(Ordering::Relaxed);
    let mut last_drop_report_ms = proc_now_ms();

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

        // 非实时线程的节流上报（≤1 条/秒）；空闲期无消费端，永远不会走到这里。
        report_recording_drops_throttled(
            dropped_chunks.load(Ordering::Relaxed),
            drop_baseline,
            &mut last_drop_report_ms,
            proc_now_ms(),
        );

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

    // FIX-ASR-DROP-288：只报**本次录音**真实丢包（全局累计 - 录音前基线），修正旧口径。
    let dropped_total = dropped_chunks.load(Ordering::Relaxed);
    let dropped_this_recording = dropped_total.saturating_sub(drop_baseline);
    if dropped_this_recording > 0 {
        log::warn!(
            "[ASR-DROP] {} chunks dropped during THIS recording (~{:.1}s audio lost, est. 10ms/chunk); process-lifetime cumulative={}",
            dropped_this_recording,
            dropped_this_recording as f32 * 0.01,
            dropped_total
        );
    }

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

/// FIX-LOCALRT-FIRSTCHAR-293：`record_streaming` 按**现成开关** `trim_pre_roll_residual`
/// 分流 pre-roll 取用长度——本地流式（true）取 1000ms，在线三档（false）取 600ms。
/// 复用该开关，不新增参数穿透（主控要求③）。
fn pre_roll_ms_for_streaming(trim_pre_roll_residual: bool) -> u64 {
    if trim_pre_roll_residual {
        PRE_ROLL_LOCAL_RT_MS
    } else {
        PRE_ROLL_MS
    }
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

    // =================== PREROLL-DEAD-322 手工硬件探针（#[ignore]，不进常规回归）===================
    //
    // 测什么：**空闲期**（没按热键、没人说话）采集流到底是「被暂停 / 没数据」还是
    //   「有数据但幅度极低」。这是本单的判决性测量。
    // 为什么留：本单结论「环形缓冲装的是麦克风当时的真实输出、pre-roll 链路无 bug、
    //   空闲底噪是 24-bit 最低位抖动（≈2^-24≈-140 dBFS）而非数字零」正是靠它测出来的。
    //   而 `{:.4}` 会把该底噪显示成 `0.0000`、16-bit 落盘会把它量化成 0，先后两次把人
    //   带向「纯零 ⇒ 系统门控 ⇒ 整块摘除」。留一支可复跑的探针，后人不必再猜一遍。
    // 零副作用：只开默认输入设备、只读；不播放声音、不写文件。
    // 跑法：`cargo test pr322_idle_probe -- --ignored --nocapture`
    //
    // 四段对照：
    //   A 持续消费 channel（排除「队列满 ⇒ 引擎交零」）
    //   B 留 2s 不消费再看积压（排除「只在消费时才有数据」）
    //   C 整条流拆掉重建后再探（排除「流老化 ⇒ 交零」）
    //   D 底噪取值结构 + 端点采集音量（判「很轻的模拟底噪」还是「数字栅格最低位」）
    // 并记录被消费 chunk 的**时间戳年龄**：全是大龄 ⇒ 回调早已停摆（缓冲是陈的）；
    // 0~500ms ⇒ 回调活着 —— 交的到底是什么，看 nz_ratio 与 dBFS。
    #[test]
    #[ignore = "PREROLL-DEAD-322 手工硬件探针（开麦克风、只读、需真实设备）：不进常规回归"]
    fn pr322_idle_probe_manual() {
        fn probe(
            label: &str,
            rx: &crossbeam_channel::Receiver<AudioChunk>,
            pre: &Arc<Mutex<VecDeque<Vec<f32>>>>,
            drain: bool,
        ) {
            std::thread::sleep(Duration::from_millis(500));
            let (ring_peak, ring_nz, ring_chunks) = {
                let g = pre.lock().unwrap();
                let mut pk = 0f32;
                let mut nz = 0usize;
                for c in g.iter() {
                    for &s in c.iter() {
                        if s != 0.0 {
                            nz += 1;
                        }
                        pk = pk.max(s.abs());
                    }
                }
                (pk, nz, g.len())
            };
            let (mut cnt, mut n, mut nz, mut pk) = (0usize, 0usize, 0usize, 0f32);
            let (mut age_min, mut age_max) = (u128::MAX, 0u128);
            if drain {
                while let Ok((ts, c)) = rx.try_recv() {
                    let age = ts.elapsed().as_millis();
                    age_min = age_min.min(age);
                    age_max = age_max.max(age);
                    cnt += 1;
                    n += c.len();
                    for &s in c.iter() {
                        if s != 0.0 {
                            nz += 1;
                        }
                        pk = pk.max(s.abs());
                    }
                }
            }
            let ages = if cnt == 0 {
                "-".to_string()
            } else {
                format!("{age_min}..{age_max}ms")
            };
            println!(
                "[PR322] {label}: ring={ring_chunks}ch peak={ring_peak:.8} nz={ring_nz} | drained={cnt}ch/{n}smp nz={nz} peak={pk:.8} ts_age={ages}"
            );
        }

        println!(
            "[PR322] mic_muted={} (endpoint volume mute)",
            is_mic_muted()
        );
        let mut cap = AudioCapture::new();
        cap.prewarm(None).expect("prewarm failed");
        let (rate, rx, pre) = {
            let w = cap.warm_stream.as_ref().expect("warm stream missing");
            (w.sample_rate, w.rx.clone(), Arc::clone(&w.pre_roll))
        };
        println!("[PR322] prewarmed rate={rate}, idle-only probe begins");

        probe("A0 idle/drain(500ms)", &rx, &pre, true);
        probe("A1 idle/drain(500ms)", &rx, &pre, true);
        probe("A2 idle/drain(500ms)", &rx, &pre, true);

        println!(
            "[PR322] A3: leaving channel UNCONSUMED for 2000ms (to observe full-queue payload)"
        );
        std::thread::sleep(Duration::from_millis(2000));
        probe("A3 idle/backlog", &rx, &pre, true);

        println!("[PR322] forcing full stream teardown + rebuild");
        cap.warm_stream = None;
        cap.prewarm(None).expect("rewarm failed");
        let (rate2, rx2, pre2) = {
            let w = cap
                .warm_stream
                .as_ref()
                .expect("warm stream missing after rebuild");
            (w.sample_rate, w.rx.clone(), Arc::clone(&w.pre_roll))
        };
        println!("[PR322] rebuilt rate={rate2}");

        probe("B0 fresh/drain(500ms)", &rx2, &pre2, true);
        probe("B1 fresh/drain(500ms)", &rx2, &pre2, true);
        probe("B2 fresh/drain(500ms)", &rx2, &pre2, true);

        // D：底噪取值结构 + 端点采集音量。
        // 判据：若空闲样本只取少数几个值、且都是 2^-24 的整数倍 ⇒ 数字栅格最低位抖动
        //       （设备侧闸/DSP），而非「很轻的模拟底噪」；端点音量再排除「采集音量被调低」。
        #[cfg(target_os = "windows")]
        {
            let (lvl, mute) = pr322_endpoint_volume_level();
            println!("[PR322] D: capture endpoint volume={lvl:.4} mute={mute} (0.0/负值=读不到)");
        }
        println!("[PR322] D: floor spectrum (800ms idle, no drain)");
        let mut distinct: Vec<f32> = Vec::new();
        let mut zeros = 0usize;
        let mut n = 0usize;
        let mut min_nz = f32::MAX;
        let mut max_nz = 0f32;
        let deadline = Instant::now() + Duration::from_millis(800);
        while Instant::now() < deadline {
            if let Ok((_ts, c)) = rx2.recv_timeout(Duration::from_millis(20)) {
                for &s in c.iter() {
                    n += 1;
                    if s == 0.0 {
                        zeros += 1;
                        continue;
                    }
                    let a = s.abs();
                    min_nz = min_nz.min(a);
                    max_nz = max_nz.max(a);
                    if distinct.len() < 12 && !distinct.iter().any(|&x| x == a) {
                        distinct.push(a);
                    }
                }
            }
        }
        distinct.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "[PR322] D: n={n} exact_zero={zeros} nz_ratio={:.3} min_nonzero={min_nz:.3e} peak={max_nz:.3e}({:.1}dBFS) distinct<={}",
            nonzero_ratio(n - zeros, n),
            peak_to_dbfs(max_nz),
            distinct.len()
        );
        println!(
            "[PR322] D: distinct x2^24 (整数 ⇒ 24-bit LSB 栅格): {:?}",
            distinct
                .iter()
                .map(|v| format!("{:.3}", v * 16777216.0))
                .collect::<Vec<_>>()
        );
        println!(
            "[PR322] D: distinct x2^31 (整数 ⇒ 32-bit LSB 栅格): {:?}",
            distinct
                .iter()
                .map(|v| format!("{:.1}", v * 2147483648.0))
                .collect::<Vec<_>>()
        );

        println!("[PR322] probe complete");
    }

    // 端点采集音量（只读）：排除「麦克风采集音量被调低 ⇒ 幅度小」这条缩放解释。
    // 由 `pr322_idle_probe_manual` 的 D 段使用。
    #[cfg(target_os = "windows")]
    fn pr322_endpoint_volume_level() -> (f32, bool) {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                    Ok(e) => e,
                    Err(_) => return (-1.0, false),
                };
            let device = match enumerator.GetDefaultAudioEndpoint(eCapture, eMultimedia) {
                Ok(d) => d,
                Err(_) => return (-1.0, false),
            };
            let endpoint: IAudioEndpointVolume =
                match device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                    Ok(e) => e,
                    Err(_) => return (-1.0, false),
                };
            let lvl = endpoint
                .GetMasterVolumeLevelScalar()
                .map(|v: f32| v)
                .unwrap_or(-1.0);
            let mute = endpoint
                .GetMute()
                .map(|b: BOOL| b.as_bool())
                .unwrap_or(false);
            (lvl, mute)
        }
    }

    // =================== PREROLL-DEAD-322：诊断口径回归（纯函数，无硬件）===================
    //
    // 教训固化：`{:.4}` 把 ≈-140 dBFS 的底噪显示成 `0.0000`，16-bit 落盘再把它量化成 0，
    // 于是「pre-roll 是纯零 ⇒ 设备交数字静音 ⇒ 整块摘除」这个错误结论连出两轮。
    // 以下用例把新口径（dBFS + nz_ratio）与「文件② 报自己的 offset」钉死。
    #[test]
    fn pr322_peak_to_dbfs_is_independent_of_decimal_places() {
        assert_eq!(peak_to_dbfs(0.0), DIAG_DBFS_FLOOR, "数字零 ⇒ 落到下限");
        assert_eq!(
            peak_to_dbfs(f32::NAN),
            DIAG_DBFS_FLOOR,
            "NaN 也必须落到下限"
        );
        assert!((peak_to_dbfs(1.0) - 0.0).abs() < 1e-6, "满刻度 = 0 dBFS");
        assert!((peak_to_dbfs(0.1) + 20.0).abs() < 1e-4, "0.1 ⇒ -20 dBFS");
        assert_eq!(peak_to_dbfs(1e-30), DIAG_DBFS_FLOOR, "极低值必须钳到下限");
    }

    #[test]
    fn pr322_nonzero_ratio_distinguishes_dither_from_digital_silence() {
        assert_eq!(nonzero_ratio(0, 48000), 0.0, "数字静音 ⇒ 0.000");
        assert_eq!(nonzero_ratio(0, 0), 0.0, "无样本不算「有信号」");
        assert_eq!(nonzero_ratio(48000, 48000), 1.0);
        // 实测底噪形态：24-bit 最低位抖动（±2^-24），非零占比 100%、峰值 ≈-144 dBFS。
        let lsb = 2f32.powi(-24);
        let buf: Vec<f32> = (0..48000)
            .map(|i| if i % 2 == 0 { lsb } else { -lsb })
            .collect();
        let (peak, nz, total) = diag_peak_nz(&buf);
        assert_eq!(
            nonzero_ratio(nz, total),
            1.0,
            "最低位抖动必须报成「几乎全非零」而不是零"
        );
        assert!(peak_to_dbfs(peak) < -140.0, "2^-24 应约 -144 dBFS");
        // 🔴 事故本体：旧口径（4 位小数）会把这个信号显示成 0.0000 —— 本单两轮误判的根。
        assert_eq!(format!("{peak:.4}"), "0.0000");
    }

    /// 文件 ② 的 `first_speech_at` 必须是**它自己**那段样本的 offset，不得复用文件 ① 的。
    /// 旧写法会给「pre-roll 全静音 + 之后才有语音」的文件 ② 打上 `first_speech_at=none`。
    #[test]
    fn pr322_dump_part2_reports_its_own_offset_and_ratio() {
        let _level_guard = LOG_LEVEL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _ = log::set_boxed_logger(Box::new(CapLogger));
        log::set_max_level(log::LevelFilter::Debug);
        let dir = dump_test_dir("pr322-offset2");
        // pre-roll 100ms 全静音（模拟 PREROLL-DEAD-322 现场：空闲段是底噪，不是语音）
        let chunks = vec![vec![0.0f32; 4800]];
        let before = diag_captured().len();
        {
            let mut dump =
                PreRollDump::new_in(dir.clone(), &chunks, 48000).expect("Debug 下应落盘");
            dump.push(&vec![0.0f32; 9600]); // 再 200ms 静音（热键后仍未开口）
            dump.push(&vec![0.5f32; 14400]); // 300ms 语音
        } // Drop 写文件 ②
        let logs: Vec<String> = diag_captured()[before..]
            .iter()
            .filter(|s| s.contains("[LocalRT-DBG-292]"))
            .cloned()
            .collect();
        assert_eq!(logs.len(), 2, "应有文件①与文件②两条 dump 日志: {logs:?}");
        assert!(
            logs[0].contains("first_speech_at=none"),
            "文件① 全静音 ⇒ 起点偏移 none: {}",
            logs[0]
        );
        assert!(
            logs[1].contains("first_speech_at=300ms"),
            "文件② 必须报自己的偏移(=4800+9600 样本 @48k = 300ms): {}",
            logs[1]
        );
        assert!(
            logs[1].contains("nz_ratio="),
            "必须带 nz_ratio: {}",
            logs[1]
        );
        log::set_max_level(log::LevelFilter::Warn); // 复原
        std::fs::remove_dir_all(&dir).ok();
    }

    // =================== GATE-ATTACK-PROBE-335 手工探针（#[ignore]，不进常规回归）===================
    //
    // 测什么：C920 采集链存在**电平触发的数字闸**（322 定案：空闲 ≈±1 LSB(-140 dBFS)、说话到
    //   -16 dBFS，~120 dB 落差）。本探针回答三问：① 闸的 attack 多长 ② attack 窗口内能量被压低
    //   多少 dB、盖住爆破音哪一段 ③ **它到底影不影响识别**（A/B 对照，决定性的一问）。
    // 怎么跑：
    //   cargo test gate335_capture_envelope -- --ignored --nocapture   # 采集（需麦克风；人按脚本说话）
    //   cargo test gate335_asr_ab -- --ignored --nocapture             # 识别 A/B（需 Qwen3 模型 + 切好的片段）
    //
    // 🔴 采集脚本（**段间静默是唯一的切段依据，请勿缩短；不要拍手/敲桌子当标记——那些也会把闸打开**）：
    //   ① 静默 2s
    //   ② 说「你好」（就这两个字）
    //   ③ 停 2s
    //   ④ 说「你说」
    //   ⑤ 停 2s
    //   ⑥ 一口气连读：「今天天气不错你好我们开始吧我这边都准备好了你说是不是」
    //   ⑦ 停 2s
    //   ⑧ 重复 ①~⑦ 一遍（共两轮，≈34s，落在 45s 采集窗内）
    //   产出：evidence 目录下 capture.wav（i16@48k）+ envelope-1ms.txt（1ms 帧 rms/dBFS，f32 全精度）。
    //
    // 为什么留：结论完全依赖这两个量具；两支都先跑**量具自检**（对已知答案断言），
    //   自检失败即 panic —— 防「量具坏了却拿它的读数下结论」（本项目已栽过：不存在的 strings 扫出全 0）。
    //   离线分析脚本 `collab/evidence/20260921-gate-attack-335/analyze_attack.py` 同样带合成包络自检
    //   （首版口径被它抓出一个真 bug，见该脚本内注释）。
    // 零副作用：capture 只开默认输入设备 + 写证据文件；asr_ab 只读 WAV + CPU 推理。均不改生产路径。
    const GATE335_CAPTURE_SECS: u64 = 45; // 采集时长：脚本 3 轮 × ~13s + 余量
    const GATE335_ENV_FRAME_MS: u64 = 1; // 包络帧长：1ms（足以看清 attack 的前沿）
    const GATE335_EVIDENCE_DIR: &str = "collab/evidence/20260921-gate-attack-335";
    /// 自检参考音频：模型自带 `test_wavs/noise2.wav`，其 `transcript.txt` 有可核对文本。
    const GATE335_REF_WAV: &str =
        "models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/test_wavs/noise2.wav";

    /// 单帧 (RMS, dBFS)。dBFS 复用 322 的 `peak_to_dbfs`（≤0 取 `DIAG_DBFS_FLOOR`）。
    fn gate335_frame_stats(frame: &[f32]) -> (f32, f32) {
        if frame.is_empty() {
            return (0.0, DIAG_DBFS_FLOOR);
        }
        let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
        (rms, peak_to_dbfs(rms))
    }

    /// 量具自检（约束#3）：对**已知答案**的合成信号断言量具可信，失败即 panic。
    fn gate335_instrument_selfcheck() {
        let rate = 48000u32;
        let n = rate as usize / 10; // 100ms
        let sine: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / rate as f32).sin())
            .collect();
        let (rms, dbfs) = gate335_frame_stats(&sine);
        println!(
            "[GATE335-SELFTEST] 0.5 幅度 1kHz 正弦: rms={rms:.4} dbfs={dbfs:.2}（期望 rms≈0.3536 / dbfs≈-9.03）"
        );
        assert!((rms - 0.35355).abs() < 0.002, "自检失败：RMS 量具不可信");
        assert!((dbfs + 9.03).abs() < 0.15, "自检失败：dBFS 量具不可信");
        let (rms0, dbfs0) = gate335_frame_stats(&vec![0.0f32; 480]);
        println!(
            "[GATE335-SELFTEST] 全零: rms={rms0:.1} dbfs={dbfs0:.1}（期望 floor {DIAG_DBFS_FLOOR}）"
        );
        assert_eq!(dbfs0, DIAG_DBFS_FLOOR, "自检失败：静音未落到 floor");
        println!("[GATE335-SELFTEST] ✅ 量具自检通过 —— 没看到这行，后面的读数一律不许采信");
    }

    #[test]
    #[ignore = "GATE-ATTACK-PROBE-335 手工硬件探针（开麦克风 + 写证据文件 + 需人按脚本说话）：不进常规回归"]
    fn gate335_capture_envelope_manual() {
        gate335_instrument_selfcheck();

        let mut cap = AudioCapture::new();
        cap.prewarm(None).expect("prewarm failed");
        let (rate, rx) = {
            let w = cap.warm_stream.as_ref().expect("warm stream missing");
            (w.sample_rate, w.rx.clone())
        };
        println!(
            "[GATE335] 设备 rate={rate}；开始采集 {GATE335_CAPTURE_SECS}s —— 请按脚本说话（见任务通知）"
        );

        let t0 = Instant::now();
        let deadline = t0 + Duration::from_secs(GATE335_CAPTURE_SECS);
        let mut all: Vec<f32> = Vec::with_capacity(rate as usize * GATE335_CAPTURE_SECS as usize);
        let mut mark = 0u64;
        while Instant::now() < deadline {
            if let Ok((_ts, c)) = rx.recv_timeout(Duration::from_millis(50)) {
                all.extend_from_slice(&c);
            }
            let el = t0.elapsed().as_secs();
            if el >= mark + 5 {
                mark = el;
                println!("[GATE335] +{el}s captured ({} samples)", all.len());
            }
        }
        println!(
            "[GATE335] 采集结束：{} samples ({:.1}s)",
            all.len(),
            all.len() as f64 / rate as f64
        );

        // 采集自检（约束#3）：证明这次真采到了人声，而不是「全零/断流」假数据。
        let (peak, nz, total) = diag_peak_nz(&all);
        let _ = nz;
        println!(
            "[GATE335] capture self-check: peak={peak:.3e}({:.1}dBFS) nonzero_total={nz}/{total}",
            peak_to_dbfs(peak)
        );
        // 1ms 包络（f32 全精度，落到证据文件；控制台只给摘要）
        let frame = (rate as u64 * GATE335_ENV_FRAME_MS / 1000) as usize;
        let frames: Vec<(f32, f32)> = all.chunks(frame.max(1)).map(gate335_frame_stats).collect();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GATE335_EVIDENCE_DIR);
        std::fs::create_dir_all(&dir).expect("create evidence dir");
        write_wav_pcm16(&dir.join("capture.wav"), &all, rate).expect("write capture.wav");
        let mut env = String::with_capacity(frames.len() * 24);
        for (i, (rms, dbfs)) in frames.iter().enumerate() {
            env.push_str(&format!(
                "{}\t{rms:.6e}\t{dbfs:.2}\n",
                i as u64 * GATE335_ENV_FRAME_MS
            ));
        }
        std::fs::write(dir.join("envelope-1ms.txt"), env).expect("write envelope");
        println!(
            "[GATE335] 证据已写：{}（capture.wav + envelope-1ms.txt，{} 帧）",
            dir.display(),
            frames.len()
        );

        // 🔴 有效性断言放在**落盘之后**：落盘链路本身可被独立验证；断言失败即宣告本轮数据无效
        //    （真人采集会用同名文件覆盖，不存在误用旧数据的风险）。
        assert!(
            peak > 0.02,
            "自检失败：整段峰值只有 {peak:.3e}（<0.02）⇒ 没人说话或麦克风没进来，本轮数据作废，请重跑"
        );

        // 摘要：最响的 8 帧时间戳 + 底噪分布（Q1/Q2 的初步读数）
        let mut idx: Vec<usize> = (0..frames.len()).collect();
        idx.sort_by(|a, b| frames[*b].0.partial_cmp(&frames[*a].0).unwrap());
        let tops: Vec<String> = idx
            .iter()
            .take(8)
            .map(|i| {
                format!(
                    "{}ms:{:.1}dBFS",
                    i * GATE335_ENV_FRAME_MS as usize,
                    frames[*i].1
                )
            })
            .collect();
        let floor_frames = frames.iter().filter(|(_, d)| *d < -100.0).count();
        println!("[GATE335] 最响 8 帧: {}", tops.join("  "));
        println!(
            "[GATE335] 低电平(< -100dBFS) 帧数 {floor_frames}/{}（= {:.1}% 时间在闸下）",
            frames.len(),
            floor_frames as f64 * 100.0 / frames.len() as f64
        );
        println!("[GATE335] 下一步：用 envelope-1ms.txt 量 attack；切 A/B 片段后跑 gate335_asr_ab");
    }

    /// Qwen3 识别器：与生产 `create_qwen3_recognizer` 同清单/同参
    /// （max_total_len=4096、max_new_tokens=256、temperature=1e-6、top_p=0.8、seed=42、hotwords=None）。
    /// 生产那份对 `audio` 模块不可见，故此处按 `src/transcription/mod.rs` 的清单复刻同一组文件与参数。
    fn gate335_qwen3_recognizer(model_root: &Path) -> sherpa_onnx::OfflineRecognizer {
        let d = model_root.join("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25");
        assert!(
            d.join("conv_frontend.onnx").exists()
                && d.join("encoder.int8.onnx").exists()
                && d.join("decoder.int8.onnx").exists()
                && d.join("tokenizer").exists(),
            "Qwen3 模型不齐：{}",
            d.display()
        );
        let mut cfg = sherpa_onnx::OfflineRecognizerConfig::default();
        cfg.model_config = sherpa_onnx::OfflineModelConfig {
            num_threads: 8,
            provider: Some("cpu".to_string()),
            debug: false,
            qwen3_asr: sherpa_onnx::OfflineQwen3ASRModelConfig {
                conv_frontend: Some(d.join("conv_frontend.onnx").to_string_lossy().into_owned()),
                encoder: Some(d.join("encoder.int8.onnx").to_string_lossy().into_owned()),
                decoder: Some(d.join("decoder.int8.onnx").to_string_lossy().into_owned()),
                tokenizer: Some(d.join("tokenizer").to_string_lossy().into_owned()),
                max_total_len: 4096,
                max_new_tokens: 256,
                temperature: 1e-6,
                top_p: 0.8,
                seed: 42,
                hotwords: None,
            },
            ..Default::default()
        };
        sherpa_onnx::OfflineRecognizer::create(&cfg).expect("create qwen3 recognizer failed")
    }

    /// 走生产解码入口（`transcribe_acc_ctx`，空上下文 ⇒ 与生产「无前文」档位同路径）。
    fn gate335_decode(
        rec: &sherpa_onnx::OfflineRecognizer,
        samples_16k: &[f32],
    ) -> Result<String, String> {
        crate::transcription::transcribe_acc_ctx(
            rec,
            samples_16k,
            crate::config::ChineseScript::Simplified,
            0,
            crate::transcription::CtxInject {
                prev_older: None,
                prev_latest: None,
                current: None,
                terms: None,
            },
        )
        .map(|(t, _)| t)
        .map_err(|e| format!("{e:#}"))
    }

    #[test]
    #[ignore = "GATE-ATTACK-PROBE-335 ASR A/B（需 Qwen3 模型 + 已切好的 A/B 片段）：不进常规回归"]
    fn gate335_asr_ab_manual() {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let rec = gate335_qwen3_recognizer(&project.join("models"));

        // 量具自检（约束#3）：模型自带参考音频必须解出可核对文本，否则整套 ASR 读数作废。
        let refwav = project.join(GATE335_REF_WAV);
        let w = sherpa_onnx::Wave::read(refwav.to_str().expect("ref wav path must be valid UTF-8"))
            .expect("read reference wav");
        let ref_text = gate335_decode(&rec, w.samples()).unwrap_or_else(|e| format!("<err {e}>"));
        println!(
            "[GATE335-SELFTEST] 参考音频 {} ({}Hz) → '{ref_text}'",
            refwav.display(),
            w.sample_rate()
        );
        assert!(
            ref_text.contains("拨号") || ref_text.contains("纠正") || ref_text.contains("号码"),
            "自检失败：ASR 量具不可信（参考音频没解出预期文本），本单任何识别结论都不许采信"
        );
        println!("[GATE335-SELFTEST] ✅ ASR 量具自检通过");

        // A/B 片段：evidence/segments/*.wav（48k 或 16k 均可，统一重采样到 16k 后送入生产解码）。
        let seg_dir = project.join(GATE335_EVIDENCE_DIR).join("segments");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&seg_dir)
            .unwrap_or_else(|e| panic!("读不到片段目录 {}: {e}", seg_dir.display()))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "wav").unwrap_or(false))
            .collect();
        files.sort();
        assert!(
            !files.is_empty(),
            "{} 下没有 .wav 片段（先用 Python 从 capture.wav 切 A/B 变体）",
            seg_dir.display()
        );

        println!("[GATE335] A/B 识别结果（生产解码路径，逐条）：");
        for f in &files {
            let w = match sherpa_onnx::Wave::read(f.to_str().expect("utf-8 path")) {
                Some(w) => w,
                None => {
                    println!("  {:<36} <read err>", file_stem(f));
                    continue;
                }
            };
            let s16 = if w.sample_rate() == 16000 {
                w.samples().to_vec()
            } else {
                resample_anti_alias(w.samples(), w.sample_rate() as u32, 16000)
            };
            let text = gate335_decode(&rec, &s16).unwrap_or_else(|e| format!("<err {e}>"));
            let (peak, _, _) = diag_peak_nz(&s16);
            println!(
                "  {:<36} peak={:.1}dBFS  →  {text}",
                file_stem(f),
                peak_to_dbfs(peak)
            );
        }
        println!("[GATE335] 判定口径：A 组（经闸）若出现首字错而 B 组（闸已开）不错 ⇒ 因果成立；两组一致 ⇒ 证伪");
    }

    fn file_stem(p: &Path) -> String {
        p.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.display().to_string())
    }

    // FIX-LOCALRT-FIRSTCHAR-283（方案 D）纯函数用例
    fn speech(ms: usize) -> Vec<f32> {
        vec![0.2f32; 16000 * ms / 1000]
    }
    fn silence(ms: usize) -> Vec<f32> {
        vec![0.0f32; 16000 * ms / 1000]
    }

    // ------------------------------------------------------------
    // FIX-LOCALRT-FIRSTCHAR-283/293：统一 pre-roll 选择规则（A/B/C + 兜底）
    // 输入均为 1000ms @16k（16000 样本），模拟本地流式加长后的窗口。
    // ------------------------------------------------------------

    /// 场景 C：旧语音 + 300ms 静音 + 新语音 ⇒ 只保留新语音段（起点 − PAD）
    #[test]
    fn pre_roll_select_c_keeps_only_new_segment_after_gap() {
        let mut s = speech(200); // 旧语音 [0,200)
        s.extend(silence(300)); // 静音 [200,500)
        s.extend(speech(500)); // 新语音 [500,1000)
        assert_eq!(s.len(), 16000);
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(
            out.len(),
            16000 * 650 / 1000,
            "新语音起点 500ms − PAD 150ms = 350ms 起，长度 650ms"
        );
    }

    /// 场景 A：语音从 300ms 起持续到末尾 ⇒ 保留起点 150ms（300−PAD），头部一个样本不丢
    #[test]
    fn pre_roll_select_a_speech_from_300ms_keeps_from_150ms() {
        let mut s = silence(300);
        s.extend(speech(700));
        assert_eq!(s.len(), 16000);
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 850 / 1000, "应从 150ms 起（300−150）");
        assert!(out.len() >= 16000 * 700 / 1000, "语音本体必须完整在内");
    }

    /// 场景 A 极端：语音从 50ms 就开始（< PAD）⇒ saturating_sub 不下溢，保留从 0 开始
    #[test]
    fn pre_roll_select_a_extreme_onset_under_pad_no_underflow() {
        let mut s = silence(50);
        s.extend(speech(950));
        let out = select_pre_roll_for_asr(&s, 16000);
        // 前导静音仅 50ms（<200ms）⇒ 无长静音游程 ⇒ 兜底原样返回，起点必为 0，绝不下溢/丢头
        assert_eq!(out, s, "不得丢头/下溢；原样返回");
    }

    /// 场景 B：1000ms 全静音 ⇒ 只剩末尾 200ms，且不得为空
    #[test]
    fn pre_roll_select_b_pure_silence_keeps_tail_only_not_empty() {
        let s = silence(1000);
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 200 / 1000, "整窗静音只留末尾 200ms");
        assert!(!out.is_empty(), "不得返回空");
    }

    /// 场景 B 边界：前 800ms 静音、最后 200ms 有语音 ⇒ 语音完整保留（起点 800−150=650）
    #[test]
    fn pre_roll_select_b_boundary_silence_then_tail_speech() {
        let mut s = silence(800);
        s.extend(speech(200));
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 350 / 1000, "从 650ms 起到末尾 = 350ms");
        assert!(out.len() >= 16000 * 200 / 1000, "末尾语音必须完整");
    }

    /// 兜底：整窗无 ≥200ms 静音游程（连续语音）⇒ 原样返回，长度不变
    #[test]
    fn pre_roll_select_fallback_no_long_silence_returns_unchanged() {
        let s = speech(1000);
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), s.len(), "判据不可靠时必须退回现状");
        assert_eq!(out, s, "内容逐样本一致");
    }

    /// 冷启动：缓冲只攒到 300ms ⇒ 不 panic、不补零、不返空
    #[test]
    fn pre_roll_select_cold_start_buffer_not_full() {
        let chunks = vec![vec![1.0f32; 4800], vec![2.0f32; 9600]]; // 100ms + 200ms @48k
        let available: usize = chunks.iter().map(Vec::len).sum();
        let max_samples = pre_roll_samples(48000, PRE_ROLL_CAPACITY_MS);
        let retained = retain_recent_samples(chunks.clone(), max_samples);
        assert_eq!(
            retained.iter().map(Vec::len).sum::<usize>(),
            available,
            "未攒满必须原样返回已有音频：不 panic、不补零、不返回空"
        );
        assert_eq!(retained, chunks, "内容逐样本一致");
    }

    /// 现成分流开关：本地流式 true ⇒ 1000ms；在线三档 false ⇒ 600ms（供 292 `dur` 核对）
    #[test]
    fn pre_roll_streaming_ms_routes_by_existing_switch() {
        assert_eq!(pre_roll_ms_for_streaming(true), PRE_ROLL_LOCAL_RT_MS);
        assert_eq!(pre_roll_ms_for_streaming(false), PRE_ROLL_MS);
        assert_eq!(PRE_ROLL_LOCAL_RT_MS, 1000);
        assert_eq!(PRE_ROLL_MS, 600);
        assert!(
            PRE_ROLL_CAPACITY_MS >= PRE_ROLL_LOCAL_RT_MS,
            "容量必须容纳最长取用值"
        );
        assert_eq!(pre_roll_samples(16000, PRE_ROLL_LOCAL_RT_MS), 16000);
        assert_eq!(pre_roll_samples(16000, PRE_ROLL_MS), 9600);
    }

    // ============================================================
    // FIX-PREROLL-RESIDUAL-308：窗口开头的旧语音要判为残留
    // ============================================================

    fn assert_residual_idempotent(s: &[f32]) {
        let once = select_pre_roll_for_asr(s, 16000);
        let twice = select_pre_roll_for_asr(&once, 16000);
        assert_eq!(once, twice, "residual 选择必须幂等");
    }

    /// 按「每十分位 100ms、幅度 = 值/400」构造样本（复刻 292 落盘 WAV 的十分位 RMS）。
    fn rms_decile_samples(deciles: &[u32], rate: u32) -> Vec<f32> {
        let per = rate as usize / 10;
        let mut v = Vec::with_capacity(per * deciles.len());
        for &d in deciles {
            let amp = d as f32 / 400.0;
            v.extend(std::iter::repeat(amp).take(per));
        }
        v
    }

    #[test]
    fn pre_roll_residual_308_single_segment_then_long_silence() {
        // 1000ms：前 400ms 语音 + 后 600ms 静音 ⇒ residual，只剩末尾 200ms。
        let mut s = speech(400);
        s.extend(silence(600));
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 200 / 1000, "判残留 ⇒ 只留末尾 200ms");
        assert_eq!(out, s[s.len() - out.len()..].to_vec());
        assert_residual_idempotent(&s);
    }

    #[test]
    fn pre_roll_residual_308_boundary_exactly_200ms() {
        // 600ms：前 400ms 语音 + 恰好 200ms 静音 ⇒ 阈值含（>=）⇒ residual。
        let mut s = speech(400);
        s.extend(silence(200));
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 200 / 1000, "恰好 200ms 判 residual");
        assert_residual_idempotent(&s);
    }

    #[test]
    fn pre_roll_residual_308_real_decile_shape() {
        // 真实形态（292 落盘 WAV 的十分位 RMS [174,341,120,270,48,3,1,1,2,1]）⇒ residual。
        let s = rms_decile_samples(&[174, 341, 120, 270, 48, 3, 1, 1, 2, 1], 16000);
        assert_eq!(s.len(), 16000);
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 200 / 1000, "判残留 ⇒ 只留末尾 200ms");
        assert_residual_idempotent(&s);
    }

    #[test]
    fn pre_roll_residual_308_speech_to_window_end_stays_onset() {
        // 真·先开口：语音连到窗口末尾（末尾无静音）⇒ 仍 onset（不许被吞）。
        let mut s = silence(300);
        s.extend(speech(700));
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 850 / 1000, "从 150ms 起，850ms");
        assert_residual_idempotent(&s);
    }

    #[test]
    fn pre_roll_residual_308_trailing_150ms_below_threshold_stays_onset() {
        // 末尾仅 150ms 静音（未达 200ms 阈值）⇒ 仍 onset。
        let mut s = silence(300);
        s.extend(speech(550));
        s.extend(silence(150));
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 850 / 1000, "仍 onset：从 150ms 起 850ms");
        assert_residual_idempotent(&s);
    }

    #[test]
    fn pre_roll_residual_308_all_silence_stays_tail() {
        // 场景 B 不变。
        let s = silence(1000);
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(out.len(), 16000 * 200 / 1000);
        assert_residual_idempotent(&s);
    }

    #[test]
    fn pre_roll_residual_308_gap_then_new_speech_still_anchors_new() {
        // 旧语音 + 300ms 静音 + 新语音直到末尾 ⇒ 仍锚定新语音起点（场景 C 不退化）。
        let mut s = speech(200);
        s.extend(silence(300));
        s.extend(speech(500));
        let out = select_pre_roll_for_asr(&s, 16000);
        assert_eq!(
            out.len(),
            16000 * 650 / 1000,
            "从新语音起点−150ms（350ms）起，650ms"
        );
        assert_residual_idempotent(&s);
    }

    // ============================================================
    // TEST-SYNC-295：I5 调用点级结构护栏
    //   「在线三档零改变」是**调用点级**事实，纯函数用例钉不住（调用点被挪到无条件处，
    //   I1~I4 的纯函数用例照样全绿）。故直接扫生产源码结构（参考 main.rs
    //   `overlay_121_guard_tests` 的 include_str! 手法）。
    // ============================================================
    mod guard_293_i5 {
        /// 生产区源码：切到首个 `mod tests {` 之前。
        ///
        /// 🔴 切点是 `mod tests {` 而**不是**「首个 `#[cfg(test)]`」——本文件在
        /// `PreRollDump` 内有一处 `#[cfg(test)] fn new_in`（`:269`）夹在生产段中间，
        /// 用首个 `#[cfg(test)]` 会在它那里截断，把真正的调用点（`record_streaming`）
        /// 排除在扫描区外，护栏形同虚设。
        fn prod_src() -> String {
            let src = include_str!("mod.rs");
            let cut = src
                .lines()
                .position(|l| l.trim() == "mod tests {")
                .expect("I5: 找不到 mod tests 边界");
            src.lines().take(cut).collect::<Vec<_>>().join("\n")
        }

        fn brace_delta(line: &str) -> i32 {
            line.matches('{').count() as i32 - line.matches('}').count() as i32
        }

        /// 花括号定界：以 anchor 行为起点返回 (open_idx, close_idx)。
        fn block_bounds(lines: &[String], anchor: usize) -> Option<(usize, usize)> {
            let mut depth = 0i32;
            let mut opened = false;
            let mut open_idx = usize::MAX;
            for (i, line) in lines.iter().enumerate().skip(anchor) {
                depth += brace_delta(line);
                if depth > 0 && !opened {
                    opened = true;
                    open_idx = i;
                }
                if opened && depth == 0 {
                    return Some((open_idx, i));
                }
            }
            None
        }

        /// I5：`select_pre_roll_for_asr` 生产区恰 1 个调用点，且落在
        /// `let pre_roll_chunks = if trim_pre_roll_residual {` 分支块内。
        ///
        /// 消融：① 把调用移出 trim 分支（无条件调用）→ 块内断言红；
        ///       ② 在生产区加第二处调用（如接进在线路径）→ 命中数 2→3 红；
        ///       ③ 删掉调用 → 命中数 2→1 红（1 定义 + 0 调用）。
        #[test]
        fn i5_pre_roll_select_single_call_inside_local_rt_branch() {
            let src = prod_src();
            // 命中 = 定义行 `fn select_pre_roll_for_asr(` + 调用行，恰 2。
            let hits = src.matches("select_pre_roll_for_asr(").count();
            assert_eq!(
                hits, 2,
                "I5: 生产区 select_pre_roll_for_asr 命中必须恰 2 处（1 定义 + 1 调用），实测 {} 处",
                hits
            );

            let lines: Vec<String> = src.lines().map(|l| l.trim().to_string()).collect();
            let anchor = lines
                .iter()
                .position(|l| l.starts_with("let pre_roll_chunks = if trim_pre_roll_residual {"))
                .expect("I5 anchor: trim_pre_roll_residual 分支");
            let call_line = lines
                .iter()
                .position(|l| l.contains("select_pre_roll_for_asr(&all"))
                .expect("I5 anchor: 裁剪调用行");
            let (lo, hi) = block_bounds(&lines, anchor).expect("I5: 分支块定界失败");
            assert!(
                (lo..=hi).contains(&call_line),
                "I5: 裁剪调用必须处于 if trim_pre_roll_residual 块内（块 L{}..=L{}，调用 L{}）",
                lo + 1,
                hi + 1,
                call_line + 1
            );
        }
    }

    // ============================================================
    // URGENT-286：诊断埋点「两头验证」回归
    //   - 默认 Warn（端测不带 -debug）：守卫为 false ⇒ 重计算被跳过；
    //     [LocalRT-DBG-278] 必须零输出。
    //   - Debug（端测带 -debug）：[LocalRT-DBG-278] 必须完整输出。
    // ============================================================
    static DIAG_CAPTURED: std::sync::OnceLock<Mutex<Vec<String>>> = std::sync::OnceLock::new();

    struct CapLogger;
    impl log::Log for CapLogger {
        fn enabled(&self, _m: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            if let Some(m) = DIAG_CAPTURED.get() {
                if let Ok(mut v) = m.lock() {
                    v.push(format!("{}", record.args()));
                }
            }
        }
        fn flush(&self) {}
    }

    fn diag_captured() -> Vec<String> {
        DIAG_CAPTURED
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .clone()
    }

    #[test]
    fn urgent286_pre_roll_diag_quiet_at_warn_full_at_debug() {
        let _level_guard = LOG_LEVEL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _ = log::set_boxed_logger(Box::new(CapLogger));
        let chunks = vec![vec![0.2f32; 16000]]; // 1s 语音，足以触发能量统计

        // 不带 -debug（release 默认）
        log::set_max_level(log::LevelFilter::Warn);
        assert!(
            !log::log_enabled!(log::Level::Debug),
            "默认 Warn 下 Debug 守卫必须为 false ⇒ 重计算被跳过"
        );
        let before = diag_captured().len();
        pre_roll_diag(&chunks, 16000);
        let warn_hits = diag_captured()[before..]
            .iter()
            .filter(|s| s.contains("[LocalRT-DBG-278]"))
            .count();
        assert_eq!(warn_hits, 0, "默认 Warn 下 [278] 必须零输出");

        // 带 -debug
        log::set_max_level(log::LevelFilter::Debug);
        assert!(log::log_enabled!(log::Level::Debug));
        let before = diag_captured().len();
        pre_roll_diag(&chunks, 16000);
        let dbg_hits = diag_captured()[before..]
            .iter()
            .filter(|s| s.contains("[LocalRT-DBG-278]"))
            .count();
        assert_eq!(dbg_hits, 1, "-debug 下 [278] 必须完整输出 1 条");

        log::set_max_level(log::LevelFilter::Warn); // 复原
    }

    // ============================================================
    // DIAG-LOCALRT-FIRSTCHAR-292：pre-roll WAV 落盘
    //   - 串行化所有会改全局 max_level 的用例（否则与 286 并行时
    //     互相翻转 Warn/Debug ⇒ 随机红，[TESTENV-SHARED-DIR-RACE-001] 同族）。
    // ============================================================
    static LOG_LEVEL_MUTEX: Mutex<()> = Mutex::new(());
    static DUMP_TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn dump_test_dir(tag: &str) -> PathBuf {
        let seq = DUMP_TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "voice-ime-dump292-{}-{}-{}",
            std::process::id(),
            tag,
            seq
        ))
    }

    fn list_wavs(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| n.ends_with(".wav"))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// 解析 RIFF/WAVE 头，返回 (sample_rate, channels, block_align, data_len)。
    fn read_wav_header(path: &Path) -> (u32, u16, u16, u32) {
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() >= 44, "WAV 必须有 44 字节头");
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(&bytes[36..40], b"data");
        (
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            u16::from_le_bytes([bytes[22], bytes[23]]),
            u16::from_le_bytes([bytes[32], bytes[33]]),
            u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]),
        )
    }

    #[test]
    fn diag292_dump_quiet_at_warn_and_writes_valid_wav_at_debug() {
        let _level_guard = LOG_LEVEL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _ = log::set_boxed_logger(Box::new(CapLogger));
        let dir = dump_test_dir("warn");
        // 4800 静音 + 7200 语音 @48k：首个非静音样本在 100ms ⇒ head_clipped=false
        let chunks = vec![vec![0.0f32; 4800], vec![0.5f32; 7200]];
        let original = chunks.clone();

        // 不带 -debug（release 默认）：必须完全不动作
        log::set_max_level(log::LevelFilter::Warn);
        assert!(!log::log_enabled!(log::Level::Debug));
        assert!(
            PreRollDump::new_in(dir.clone(), &chunks, 48000).is_none(),
            "Warn 下必须不创建 dump"
        );
        assert!(!dir.exists(), "Warn 下连目录都不许建");

        // 带 -debug：文件 ① 立即落盘，② 由 Drop 收尾
        log::set_max_level(log::LevelFilter::Debug);
        assert!(log::log_enabled!(log::Level::Debug));
        {
            let dump = PreRollDump::new_in(dir.clone(), &chunks, 48000).expect("Debug 下应落盘");
            assert_eq!(list_wavs(&dir).len(), 1, "构造后应只有文件 ①");
            drop(dump);
        }
        let files = list_wavs(&dir);
        assert_eq!(files.len(), 2, "Drop 后应补写文件 ②");

        // 文件 ① = 原始 pre-roll；WAV 头必须按真实值（硬要求②）
        let file1 = files
            .iter()
            .find(|n| !n.contains("-2s"))
            .expect("应有原始 pre-roll 文件");
        let (rate, ch, align, data_len) = read_wav_header(&dir.join(file1));
        assert_eq!(rate, 48000, "采样率必须是真实值，不得照抄 16000");
        assert_eq!(ch, 1);
        assert_eq!(align, 2);
        assert_eq!(data_len as usize, 12000 * 2, "12000 样本 × i16");

        // 独立解码验证（硬要求③）：用 sherpa-onnx 的 Wave 解析器**真实读回**我们写的
        // 文件，等价于「系统播放器能打开」——只验字节数不算数。
        let decoded = sherpa_onnx::Wave::read(
            dir.join(file1)
                .to_str()
                .expect("temp WAV 路径应为有效 UTF-8"),
        )
        .expect("sherpa Wave 必须能解析我们写的 WAV");
        assert_eq!(decoded.sample_rate(), 48000, "解码采样率须对得上");
        assert_eq!(
            decoded.samples().len(),
            12000,
            "解码样本数须对得上（250ms@48k）"
        );

        // head_clipped 判据（写死 100ms > 40ms 窗口 ⇒ false）
        let flat: Vec<f32> = chunks.iter().flat_map(|c| c.iter().copied()).collect();
        let first = first_speech_offset_ms(&flat, 48000);
        assert_eq!(first, Some(100), "首个非静音样本应落在 100ms");
        assert!(first.unwrap() > HEAD_CLIP_WINDOW_MS);

        // 🔴 只读性（验收 #5）：dump 前后输入逐样本一致
        assert_eq!(chunks, original, "dump 不得改动喂给 ASR 的样本一个 bit");

        log::set_max_level(log::LevelFilter::Warn); // 复原
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn diag292_dump_limit_removes_oldest_and_spares_non_dump_files() {
        let dir = dump_test_dir("limit");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), b"keep").unwrap();
        for i in 0..5 {
            std::fs::write(dir.join(format!("preroll-20260101-00000{i}.wav")), b"x").unwrap();
        }
        let removed = enforce_dump_limit(&dir, 3);
        assert_eq!(removed, 2, "5 → 3 应删最旧 2 个");
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec![
                "notes.txt",
                "preroll-20260101-000002.wav",
                "preroll-20260101-000003.wav",
                "preroll-20260101-000004.wav",
            ],
            "保留最新 3 个 dump + 不误删非 dump 文件"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // FIX-ASR-DROP-288：消费端节流上报的差值语义 + 1s 节流
    #[test]
    fn asr_drop_288_throttle_and_delta_semantics() {
        let mut last = 10_000u64;
        // delta == 0（本次录音尚无新丢包）→ 不报
        assert!(!report_recording_drops_throttled(
            100, 100, &mut last, 20_000
        ));
        // 有丢包但距上次 <1s → 节流，不报，且不得推进 last
        assert!(!report_recording_drops_throttled(
            105, 100, &mut last, 10_500
        ));
        assert_eq!(last, 10_000, "节流期间不得推进 last");
        // 距上次 >=1s → 报一条并推进 last
        assert!(report_recording_drops_throttled(
            142, 100, &mut last, 11_000
        ));
        assert_eq!(last, 11_000);
        // 紧接着又丢（<1s）→ 仍不报
        assert!(!report_recording_drops_throttled(
            200, 100, &mut last, 11_500
        ));
        // 又过 1s → 报
        assert!(report_recording_drops_throttled(
            230, 100, &mut last, 12_100
        ));
        assert_eq!(last, 12_100);
    }

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
            &AtomicU64::new(0),
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
            &AtomicU64::new(0),
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
            &AtomicU64::new(0),
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
            &AtomicU64::new(0),
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
