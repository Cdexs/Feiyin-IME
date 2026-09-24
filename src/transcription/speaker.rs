// SPEAKER-VERIFY-408A · 声纹模块（独立模块，暂不接入管线）
//
// 背景：BUILD-399 端测「背景人声被近场门放行」——本人与背景只差约 1dB，靠音量从原理上分不开
//（`collab/research/rt-perf-audit-403.md` / 404·404B 声纹选型 PoC）⇒ 改为「按声纹认人」。
//
// 本模块按主控设计（Gavin 拍板「声纹模型选 cam++双语版」「自动注册、只存本机、不误删」）实现：
// - 模型：`3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx`（CAM++ 中英，404B §B6 推荐）。
// - **自动注册、无设置界面**：从本人语音积累，**攒够 ≥12s 且 ≥3 段才就绪**；未就绪一律不剔除。
// - **只判 ≥2s 的段**；**只在高置信「非本人」时剔除**（`score < DROP_THR`），其余一律保留。
// - **日语 / 未知语言一律保留**（404B 未测日语，且中文→日语注册不通用）。
// - 声纹只存本机（调用方给路径），不含音频、不上传。
//
// 🔴 本单（408A）**不接入管线**（剪静音 / main.rs 是 408B）。模块顶部 `#![allow(dead_code)]`
//    是给「未接入」用的，**408B 接入后应移除本行**。
//
// ⚠️ 临时：`src/transcription/mod.rs` 的 `pub(crate) mod speaker;` 由主控在 406 交付后合入；
//    在此之前用 `src/bin/poc_speaker_408.rs` 的 `#[path]` 宿主跑 `cargo test --bin poc_speaker_408`。
#![allow(dead_code)] // 408B 接入管线后移除（本单仅新增文件、无调用方）

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sherpa_onnx::{SpeakerEmbeddingExtractor, SpeakerEmbeddingExtractorConfig};

/// 模型子目录（`<exe_dir>/models/<本名>`，DEC-011）。
pub(crate) const MODEL_SUBDIR: &str = "speaker-campplus-zh-en";
/// 模型文件名（404B §B6 推荐：CAM++ 中英，27MB，192 维）。
pub(crate) const MODEL_FILE: &str = "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx";
/// 声纹存档格式版本（结构变更时递增 ⇒ 旧档丢弃重建）。
const FORMAT_VERSION: u32 = 1;
/// 存档内记录的模型标识；与当前不符 ⇒ 丢弃重建（换模型后旧声纹不可用）。
const MODEL_TAG: &str = "campplus-zh_en-16k-common-advanced-v1";

/// 只判 ≥2s 的段（<2s 声纹不稳，404B 实测 1-2s 明显偏弱）。
pub(crate) const MIN_JUDGE_SECS: f32 = 2.0;
/// 就绪所需的最少有效语音（秒）与段数（Gavin 定「攒够 ≥12s 且 ≥3 段」）。
pub(crate) const ENROLL_MIN_SECS: f32 = 12.0;
pub(crate) const ENROLL_MIN_SEGS: u32 = 3;
/// 注册离群阈：候选段与候选质心 cos 低于此值即判为「混入的他人」而剔除（防开头把声纹学歪）。
pub(crate) const ENROLL_OUTLIER_THR: f32 = 0.5;
/// 漂移更新阈：仅 `score >= 此值` 的段参与 EMA 更新（高置信本人段）。
pub(crate) const UPDATE_THR: f32 = 0.75;
/// 漂移更新单段权重上限：防被单段（哪怕很长）带偏。
pub(crate) const UPDATE_MAX_WEIGHT: f32 = 0.25;
/// 剔除阈：`score < 此值` 且段 ≥2s 且语言受支持且已就绪 ⇒ 高置信非本人 ⇒ 剔除。
///
/// 依据 404B §B6：跨语言通用建议 0.45~0.65，取 **0.45 偏保守**（宁放过、不误删，守 390 教训）。
pub(crate) const DROP_THR: f32 = 0.45;

const SAMPLE_RATE: i32 = 16_000;

/// 余弦相似度（两侧各自 L2 归一化时等价于点积；此处对任意输入做归一化，鲁棒）。
pub(crate) fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0f32;
    let mut na = 0f32;
    let mut nb = 0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    dot / (na.sqrt() * nb.sqrt()).max(1e-9)
}

fn l2_normalize(v: &[f32]) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    v.iter().map(|x| x / n).collect()
}

fn centroid_of(embs: &[Vec<f32>]) -> Vec<f32> {
    let d = embs[0].len();
    let mut acc = vec![0f32; d];
    for e in embs {
        for (i, x) in e.iter().enumerate() {
            acc[i] += *x;
        }
    }
    l2_normalize(&acc)
}

/// 语言是否受声纹判定支持（404B 仅验中/英/韩）。`ja` / `None`（未知）一律不支持 ⇒ 保留。
fn lang_supported(lang: Option<&str>) -> bool {
    match lang {
        Some(l) => {
            let l = l.to_ascii_lowercase();
            l.starts_with("zh") || l.starts_with("en") || l.starts_with("ko")
        }
        None => false,
    }
}

/// 声纹提取器（持有 sherpa-onnx speaker-embedding 会话；线程数 1，404 实测 3s≈40ms）。
pub(crate) struct SpeakerVerifier {
    extractor: SpeakerEmbeddingExtractor,
}

impl SpeakerVerifier {
    /// 从模型目录加载；**缺模型 / 加载失败 ⇒ `None`**（功能静默关闭，各 warn 一次）。
    pub(crate) fn load(model_dir: &Path) -> Option<Self> {
        let path = model_dir.join(MODEL_SUBDIR).join(MODEL_FILE);
        if !path.is_file() {
            log::warn!(
                "speaker: model not found at {}, voiceprint disabled",
                path.display()
            );
            return None;
        }
        let cfg = SpeakerEmbeddingExtractorConfig {
            model: Some(path.to_string_lossy().to_string()),
            num_threads: 1,
            debug: false,
            provider: Some("cpu".to_string()),
        };
        match SpeakerEmbeddingExtractor::create(&cfg) {
            Some(extractor) => {
                log::info!("speaker: CAM++ extractor loaded at {}", path.display());
                Some(Self { extractor })
            }
            None => {
                log::warn!(
                    "speaker: failed to create extractor from {}",
                    path.display()
                );
                None
            }
        }
    }

    /// 提取声纹（16k 单声道 f32）并 L2 归一化；空音频 / 未就绪 ⇒ `None`。
    pub(crate) fn embed(&self, samples_16k: &[f32]) -> Option<Vec<f32>> {
        if samples_16k.is_empty() {
            return None;
        }
        let stream = self.extractor.create_stream()?;
        stream.accept_waveform(SAMPLE_RATE, samples_16k);
        stream.input_finished();
        if !self.extractor.is_ready(&stream) {
            return None;
        }
        self.extractor.compute(&stream).map(|v| l2_normalize(&v))
    }
}

/// 段判定结论（保守：只有 `DropNonUser` 才剔除内容）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SegVerdict {
    /// 段 < [`MIN_JUDGE_SECS`] ⇒ 保留（不判）。
    KeepShort,
    /// 声纹未就绪（积累不足）⇒ 保留。
    KeepNotReady,
    /// 语言不受支持（日语 / 未知）⇒ 保留。
    KeepLanguage,
    /// 已就绪且像本人（score ≥ [`DROP_THR`]）⇒ 保留（附 score，供漂移/调参）。
    KeepUser(f32),
    /// 已就绪且高置信非本人（score < [`DROP_THR`]）⇒ 剔除（附 score）。
    DropNonUser(f32),
}

/// 判定：段 ≥2s + 语言受支持 + 声纹就绪 + 声纹非本人 ⇒ `DropNonUser`；否则一律保留。
pub(crate) fn judge(
    vp: &Voiceprint,
    emb: Option<&[f32]>,
    secs: f32,
    lang: Option<&str>,
) -> SegVerdict {
    if secs < MIN_JUDGE_SECS {
        return SegVerdict::KeepShort;
    }
    if !lang_supported(lang) {
        return SegVerdict::KeepLanguage;
    }
    if !vp.is_ready() {
        return SegVerdict::KeepNotReady;
    }
    let Some(e) = emb else {
        return SegVerdict::KeepNotReady;
    };
    let score = cosine(&vp.centroid, e);
    if score < DROP_THR {
        SegVerdict::DropNonUser(score)
    } else {
        SegVerdict::KeepUser(score)
    }
}

/// 声纹存档（JSON；仅质心/时长/段数，**不含任何音频**）。
#[derive(Serialize, Deserialize)]
struct PersistedVp {
    version: u32,
    model: String,
    centroid: Vec<f32>,
    total_secs: f32,
    segments: u32,
}

/// 使用人声纹：就绪前积累候选、就绪后按时长加权 EMA 漂移更新。
pub(crate) struct Voiceprint {
    centroid: Vec<f32>,
    total_secs: f32,
    segments: u32,
    /// 就绪前的候选段（质心确定后清空）。
    candidates: Vec<(Vec<f32>, f32)>,
}

impl Default for Voiceprint {
    fn default() -> Self {
        Self {
            centroid: Vec::new(),
            total_secs: 0.0,
            segments: 0,
            candidates: Vec::new(),
        }
    }
}

impl Voiceprint {
    pub(crate) fn is_ready(&self) -> bool {
        !self.centroid.is_empty()
            && self.total_secs >= ENROLL_MIN_SECS
            && self.segments >= ENROLL_MIN_SEGS
    }

    /// 喂入一段「本人候选」声纹（`emb` 应已 L2 归一化或任意，内部归一化）。
    ///
    /// - 未就绪：入候选；若候选足以定稿（≥[`ENROLL_MIN_SECS`]/≥[`ENROLL_MIN_SEGS`]），
    ///   先算候选质心、剔除 `cos < [`ENROLL_OUTLIER_THR`]` 的离群段，剩余仍达标才就绪。
    /// - 已就绪：`score_if_ready`（缺省用 `cosine(质心, emb)`）≥[`UPDATE_THR`] 才按
    ///   `alpha = min(secs/(total+secs), UPDATE_MAX_WEIGHT)` 做 EMA：`c←normalize((1-α)c + α·emb)`。
    pub(crate) fn offer(&mut self, emb: &[f32], secs: f32, score_if_ready: Option<f32>) {
        if emb.is_empty() || secs <= 0.0 {
            return;
        }
        let emb = l2_normalize(emb);
        if self.is_ready() {
            let score = score_if_ready.unwrap_or_else(|| cosine(&self.centroid, &emb));
            if score >= UPDATE_THR {
                let alpha = (secs / (self.total_secs + secs).max(1e-6)).min(UPDATE_MAX_WEIGHT);
                for (c, e) in self.centroid.iter_mut().zip(emb.iter()) {
                    *c = *c * (1.0 - alpha) + *e * alpha;
                }
                self.centroid = l2_normalize(&self.centroid);
                self.total_secs += secs;
                self.segments += 1;
            }
        } else {
            self.candidates.push((emb, secs));
            self.try_finalize();
        }
    }

    /// 候选定稿：候选达标后算质心、剔除离群、复检（防开头混入他人）。
    fn try_finalize(&mut self) {
        let total: f32 = self.candidates.iter().map(|(_, s)| s).sum();
        if total < ENROLL_MIN_SECS || self.candidates.len() < ENROLL_MIN_SEGS as usize {
            return;
        }
        let embs: Vec<Vec<f32>> = self.candidates.iter().map(|(e, _)| e.clone()).collect();
        let c0 = centroid_of(&embs);
        let mut kept: Vec<(Vec<f32>, f32)> = Vec::new();
        for (e, s) in self.candidates.drain(..) {
            if cosine(&c0, &e) >= ENROLL_OUTLIER_THR {
                kept.push((e, s));
            }
        }
        let kept_total: f32 = kept.iter().map(|(_, s)| s).sum();
        if kept_total >= ENROLL_MIN_SECS && kept.len() >= ENROLL_MIN_SEGS as usize {
            self.centroid = centroid_of(&kept.iter().map(|(e, _)| e.clone()).collect::<Vec<_>>());
            self.total_secs = kept_total;
            self.segments = kept.len() as u32;
        } else {
            // 未达标：保留剔除后的候选继续积累（本次剔除的离群段已丢弃）。
            self.candidates = kept;
        }
    }

    /// 读档；版本 / 模型名不符或解析失败 ⇒ 空（丢弃重建）。
    pub(crate) fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(p) = serde_json::from_str::<PersistedVp>(&text) else {
            log::warn!(
                "speaker: voiceprint parse failed at {}, rebuilding",
                path.display()
            );
            return Self::default();
        };
        if p.version != FORMAT_VERSION || p.model != MODEL_TAG || p.centroid.is_empty() {
            log::info!(
                "speaker: voiceprint discarded (version={} model={} expected={})",
                p.version,
                p.model,
                MODEL_TAG
            );
            return Self::default();
        }
        Self {
            centroid: p.centroid,
            total_secs: p.total_secs,
            segments: p.segments,
            candidates: Vec::new(),
        }
    }

    /// 存档（best-effort；失败仅 warn，不影响运行）。
    pub(crate) fn save(&self, path: &Path) {
        if !self.is_ready() {
            return; // 未就绪不落盘（避免半成品档）
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let p = PersistedVp {
            version: FORMAT_VERSION,
            model: MODEL_TAG.to_string(),
            centroid: self.centroid.clone(),
            total_secs: self.total_secs,
            segments: self.segments,
        };
        match serde_json::to_string(&p) {
            Ok(s) => {
                if let Err(e) = std::fs::write(path, s) {
                    log::warn!("speaker: voiceprint save failed: {e}");
                }
            }
            Err(e) => log::warn!("speaker: voiceprint serialize failed: {e}"),
        }
    }

    /// 声纹默认存储路径（调用方通常用配置目录；408B 用 wordbook 同级）。
    pub(crate) fn default_path(config_dir: &Path) -> PathBuf {
        config_dir.join("voiceprint.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个与 `centroid=[1,0]` 余弦恰为 `c` 的单位向量（二维足够）。
    fn unit_with_cos(c: f32) -> Vec<f32> {
        let s = (1.0 - c * c).max(0.0).sqrt();
        vec![c, s]
    }
    fn vp_ready(centroid: Vec<f32>) -> Voiceprint {
        Voiceprint {
            centroid,
            total_secs: ENROLL_MIN_SECS,
            segments: ENROLL_MIN_SEGS,
            candidates: Vec::new(),
        }
    }

    #[test]
    fn cosine_basic() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0 < 1e-6);
        // 长度不等 / 空 ⇒ 0（不 panic）
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
        assert_eq!(cosine(&[], &[]), 0.0);
    }

    #[test]
    fn judge_short_is_kept() {
        let vp = vp_ready(vec![1.0, 0.0]);
        assert_eq!(
            judge(&vp, Some(&unit_with_cos(0.1)), 1.9, Some("zh")),
            SegVerdict::KeepShort
        );
        // 即便语言不支持，<2s 仍是 KeepShort（顺序：短段优先）。
        assert_eq!(judge(&vp, None, 0.5, Some("ja")), SegVerdict::KeepShort);
    }

    #[test]
    fn judge_not_ready_is_kept() {
        let vp = Voiceprint::default();
        assert!(!vp.is_ready());
        assert_eq!(
            judge(&vp, Some(&unit_with_cos(0.1)), 3.0, Some("zh")),
            SegVerdict::KeepNotReady
        );
    }

    #[test]
    fn judge_ja_or_unknown_language_is_kept() {
        let vp = vp_ready(vec![1.0, 0.0]);
        // 高置信非本人，但语言不支持 ⇒ 仍保留。
        assert_eq!(
            judge(&vp, Some(&unit_with_cos(0.1)), 3.0, Some("ja")),
            SegVerdict::KeepLanguage
        );
        assert_eq!(
            judge(&vp, Some(&unit_with_cos(0.1)), 3.0, None),
            SegVerdict::KeepLanguage
        );
    }

    #[test]
    fn judge_scores_drop_vs_keep() {
        let vp = vp_ready(vec![1.0, 0.0]);
        match judge(&vp, Some(&unit_with_cos(0.3)), 3.0, Some("zh")) {
            SegVerdict::DropNonUser(s) => assert!((s - 0.3).abs() < 1e-3),
            other => panic!("0.3 应 Drop，实测 {other:?}"),
        }
        match judge(&vp, Some(&unit_with_cos(0.6)), 3.0, Some("zh")) {
            SegVerdict::KeepUser(s) => assert!((s - 0.6).abs() < 1e-3),
            other => panic!("0.6 应 KeepUser，实测 {other:?}"),
        }
        // 恰好阈值：0.45 不算 Drop（>= DROP_THR 保留）。
        assert!(matches!(
            judge(&vp, Some(&unit_with_cos(DROP_THR)), 3.0, Some("zh")),
            SegVerdict::KeepUser(_)
        ));
    }

    #[test]
    fn enrollment_needs_min_secs_and_segs() {
        let e = l2_normalize(&[1.0, 0.0]);
        let mut vp = Voiceprint::default();
        // 2 段共 12s：段数不足。
        vp.offer(&e, 6.0, None);
        vp.offer(&e, 6.0, None);
        assert!(!vp.is_ready(), "2 段 < ENROLL_MIN_SEGS");
        // 3 段共 6s：时长不足。
        let mut vp2 = Voiceprint::default();
        vp2.offer(&e, 2.0, None);
        vp2.offer(&e, 2.0, None);
        vp2.offer(&e, 2.0, None);
        assert!(!vp2.is_ready(), "6s < ENROLL_MIN_SECS");
        // 3 段共 12s：就绪。
        let mut vp3 = Voiceprint::default();
        vp3.offer(&e, 4.0, None);
        vp3.offer(&e, 4.0, None);
        vp3.offer(&e, 4.0, None);
        assert!(vp3.is_ready(), "12s/3 段应就绪");
    }

    #[test]
    fn enrollment_outlier_removed_and_still_ready() {
        let e = l2_normalize(&[1.0, 0.0]);
        let mut vp = Voiceprint::default();
        // 4 段本人（各 5s）+ 1 段离群（他人，与本人 cos≈0，4s）。
        vp.offer(&e, 5.0, None);
        vp.offer(&e, 5.0, None);
        vp.offer(&e, 5.0, None);
        vp.offer(&unit_with_cos(0.0), 4.0, None);
        vp.offer(&e, 5.0, None); // 触发定稿
        assert!(vp.is_ready(), "剔除离群后 4×5s 应就绪");
        // 质心仍接近本人。
        assert!(cosine(&vp.centroid, &e) > 0.99, "离群段不应污染质心");
    }

    #[test]
    fn enrollment_outlier_removed_but_not_ready() {
        let e = l2_normalize(&[1.0, 0.0]);
        let mut vp = Voiceprint::default();
        // 本人仅 2 段 ×3s =6s，加 1 段离群 ⇒ 剔除后 6s/2 段，不达标。
        vp.offer(&e, 3.0, None);
        vp.offer(&unit_with_cos(0.0), 5.0, None);
        vp.offer(&e, 3.0, None);
        assert!(!vp.is_ready(), "剔除离群后 6s/2 段 < 门槛");
    }

    #[test]
    fn drift_low_score_no_update_high_score_updates_with_cap() {
        let e = l2_normalize(&[1.0, 0.0]);
        let mut vp = vp_ready(e.clone());
        let before = vp.centroid.clone();
        // 低分（<UPDATE_THR）不更新，也不增时长/段数。
        vp.offer(&unit_with_cos(0.5), 10.0, Some(0.5));
        assert_eq!(vp.centroid, before, "低分段不得更新质心");
        assert_eq!(vp.segments, ENROLL_MIN_SEGS);
        // 高分（≥UPDATE_THR）更新：质心朝新段移动。
        let target = unit_with_cos(0.95);
        vp.offer(&target, 6.0, Some(0.95));
        assert_eq!(vp.segments, ENROLL_MIN_SEGS + 1);
        let moved = cosine(&vp.centroid, &target) > cosine(&before, &target);
        assert!(moved, "高分段应把质心拉向自己");
        // 权重上限：就算喂一段超长音频，单次 α ≤ UPDATE_MAX_WEIGHT。
        let e2 = l2_normalize(&[1.0, 0.0]);
        let mut vp2 = vp_ready(e2.clone());
        let before2 = vp2.centroid.clone();
        vp2.offer(&unit_with_cos(0.0), 100_000.0, Some(1.0));
        let alpha_eff = 1.0 - cosine(&before2, &vp2.centroid); // 粗略：移动幅度受 α 限
        assert!(
            alpha_eff <= UPDATE_MAX_WEIGHT + 1e-3,
            "单段权重须≤上限，实测 {alpha_eff}"
        );
    }

    #[test]
    fn persisted_load_discards_on_model_mismatch() {
        let dir = std::env::temp_dir().join(format!(
            "voice-ime-spk-408a-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("voiceprint.json");
        // 好档：写后读回就绪。
        let mut vp = vp_ready(l2_normalize(&[1.0, 0.0]));
        vp.total_secs = 20.0;
        vp.save(&path);
        let back = Voiceprint::load(&path);
        assert!(back.is_ready(), "正常档读回应就绪");
        assert!((back.total_secs - 20.0).abs() < 1e-3);
        // 模型名不符 ⇒ 丢弃（空）。
        let bad = format!(
            "{{\"version\":{},\"model\":\"OLD\",\"centroid\":[1.0,0.0],\"total_secs\":20.0,\"segments\":5}}",
            FORMAT_VERSION
        );
        std::fs::write(&path, bad).unwrap();
        assert!(!Voiceprint::load(&path).is_ready(), "模型名不符应丢弃");
        // 版本不符 ⇒ 丢弃。
        std::fs::write(&path, "{\"version\":999,\"model\":\"campplus-zh_en-16k-common-advanced-v1\",\"centroid\":[1.0,0.0],\"total_secs\":20.0,\"segments\":5}").unwrap();
        assert!(!Voiceprint::load(&path).is_ready(), "版本不符应丢弃");
        // 坏 JSON ⇒ 丢弃不 panic。
        std::fs::write(&path, "not json").unwrap();
        assert!(!Voiceprint::load(&path).is_ready());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `#[ignore]` 真模型：加载 + embed 正常（本人 vs 他人 score），贴耗时。
    /// 运行：`cargo test --bin poc_speaker_408 -- --ignored --nocapture spk408_real_model`
    #[test]
    #[ignore = "requires CAM++ model + wav; cargo test --bin poc_speaker_408 -- --ignored --nocapture spk408_real_model"]
    fn spk408_real_model() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        let Some(v) = SpeakerVerifier::load(&model_dir) else {
            eprintln!("skip: 模型不在位（{}）", model_dir.display());
            return;
        };
        let read = |p: &Path| -> Option<Vec<f32>> {
            let w = sherpa_onnx::Wave::read(p.to_str()?)?;
            Some(w.samples().to_vec())
        };
        let g = root.join("collab/research/audio-real-gavin/processed");
        let g1 = read(&g.join("para1.wav")).expect("para1");
        let g2 = read(&g.join("para2.wav")).expect("para2");
        let others = [
            root.join("models/speaker-408-scratch/fangjun-test-sr-1.wav"),
            root.join("models/speaker-408-scratch/leijun-test-sr-1.wav"),
            root.join("models/speaker-408-scratch/liudehua-test-sr-1.wav"),
        ];
        let t = std::time::Instant::now();
        let e1 = v.embed(&g1).expect("embed g1");
        let e2 = v.embed(&g2).expect("embed g2");
        let dt = t.elapsed().as_secs_f64() * 1000.0;
        let self_cos = cosine(&e1, &e2);
        println!(
            "\n[408A] dim={} 本人 para1 vs para2 cos={:.3}  (embed×2 {:.0}ms)",
            e1.len(),
            self_cos,
            dt
        );
        for o in &others {
            if let Some(s) = read(o) {
                if let Some(e) = v.embed(&s) {
                    println!(
                        "  他人 {} vs 本人 para1 cos={:.3}",
                        o.file_name().unwrap().to_string_lossy(),
                        cosine(&e1, &e)
                    );
                }
            }
        }
        assert!(self_cos > DROP_THR, "本人自相似应高于剔除阈");
    }
}
