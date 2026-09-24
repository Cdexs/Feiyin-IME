// SPEAKER-VERIFY-408 · 声纹模块（408A 独立模块 + 408B 接入本地实时路B）
//
// 背景：BUILD-399 端测「背景人声被近场门放行」——本人与背景只差约 1dB，靠音量从原理上分不开
//（`collab/research/rt-perf-audit-403.md` / 404·404B 声纹选型 PoC）⇒ 改为「按声纹认人」。
//
// 主控设计（Gavin：「声纹模型选 cam++双语版」「自动注册、只存本机、不误删」
//「语音识别模型给出的标签是哪个语言，就用对应语言版本的声纹存档来比照」）：
// - 模型：`3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx`（CAM++ 中英）。
// - **先解码、后判定**：先按现有流程出首解（不剔除）→ 取 1.7B 语种前缀得 L（无前缀按字符集粗判）；
//   L=ja ｜ L 未知 ｜ L 档未就绪 ⇒ **不剔除**，直接用首解结果；否则用 **L 档**声纹逐段判定
//   （≥2s、score < `DROP_THR` 剔）→ 有段被剔 ⇒ 用**剔除后的 ranges** 重解一次并采用；
//   无段被剔 ⇒ 直接用首解结果。
// - **按语种分档**的声纹存档（同一文件）；自动注册：该档 ≥12s 且 ≥3 段才就绪；未就绪不剔。
// - **跨语种保守**：新语种注册时，若已有其它就绪档，候选与之最高分 < `NEW_LANG_MIN_SCORE` ⇒ 不收
//   （多半是背景人，防学成新语种声纹）。
// - 声纹只存本机（`<exe>/voiceprint.bin`），不含音频、不上传。
//
// ⚠️ 408A 的单档存档从未发布 ⇒ 直接按**多语种档案**格式实现；存档带版本号，升级迁移不丢弃，
//    仅当**换了声纹模型**（向量空间不同）才无法迁移、需重建（日志说明）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sherpa_onnx::{SpeakerEmbeddingExtractor, SpeakerEmbeddingExtractorConfig};

/// 模型子目录（`<exe_dir>/models/<本名>`，DEC-011）。
pub(crate) const MODEL_SUBDIR: &str = "speaker-campplus-zh-en";
/// 模型文件名（404B §B6 推荐：CAM++ 中英，27MB，192 维）。
pub(crate) const MODEL_FILE: &str = "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx";
/// 声纹存档格式版本（结构变更时递增）。
///
/// v1 → **v2**（408B）：单档 → **按语种分档**（`profiles: {lang → profile}`）；v1 档迁移到 `und`（不丢弃）。
const FORMAT_VERSION: u32 = 2;
/// 存档内记录的模型标识；与当前不符 ⇒ 丢弃重建（换模型后向量空间不同，旧声纹不可用）。
const MODEL_TAG: &str = "campplus-zh_en-16k-common-advanced-v1";
/// v1 单档迁移到该语种键（语种未知）。
const MIGRATED_LANG: &str = "und";

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
/// 剔除阈：最高分 < 此值且段 ≥2s 且该语种档就绪 ⇒ 高置信非本人 ⇒ 剔除。
///
/// 依据 404B §B6：跨语言通用建议 0.45~0.65，取 **0.45 偏保守**（宁放过、不误删，守 390 教训）。
pub(crate) const DROP_THR: f32 = 0.45;
/// 新语种注册闸（408B）：已有其它就绪档时，新语种候选段与**已有就绪档**最高分须 ≥ 此值，
/// 否则多半是背景人 ⇒ 不收入候选（防把别人学成新语种声纹）。
pub(crate) const NEW_LANG_MIN_SCORE: f32 = 0.3;

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
#[allow(dead_code)] // 408A `judge` 契约 API；生产预解码走 `judge_voiceprint`（跨档最高分）
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
    /// 声纹未就绪（该语种档未就绪 / 提取失败 / 无 emb）⇒ 保留。
    KeepNotReady,
    /// 语言不受支持（日语 / 未知）⇒ 保留。
    #[allow(dead_code)] // 408A `judge` 契约；生产预解码走 `judge_voiceprint`
    KeepLanguage,
    /// 已就绪且像本人（最高分 ≥ [`DROP_THR`]）⇒ 保留（附 score）。
    KeepUser(f32),
    /// 已绪且高置信非本人（最高分 < [`DROP_THR`]）⇒ 剔除（附 score）。
    DropNonUser(f32),
}

/// SPEAKER-VERIFY-408B：**语言无关**的判定核心（`<2s → 无就绪档/无 emb → 阈值`）。
///
/// 取**本人所有已就绪语种档的最高分**；至少一档就绪且最高分 < [`DROP_THR`] 才剔除。
/// 语种选择交给「最高分」自动完成（解码前语种未知）。
pub(crate) fn judge_voiceprint(vp: &Voiceprint, emb: Option<&[f32]>, secs: f32) -> SegVerdict {
    if secs < MIN_JUDGE_SECS {
        return SegVerdict::KeepShort;
    }
    let Some(e) = emb else {
        return SegVerdict::KeepNotReady;
    };
    let Some(score) = vp.max_score_ready(e) else {
        return SegVerdict::KeepNotReady;
    };
    if score < DROP_THR {
        SegVerdict::DropNonUser(score)
    } else {
        SegVerdict::KeepUser(score)
    }
}

/// 判定：段 ≥2s + 语言受支持 + **该语种档**就绪 + 非本人 ⇒ `DropNonUser`；否则一律保留。
///
/// 🔴 **短段门（`<2s`）最优先**（408A 契约：1.9s+ja ⇒ `KeepShort`）⇒ 语言门仅对 `≥2s` 生效；
/// 判定逻辑收敛到 [`judge_voiceprint`]（单一出处）。
#[allow(dead_code)] // 408A 契约 API（保留供未来按已知语言判定）；408B 生产预解码走 `judge_voiceprint`
pub(crate) fn judge(
    vp: &Voiceprint,
    emb: Option<&[f32]>,
    secs: f32,
    lang: Option<&str>,
) -> SegVerdict {
    if secs >= MIN_JUDGE_SECS && !lang_supported(lang) {
        return SegVerdict::KeepLanguage;
    }
    judge_voiceprint(vp, emb, secs)
}

/// 单语种声纹档。
#[derive(Default, Clone)]
struct LangProfile {
    centroid: Vec<f32>,
    total_secs: f32,
    segments: u32,
    /// 就绪前的候选段（质心确定后清空）。
    candidates: Vec<(Vec<f32>, f32)>,
}

impl LangProfile {
    fn is_ready(&self) -> bool {
        !self.centroid.is_empty()
            && self.total_secs >= ENROLL_MIN_SECS
            && self.segments >= ENROLL_MIN_SEGS
    }
}

/// 使用人声纹：**按语种分档**（每种语言一份质心/时长/段数/就绪态）。
#[derive(Default)]
pub(crate) struct Voiceprint {
    profiles: BTreeMap<String, LangProfile>,
}

impl Voiceprint {
    /// 任一语种档已就绪。
    pub(crate) fn has_any_ready(&self) -> bool {
        self.profiles.values().any(|p| p.is_ready())
    }

    /// 该语种是否已有**已就绪**档。
    pub(crate) fn lang_ready(&self, lang: &str) -> bool {
        self.profiles.get(lang).is_some_and(|p| p.is_ready())
    }

    /// 该语种已就绪档的质心（未就绪 / 无该档 ⇒ `None`）。测试与诊断用。
    #[allow(dead_code)]
    pub(crate) fn ready_centroid(&self, lang: &str) -> Option<&[f32]> {
        self.profiles
            .get(lang)
            .filter(|p| p.is_ready())
            .map(|p| p.centroid.as_slice())
    }

    /// 各已就绪档时长合计（日志/节流用）。
    pub(crate) fn total_ready_secs(&self) -> f32 {
        self.profiles
            .values()
            .filter(|p| p.is_ready())
            .map(|p| p.total_secs)
            .sum()
    }

    /// 对**所有已就绪档**取最高余弦（新语种注册闸用）；无就绪档 ⇒ `None`。
    fn max_score_ready(&self, emb: &[f32]) -> Option<f32> {
        let mut best: Option<f32> = None;
        for p in self.profiles.values().filter(|p| p.is_ready()) {
            let s = cosine(&p.centroid, emb);
            best = Some(best.map_or(s, |b| b.max(s)));
        }
        best
    }

    /// 喂入一段「本人候选」声纹（`emb` 任意，内部归一化），按 `lang` 进入对应档。
    ///
    /// - 该档已就绪：`score >= UPDATE_THR` 才按 `alpha = min(secs/(total+secs), UPDATE_MAX_WEIGHT)`
    ///   做 EMA 漂移；
    /// - 该档未就绪：作为该档注册候选（≥12s/≥3 段 + 离群剔除）；🔴 若**已有其它就绪档**，候选与之
    ///   最高分 < [`NEW_LANG_MIN_SCORE`] ⇒ **不收**（多半是背景人，防学成新语种声纹）；
    /// - `lang` 未知 / 空 ⇒ 不注册（无处归档）。
    pub(crate) fn offer(
        &mut self,
        emb: &[f32],
        secs: f32,
        score_if_ready: Option<f32>,
        lang: Option<&str>,
    ) {
        let Some(lang) = lang.filter(|l| !l.is_empty()) else {
            return;
        };
        if emb.is_empty() || secs <= 0.0 {
            return;
        }
        let emb = l2_normalize(emb);
        // 先算「已有其它就绪档的最高分」（entry 前借用，避免同时可变/不可变借用）。
        let best_ready = self.max_score_ready(&emb);
        let prof = self.profiles.entry(lang.to_string()).or_default();
        if prof.is_ready() {
            let score = score_if_ready.unwrap_or_else(|| cosine(&prof.centroid, &emb));
            if score >= UPDATE_THR {
                let alpha = (secs / (prof.total_secs + secs).max(1e-6)).min(UPDATE_MAX_WEIGHT);
                for (c, e) in prof.centroid.iter_mut().zip(emb.iter()) {
                    *c = *c * (1.0 - alpha) + *e * alpha;
                }
                prof.centroid = l2_normalize(&prof.centroid);
                prof.total_secs += secs;
                prof.segments += 1;
            }
        } else {
            // 新语种档注册：已有就绪档时须过「非背景人」闸。
            if let Some(b) = best_ready {
                if b < NEW_LANG_MIN_SCORE {
                    return;
                }
            }
            prof.candidates.push((emb, secs));
            Self::try_finalize(prof);
        }
    }

    /// 单档候选定稿：候选达标后算质心、剔除离群、复检（防开头混入他人）。
    fn try_finalize(prof: &mut LangProfile) {
        let total: f32 = prof.candidates.iter().map(|(_, s)| s).sum();
        if total < ENROLL_MIN_SECS || prof.candidates.len() < ENROLL_MIN_SEGS as usize {
            return;
        }
        let embs: Vec<Vec<f32>> = prof.candidates.iter().map(|(e, _)| e.clone()).collect();
        let c0 = centroid_of(&embs);
        let mut kept: Vec<(Vec<f32>, f32)> = Vec::new();
        for (e, s) in prof.candidates.drain(..) {
            if cosine(&c0, &e) >= ENROLL_OUTLIER_THR {
                kept.push((e, s));
            }
        }
        let kept_total: f32 = kept.iter().map(|(_, s)| s).sum();
        if kept_total >= ENROLL_MIN_SECS && kept.len() >= ENROLL_MIN_SEGS as usize {
            let ce: Vec<Vec<f32>> = kept.iter().map(|(e, _)| e.clone()).collect();
            prof.centroid = centroid_of(&ce);
            prof.total_secs = kept_total;
            prof.segments = kept.len() as u32;
        } else {
            // 未达标：保留剔除后的候选继续积累（本次剔除的离群段已丢弃）。
            prof.candidates = kept;
        }
    }

    /// 读档；**模型名不符**（向量空间不同）或解析失败 ⇒ 空（丢弃重建，日志说明）。
    /// 版本升级（v1→v2）⇒ **迁移不丢弃**（v1 单档 → `und`）。
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
        if p.model != MODEL_TAG {
            log::warn!(
                "speaker: voiceprint model mismatch ({} vs {}); rebuild (vector space differs)",
                p.model,
                MODEL_TAG
            );
            return Self::default();
        }
        let mut vp = Self::default();
        if p.version >= FORMAT_VERSION {
            for (lang, pp) in p.profiles {
                if !pp.centroid.is_empty() {
                    vp.profiles.insert(
                        lang,
                        LangProfile {
                            centroid: pp.centroid,
                            total_secs: pp.total_secs,
                            segments: pp.segments,
                            candidates: Vec::new(),
                        },
                    );
                }
            }
        } else if !p.centroid.is_empty() {
            // v1 单档 ⇒ 迁移到 `und`，不丢弃。
            log::info!("speaker: migrating v1 voiceprint to per-language format ({MIGRATED_LANG})");
            vp.profiles.insert(
                MIGRATED_LANG.to_string(),
                LangProfile {
                    centroid: p.centroid,
                    total_secs: p.total_secs,
                    segments: p.segments,
                    candidates: Vec::new(),
                },
            );
        }
        vp
    }

    /// 存档（best-effort；失败仅 warn，不影响运行）。只落**已就绪**档。
    pub(crate) fn save(&self, path: &Path) {
        if !self.has_any_ready() {
            return; // 未就绪不落盘（避免半成品档）
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let profiles: BTreeMap<String, PersistedProfile> = self
            .profiles
            .iter()
            .filter(|(_, p)| p.is_ready())
            .map(|(k, p)| {
                (
                    k.clone(),
                    PersistedProfile {
                        centroid: p.centroid.clone(),
                        total_secs: p.total_secs,
                        segments: p.segments,
                    },
                )
            })
            .collect();
        let p = PersistedVp {
            version: FORMAT_VERSION,
            model: MODEL_TAG.to_string(),
            profiles,
            centroid: Vec::new(),
            total_secs: 0.0,
            segments: 0,
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
}

/// 单语种存档（JSON）。
#[derive(Serialize, Deserialize, Default)]
struct PersistedProfile {
    #[serde(default)]
    centroid: Vec<f32>,
    #[serde(default)]
    total_secs: f32,
    #[serde(default)]
    segments: u32,
}

/// 声纹存档（JSON；按语种分档；仅质心/时长/段数，**不含任何音频**）。
#[derive(Serialize, Deserialize)]
struct PersistedVp {
    version: u32,
    model: String,
    /// v2：按语种分档。
    #[serde(default)]
    profiles: BTreeMap<String, PersistedProfile>,
    // v1 遗留单档字段（仅用于迁移；v2 序列化为空）。
    #[serde(default)]
    centroid: Vec<f32>,
    #[serde(default)]
    total_secs: f32,
    #[serde(default)]
    segments: u32,
}

// ===========================================================================
// SPEAKER-VERIFY-408B：进程级状态 + 单窗判定（接入本地实时路B）
// ===========================================================================

/// 进程级声纹状态（懒加载一次；`saved_total_secs` 为节流写盘水位）。
struct ProcessVp {
    vp: Voiceprint,
    saved_total_secs: f32,
}

/// 进程级声纹（跨窗 / 跨录音复用）。
static VOICEPRINT: Mutex<Option<ProcessVp>> = Mutex::new(None);

thread_local! {
    /// 提取器**线程级**懒加载（仿 `LOCALRT_TRIM_VAD`）：外层 = 是否已尝试，内层 = 是否可用。
    static SPEAKER_EXTRACTOR: std::cell::RefCell<Option<Option<SpeakerVerifier>>> =
        std::cell::RefCell::new(None);
}

/// 声纹存档路径：`<exe 目录>/voiceprint.bin`（与 `wordbook.sqlite` 同目录约定）。
pub(crate) fn voiceprint_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voiceprint.bin")
}

/// 每区间判定明细（日志用）。
pub(crate) struct RangeJudgement {
    pub start: usize,
    pub end: usize,
    pub secs: f32,
    pub verdict: SegVerdict,
    pub score: f32,
}

/// 408B：延后到解码后落地的注册/漂移 offer（需窗口语种 L）。
pub(crate) struct PendingOffer {
    pub emb: Vec<f32>,
    pub secs: f32,
    pub score_if_ready: Option<f32>,
}

/// 408B 声纹过滤结果（解码前判定；供调用方 trim / 日志 / 406 放宽 / 注册）。
pub(crate) struct VoiceprintFilter {
    pub kept: Vec<(usize, usize)>,
    pub kept_secs: f32,
    pub dropped_secs: f32,
    /// 是否有任一已就绪档（无 ⇒ 全部保留）。
    pub any_ready: bool,
    pub enrolled_secs: f32,
    pub details: Vec<RangeJudgement>,
    /// 注册/漂移 offer（延后到解码后 `commit_voiceprint_offers`，带 L）。
    pub pending_offers: Vec<PendingOffer>,
}

fn secs_of(ranges: &[(usize, usize)]) -> f32 {
    ranges
        .iter()
        .map(|(s, e)| (e - s) as f32 / SAMPLE_RATE as f32)
        .sum()
}

fn verdict_score(v: SegVerdict) -> f32 {
    match v {
        SegVerdict::KeepUser(s) | SegVerdict::DropNonUser(s) => s,
        _ => 0.0,
    }
}

/// SPEAKER-VERIFY-408B ①：**解码前**判定 —— 每 ≥2s 区间与**本人所有已就绪档**逐一比、取最高分；
/// ≥一档就绪且最高分 < [`DROP_THR`] ⇒ 剔除（无就绪档 ⇒ 全部保留）。
///
/// 模型缺失 ⇒ 原样返回（功能静默关闭）。注册/漂移延后到解码后（需 L）⇒ 只收集本窗新片的
/// **保留**区间 `pending_offers`。
pub(crate) fn filter_ranges_by_voiceprint(
    samples: &[f32],
    ranges: &[(usize, usize)],
    new_slice_from: usize,
) -> VoiceprintFilter {
    if ranges.is_empty() {
        return VoiceprintFilter {
            kept: Vec::new(),
            kept_secs: 0.0,
            dropped_secs: 0.0,
            any_ready: false,
            enrolled_secs: 0.0,
            details: Vec::new(),
            pending_offers: Vec::new(),
        };
    }
    SPEAKER_EXTRACTOR.with(|cell| {
        let mut details: Vec<RangeJudgement> = Vec::new();
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(SpeakerVerifier::load(&super::model_dir()));
        }
        let Some(verifier) = slot.as_ref().and_then(|o| o.as_ref()) else {
            return VoiceprintFilter {
                kept: ranges.to_vec(),
                kept_secs: secs_of(ranges),
                dropped_secs: 0.0,
                any_ready: false,
                enrolled_secs: 0.0,
                details,
                pending_offers: Vec::new(),
            };
        };
        let mut guard = VOICEPRINT.lock().unwrap_or_else(|e| e.into_inner());
        let proc = guard.get_or_insert_with(|| ProcessVp {
            vp: Voiceprint::load(&voiceprint_path()),
            saved_total_secs: 0.0,
        });
        let mut verdicts: Vec<SegVerdict> = Vec::with_capacity(ranges.len());
        let mut embs: Vec<Option<Vec<f32>>> = Vec::with_capacity(ranges.len());
        for &(start, end) in ranges {
            let secs = (end - start) as f32 / SAMPLE_RATE as f32;
            if secs < MIN_JUDGE_SECS {
                verdicts.push(SegVerdict::KeepShort);
                embs.push(None);
                continue;
            }
            let emb = verifier.embed(&samples[start..end]);
            let v = judge_voiceprint(&proc.vp, emb.as_deref(), secs);
            verdicts.push(v);
            embs.push(emb);
        }
        let (kept, kept_secs, dropped_secs) = partition_ranges(ranges, &verdicts);
        let mut pending_offers: Vec<PendingOffer> = Vec::new();
        for (i, &(start, end)) in ranges.iter().enumerate() {
            if start < new_slice_from {
                continue;
            }
            let secs = (end - start) as f32 / SAMPLE_RATE as f32;
            if let Some(e) = embs[i].as_deref() {
                match verdicts[i] {
                    SegVerdict::KeepUser(s) => pending_offers.push(PendingOffer {
                        emb: e.to_vec(),
                        secs,
                        score_if_ready: Some(s),
                    }),
                    SegVerdict::KeepNotReady => pending_offers.push(PendingOffer {
                        emb: e.to_vec(),
                        secs,
                        score_if_ready: None,
                    }),
                    _ => {}
                }
            }
        }
        for (i, &(start, end)) in ranges.iter().enumerate() {
            let secs = (end - start) as f32 / SAMPLE_RATE as f32;
            details.push(RangeJudgement {
                start,
                end,
                secs,
                verdict: verdicts[i],
                score: verdict_score(verdicts[i]),
            });
        }
        VoiceprintFilter {
            kept,
            kept_secs,
            dropped_secs,
            any_ready: proc.vp.has_any_ready(),
            enrolled_secs: proc.vp.total_ready_secs(),
            details,
            pending_offers,
        }
    })
}

/// SPEAKER-VERIFY-408B ④：把本窗 offer 落地进 **L 档**（带窗口语种），并节流写盘。
///
/// 延后到解码后调用（L 来自模型前缀或解码文本字符集）。
pub(crate) fn commit_voiceprint_offers(offers: &[PendingOffer], lang: Option<&str>) {
    if offers.is_empty() {
        return;
    }
    let mut guard = VOICEPRINT.lock().unwrap_or_else(|e| e.into_inner());
    let proc = guard.get_or_insert_with(|| ProcessVp {
        vp: Voiceprint::load(&voiceprint_path()),
        saved_total_secs: 0.0,
    });
    for o in offers {
        proc.vp.offer(&o.emb, o.secs, o.score_if_ready, lang);
    }
    if proc.vp.has_any_ready() && proc.vp.total_ready_secs() - proc.saved_total_secs >= 5.0 {
        proc.vp.save(&voiceprint_path());
        proc.saved_total_secs = proc.vp.total_ready_secs();
    }
}

/// SPEAKER-VERIFY-408B ③：进程级声纹**该语种档是否已就绪**。须在**本窗 offer 之前**查询
///（用于「L 档未就绪 ⇒ 原 ranges 重解」）。
pub(crate) fn voiceprint_lang_ready(lang: &str) -> bool {
    let mut guard = VOICEPRINT.lock().unwrap_or_else(|e| e.into_inner());
    let proc = guard.get_or_insert_with(|| ProcessVp {
        vp: Voiceprint::load(&voiceprint_path()),
        saved_total_secs: 0.0,
    });
    proc.vp.lang_ready(lang)
}

/// 纯逻辑：按逐区间判定划分 keep / drop（`<2s` 的 `KeepShort` 由调用方先算好）。
///
/// 返回 `(保留区间, 保留秒数, 剔除秒数)`。判定数组缺位 ⇒ 保守保留。
pub(crate) fn partition_ranges(
    ranges: &[(usize, usize)],
    verdicts: &[SegVerdict],
) -> (Vec<(usize, usize)>, f32, f32) {
    let mut kept = Vec::new();
    let mut kept_secs = 0.0f32;
    let mut dropped_secs = 0.0f32;
    for (i, &(s, e)) in ranges.iter().enumerate() {
        let secs = (e - s) as f32 / SAMPLE_RATE as f32;
        match verdicts.get(i).copied().unwrap_or(SegVerdict::KeepShort) {
            SegVerdict::DropNonUser(_) => dropped_secs += secs,
            _ => {
                kept.push((s, e));
                kept_secs += secs;
            }
        }
    }
    (kept, kept_secs, dropped_secs)
}

// =====================================================================
// 测试
// =====================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn unit_with_cos(c: f32) -> Vec<f32> {
        let s = (1.0 - c * c).max(0.0).sqrt();
        vec![c, s]
    }
    /// 单语种已就绪声纹（`lang` 档）。
    fn vp_ready_lang(lang: &str, centroid: Vec<f32>) -> Voiceprint {
        let mut vp = Voiceprint::default();
        vp.profiles.insert(
            lang.to_string(),
            LangProfile {
                centroid,
                total_secs: ENROLL_MIN_SECS,
                segments: ENROLL_MIN_SEGS,
                candidates: Vec::new(),
            },
        );
        vp
    }
    fn vp_ready(centroid: Vec<f32>) -> Voiceprint {
        vp_ready_lang("zh", centroid)
    }

    #[test]
    fn cosine_basic() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0 < 1e-6);
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
        assert_eq!(judge(&vp, None, 0.5, Some("ja")), SegVerdict::KeepShort);
    }

    #[test]
    fn judge_not_ready_is_kept() {
        let vp = Voiceprint::default();
        assert!(!vp.has_any_ready());
        assert_eq!(
            judge(&vp, Some(&unit_with_cos(0.1)), 3.0, Some("zh")),
            SegVerdict::KeepNotReady
        );
    }

    #[test]
    fn judge_ja_or_unknown_language_is_kept() {
        let vp = vp_ready(vec![1.0, 0.0]);
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
        assert!(matches!(
            judge(&vp, Some(&unit_with_cos(DROP_THR)), 3.0, Some("zh")),
            SegVerdict::KeepUser(_)
        ));
    }

    #[test]
    fn enrollment_needs_min_secs_and_segs() {
        let e = l2_normalize(&[1.0, 0.0]);
        let mut vp = Voiceprint::default();
        vp.offer(&e, 6.0, None, Some("zh"));
        vp.offer(&e, 6.0, None, Some("zh"));
        assert!(!vp.has_any_ready(), "2 段 < ENROLL_MIN_SEGS");
        let mut vp2 = Voiceprint::default();
        vp2.offer(&e, 2.0, None, Some("zh"));
        vp2.offer(&e, 2.0, None, Some("zh"));
        vp2.offer(&e, 2.0, None, Some("zh"));
        assert!(!vp2.has_any_ready(), "6s < ENROLL_MIN_SECS");
        let mut vp3 = Voiceprint::default();
        vp3.offer(&e, 4.0, None, Some("zh"));
        vp3.offer(&e, 4.0, None, Some("zh"));
        vp3.offer(&e, 4.0, None, Some("zh"));
        assert!(vp3.has_any_ready(), "12s/3 段应就绪");
        assert!(vp3.lang_ready("zh") && !vp3.lang_ready("en"));
    }

    #[test]
    fn enrollment_outlier_removed_and_still_ready() {
        let e = l2_normalize(&[1.0, 0.0]);
        let mut vp = Voiceprint::default();
        vp.offer(&e, 5.0, None, Some("zh"));
        vp.offer(&e, 5.0, None, Some("zh"));
        vp.offer(&e, 5.0, None, Some("zh"));
        vp.offer(&unit_with_cos(0.0), 4.0, None, Some("zh"));
        vp.offer(&e, 5.0, None, Some("zh"));
        assert!(vp.has_any_ready());
        assert!(cosine(vp.ready_centroid("zh").unwrap(), &e) > 0.99);
    }

    #[test]
    fn enrollment_outlier_removed_but_not_ready() {
        let e = l2_normalize(&[1.0, 0.0]);
        let mut vp = Voiceprint::default();
        vp.offer(&e, 3.0, None, Some("zh"));
        vp.offer(&unit_with_cos(0.0), 5.0, None, Some("zh"));
        vp.offer(&e, 3.0, None, Some("zh"));
        assert!(!vp.has_any_ready(), "剔除离群后 6s/2 段 < 门槛");
    }

    #[test]
    fn drift_low_score_no_update_high_score_updates_with_cap() {
        let e = l2_normalize(&[1.0, 0.0]);
        let mut vp = vp_ready(e.clone());
        let before = vp.profiles.get("zh").unwrap().centroid.clone();
        vp.offer(&unit_with_cos(0.5), 10.0, Some(0.5), Some("zh"));
        assert_eq!(vp.profiles.get("zh").unwrap().centroid, before);
        assert_eq!(vp.profiles.get("zh").unwrap().segments, ENROLL_MIN_SEGS);
        let target = unit_with_cos(0.95);
        vp.offer(&target, 6.0, Some(0.95), Some("zh"));
        assert_eq!(vp.profiles.get("zh").unwrap().segments, ENROLL_MIN_SEGS + 1);
        let moved = cosine(vp.profiles.get("zh").unwrap().centroid.as_slice(), &target)
            > cosine(&before, &target);
        assert!(moved, "高分段应把质心拉向自己");
        let e2 = l2_normalize(&[1.0, 0.0]);
        let mut vp2 = vp_ready(e2.clone());
        let before2 = vp2.profiles.get("zh").unwrap().centroid.clone();
        vp2.offer(&unit_with_cos(0.0), 100_000.0, Some(1.0), Some("zh"));
        let alpha_eff = 1.0
            - cosine(
                &before2,
                vp2.profiles.get("zh").unwrap().centroid.as_slice(),
            );
        assert!(alpha_eff <= UPDATE_MAX_WEIGHT + 1e-3);
    }

    #[test]
    fn persisted_load_discards_on_model_mismatch() {
        let dir = std::env::temp_dir().join(format!(
            "voice-ime-spk-408b-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("voiceprint.json");
        let mut vp = vp_ready(l2_normalize(&[1.0, 0.0]));
        vp.profiles.get_mut("zh").unwrap().total_secs = 20.0;
        vp.save(&path);
        let back = Voiceprint::load(&path);
        assert!(back.has_any_ready() && back.lang_ready("zh"));
        let bad = format!(
            "{{\"version\":{FORMAT_VERSION},\"model\":\"OLD\",\"profiles\":{{\"zh\":{{\"centroid\":[1.0,0.0],\"total_secs\":20.0,\"segments\":5}}}}}}"
        );
        std::fs::write(&path, bad).unwrap();
        assert!(!Voiceprint::load(&path).has_any_ready(), "模型名不符应丢弃");
        std::fs::write(&path, "not json").unwrap();
        assert!(!Voiceprint::load(&path).has_any_ready());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `#[ignore]` 真模型：加载 + embed 正常（本人 vs 他人 score），贴耗时。
    #[test]
    #[ignore = "requires CAM++ model + wav; cargo test --bin feiyin-ime -- --ignored --nocapture spk408_real_model"]
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
            "\n[408] dim={} 本人 para1 vs para2 cos={:.3}  (embed×2 {:.0}ms)",
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

    /// `#[ignore]` 真模型：注册本人声纹后，**本人段保留、他人段被剔**（408B 判定链）。
    /// 运行：`cargo test --bin feiyin-ime -- --ignored --nocapture spk408b_real_window`
    #[test]
    #[ignore = "requires CAM++ model + wav; cargo test --bin feiyin-ime -- --ignored --nocapture spk408b_real_window"]
    fn spk408b_real_window() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let Some(v) = SpeakerVerifier::load(&root.join("models")) else {
            eprintln!("skip: 模型不在位");
            return;
        };
        let read = |p: &Path| {
            sherpa_onnx::Wave::read(p.to_str().unwrap_or("")).map(|w| w.samples().to_vec())
        };
        let g = root.join("collab/research/audio-real-gavin/processed");
        let self_a = read(&g.join("para1.wav")).expect("para1");
        let self_b = read(&g.join("para2.wav")).expect("para2");
        let other =
            read(&root.join("models/speaker-408-scratch/leijun-test-sr-1.wav")).expect("leijun");
        let (ea, eb, eo) = (
            v.embed(&self_a).expect("embed self_a"),
            v.embed(&self_b).expect("embed self_b"),
            v.embed(&other).expect("embed other"),
        );
        // 注册本人（zh 档）。
        let mut vp = Voiceprint::default();
        for _ in 0..2 {
            vp.offer(&ea, self_a.len() as f32 / 16000.0, None, Some("zh"));
            vp.offer(&eb, self_b.len() as f32 / 16000.0, None, Some("zh"));
        }
        assert!(vp.lang_ready("zh"), "本人注册应就绪");
        // 判定：本人 KeepUser、他人 DropNonUser。
        let keep_self = matches!(
            judge_voiceprint(&vp, Some(&ea), 3.0),
            SegVerdict::KeepUser(_)
        );
        let drop_other = matches!(
            judge_voiceprint(&vp, Some(&eo), 3.0),
            SegVerdict::DropNonUser(_)
        );
        println!(
            "\n[408B] self_cos={:.3} other_cos={:.3}",
            vp.max_score_ready(&ea).unwrap(),
            vp.max_score_ready(&eo).unwrap()
        );
        assert!(keep_self, "本人段应保留");
        assert!(drop_other, "他人段应剔除");
        // 区间划分：本人 + 他人 ⇒ 仅他人被剔。
        let ranges = [
            (0usize, self_a.len()),
            (self_a.len(), self_a.len() + other.len()),
        ];
        let verdicts = [
            judge_voiceprint(&vp, Some(&ea), self_a.len() as f32 / 16000.0),
            judge_voiceprint(&vp, Some(&eo), other.len() as f32 / 16000.0),
        ];
        let (kept, _, dropped) = partition_ranges(&ranges, &verdicts);
        assert_eq!(kept, vec![(0, self_a.len())]);
        assert!(dropped > 0.0);
    }
}

// =====================================================================
// TEST-SYNC-408A（阶段三 · 非作者护栏，coder-2）—— 已按 408B 多语种档适配
// =====================================================================
#[cfg(test)]
mod testsync408a_tests {
    use super::*;

    fn unit(axis: usize, dim: usize) -> Vec<f32> {
        let mut v = vec![0f32; dim];
        v[axis] = 1.0;
        v
    }
    fn at_cos(c: f32) -> Vec<f32> {
        vec![c, (1.0 - c * c).max(0.0).sqrt()]
    }
    fn ready_lang(lang: &str, centroid: Vec<f32>) -> Voiceprint {
        let mut vp = Voiceprint::default();
        vp.profiles.insert(
            lang.to_string(),
            LangProfile {
                centroid,
                total_secs: ENROLL_MIN_SECS,
                segments: ENROLL_MIN_SEGS,
                candidates: Vec::new(),
            },
        );
        vp
    }
    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "voice-ime-spk-sync408-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn ts408a_judge_short_beats_all() {
        let vp = ready_lang("zh", unit(0, 2));
        assert_eq!(
            judge(&vp, Some(&at_cos(0.0)), 1.9, Some("ja")),
            SegVerdict::KeepShort
        );
        assert_eq!(
            judge(&Voiceprint::default(), None, 1.5, Some("ja")),
            SegVerdict::KeepShort
        );
    }

    #[test]
    fn ts408a_judge_exact_2s_enters_judgment() {
        let vp = ready_lang("zh", unit(0, 2));
        assert!(matches!(
            judge(&vp, Some(&at_cos(0.3)), MIN_JUDGE_SECS, Some("zh")),
            SegVerdict::DropNonUser(_)
        ));
        assert_eq!(
            judge(&vp, Some(&at_cos(0.3)), MIN_JUDGE_SECS - 0.001, Some("zh")),
            SegVerdict::KeepShort
        );
    }

    #[test]
    fn ts408a_judge_language_beats_readiness() {
        let d = Voiceprint::default();
        assert_eq!(
            judge(&d, Some(&at_cos(0.0)), 3.0, Some("ja")),
            SegVerdict::KeepLanguage
        );
        assert_eq!(
            judge(&d, Some(&at_cos(0.0)), 3.0, None),
            SegVerdict::KeepLanguage
        );
        assert_eq!(
            judge(&d, Some(&at_cos(0.0)), 3.0, Some("zh")),
            SegVerdict::KeepNotReady
        );
    }

    #[test]
    fn ts408a_judge_exact_drop_thr_is_kept() {
        let vp = ready_lang("zh", unit(0, 2));
        let at_thr = at_cos(DROP_THR);
        assert!(cosine(vp.ready_centroid("zh").unwrap(), &at_thr) >= DROP_THR);
        assert!(matches!(
            judge(&vp, Some(&at_thr), 3.0, Some("zh")),
            SegVerdict::KeepUser(_)
        ));
        assert!(matches!(
            judge(&vp, Some(&at_cos(DROP_THR - 0.01)), 3.0, Some("zh")),
            SegVerdict::DropNonUser(_)
        ));
    }

    #[test]
    fn ts408a_judge_missing_embedding_keeps() {
        let vp = ready_lang("zh", unit(0, 2));
        assert_eq!(judge(&vp, None, 3.0, Some("zh")), SegVerdict::KeepNotReady);
    }

    #[test]
    fn ts408a_enroll_drops_outlier_still_ready() {
        let e = unit(0, 2);
        let o = unit(1, 2);
        let mut vp = Voiceprint::default();
        vp.offer(&e, 4.0, None, Some("zh"));
        vp.offer(&e, 4.0, None, Some("zh"));
        vp.offer(&o, 4.0, None, Some("zh"));
        vp.offer(&e, 4.0, None, Some("zh"));
        assert!(vp.has_any_ready());
        assert!(cosine(vp.ready_centroid("zh").unwrap(), &e) > 0.99);
        assert!(matches!(
            judge(&vp, Some(&o), 3.0, Some("zh")),
            SegVerdict::DropNonUser(_)
        ));
        assert!(matches!(
            judge(&vp, Some(&e), 3.0, Some("zh")),
            SegVerdict::KeepUser(_)
        ));
    }

    #[test]
    fn ts408a_enroll_two_self_two_others_not_ready() {
        let e = unit(0, 3);
        let mut vp = Voiceprint::default();
        vp.offer(&e, 3.0, None, Some("zh"));
        vp.offer(&unit(1, 3), 3.0, None, Some("zh"));
        vp.offer(&unit(2, 3), 3.0, None, Some("zh"));
        vp.offer(&e, 3.0, None, Some("zh"));
        assert!(!vp.has_any_ready(), "剔除两他人后 6s/2 段 < 门槛");
    }

    #[test]
    fn ts408a_enroll_all_same_other_is_known_limitation() {
        let o = unit(1, 2);
        let mut vp = Voiceprint::default();
        vp.offer(&o, 5.0, None, Some("zh"));
        vp.offer(&o, 5.0, None, Some("zh"));
        vp.offer(&o, 5.0, None, Some("zh"));
        assert!(
            vp.has_any_ready(),
            "全为同一他人 ⇒ 按设计会注册（已知局限）"
        );
    }

    #[test]
    fn ts408a_drift_low_score_bitwise_unchanged() {
        let mut vp = ready_lang("zh", unit(0, 2));
        let before = vp.profiles.get("zh").unwrap().centroid.clone();
        let (seg0, sec0) = {
            let p = vp.profiles.get("zh").unwrap();
            (p.segments, p.total_secs)
        };
        vp.offer(&at_cos(0.5), 8.0, Some(0.5), Some("zh"));
        vp.offer(&unit(1, 2), 100.0, Some(0.0), Some("zh"));
        let p = vp.profiles.get("zh").unwrap();
        assert_eq!(p.centroid, before, "低分段不得改变质心（逐位）");
        assert_eq!(p.segments, seg0);
        assert_eq!(p.total_secs, sec0);
    }

    #[test]
    fn ts408a_drift_single_step_bounded_by_max_weight() {
        let mut vp = ready_lang("zh", unit(0, 2));
        let before = vp.profiles.get("zh").unwrap().centroid.clone();
        vp.offer(&unit(1, 2), 100_000.0, Some(1.0), Some("zh"));
        let c = &vp.profiles.get("zh").unwrap().centroid;
        let n: f32 = c.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((n - 1.0).abs() < 1e-3, "EMA 后应仍单位范数，实测 {n}");
        let moved = cosine(&before, c);
        assert!(
            moved >= 0.94,
            "单步位移须受 0.25 权重限制，实测 cos={moved}"
        );
    }

    #[test]
    fn ts408a_drift_many_high_updates_converge_bounded() {
        let mut vp = ready_lang("zh", unit(0, 2));
        let c0 = vp.profiles.get("zh").unwrap().centroid.clone();
        let tgt = at_cos(0.8);
        let start_cos = cosine(&c0, &tgt);
        for _ in 0..50 {
            vp.offer(&tgt, 5.0, None, Some("zh"));
        }
        let c = &vp.profiles.get("zh").unwrap().centroid;
        let end_cos = cosine(c, &tgt);
        assert!(end_cos > start_cos && end_cos <= 1.0 + 1e-6);
        let n: f32 = c.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((n - 1.0).abs() < 1e-3);
    }

    #[test]
    fn ts408a_persist_roundtrip_and_discard_guards() {
        let dir = temp_dir("persist");
        let path = dir.join("voiceprint.json");
        let mut vp = ready_lang("zh", l2_normalize(&[0.6, 0.8]));
        vp.profiles.get_mut("zh").unwrap().total_secs = 33.0;
        vp.profiles.insert(
            "en".to_string(),
            LangProfile {
                centroid: l2_normalize(&[1.0, 0.0]),
                total_secs: 15.0,
                segments: 4,
                candidates: Vec::new(),
            },
        );
        vp.save(&path);
        let back = Voiceprint::load(&path);
        assert!(back.lang_ready("zh") && back.lang_ready("en"));
        assert!((back.profiles.get("zh").unwrap().total_secs - 33.0).abs() < 1e-3);

        let p2 = dir.join("not_ready.json");
        Voiceprint::default().save(&p2);
        assert!(!p2.exists(), "未就绪不得落盘");

        for bad in [
            "not json{",
            "{\"version\":2,\"model\":\"WRONG\",\"profiles\":{}}",
        ] {
            std::fs::write(&path, bad).unwrap();
            assert!(
                !Voiceprint::load(&path).has_any_ready(),
                "异常档应丢弃：{bad}"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ts408a_cross_user_b_not_absorbed() {
        let a = unit(0, 2);
        let b = unit(1, 2);
        let mut vp = ready_lang("zh", a.clone());
        let before = vp.profiles.get("zh").unwrap().centroid.clone();
        let (seg0, sec0) = {
            let p = vp.profiles.get("zh").unwrap();
            (p.segments, p.total_secs)
        };
        for _ in 0..20 {
            vp.offer(&b, 5.0, None, Some("zh"));
        }
        let p = vp.profiles.get("zh").unwrap();
        assert_eq!(p.centroid, before, "不得把 B 学进 A 的质心（逐位）");
        assert_eq!(p.segments, seg0);
        assert_eq!(p.total_secs, sec0);
        assert!(matches!(
            judge(&vp, Some(&b), 3.0, Some("zh")),
            SegVerdict::DropNonUser(_)
        ));
        assert!(matches!(
            judge(&vp, Some(&a), 3.0, Some("zh")),
            SegVerdict::KeepUser(_)
        ));
    }

    #[test]
    fn ts408a_cosine_edge_cases_no_panic() {
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[0.0, 0.0]), 0.0);
        assert_eq!(cosine(&[1.0, 2.0], &[1.0]), 0.0);
        assert_eq!(cosine(&[], &[1.0]), 0.0);
        assert!(cosine(&[2.0, 0.0], &[0.0, 3.0]).abs() < 1e-6);
        assert!((cosine(&[2.0, 0.0], &[5.0, 0.0]) - 1.0).abs() < 1e-6);
    }
}

// =====================================================================
// TEST-SYNC-408B（阶段三 · 非作者护栏，coder-2）：多语种档 + 跨语言 + 迁移
// =====================================================================
#[cfg(test)]
mod testsync408b_tests {
    use super::*;

    fn unit(axis: usize, dim: usize) -> Vec<f32> {
        let mut v = vec![0f32; dim];
        v[axis] = 1.0;
        v
    }

    /// 多语种各自就绪：zh 与 en 各 12s/3 段 ⇒ 两档均就绪，各自需分数判定。
    /// （en 与 zh 相似度取 0.5 ≥ `NEW_LANG_MIN_SCORE`，模拟同一使用者的双语声纹；否则会被新语种闸拒。）
    #[test]
    fn ts408b_multi_lang_each_ready() {
        let e_zh = unit(0, 2);
        let e_en = vec![0.5, (1.0f32 - 0.25).sqrt()];
        let mut vp = Voiceprint::default();
        for _ in 0..3 {
            vp.offer(&e_zh, 4.0, None, Some("zh"));
        }
        for _ in 0..3 {
            vp.offer(&e_en, 4.0, None, Some("en"));
        }
        assert!(vp.lang_ready("zh") && vp.lang_ready("en"));
        assert!(matches!(
            judge(&vp, Some(&e_zh), 3.0, Some("zh")),
            SegVerdict::KeepUser(_)
        ));
        assert!(matches!(
            judge(&vp, Some(&e_en), 3.0, Some("en")),
            SegVerdict::KeepUser(_)
        ));
    }

    /// 解码前判定取**所有已就绪档最高分**：仅有 zh 就绪，与 zh 正交的 en 段最高分 0 < 0.45 ⇒ Drop；
    /// 「L 档未就绪 ⇒ 保留」由 mod.rs 解码后保护（原 ranges 重解）兜底，不在此层。
    #[test]
    fn ts408b_predecode_uses_max_score_across_ready() {
        let e_zh = unit(0, 2);
        let e_other = unit(1, 2); // 与 zh cos=0
        let mut vp = Voiceprint::default();
        for _ in 0..3 {
            vp.offer(&e_zh, 4.0, None, Some("zh"));
        }
        assert!(vp.lang_ready("zh"));
        assert!(matches!(
            judge_voiceprint(&vp, Some(&e_other), 3.0),
            SegVerdict::DropNonUser(_)
        ));
        assert!(vp.max_score_ready(&e_other).unwrap() < DROP_THR);
    }

    /// 新语种注册闸：已有 zh 就绪；与 zh 最高分 <0.3 的 en 段 ⇒ 不进 en 候选。
    #[test]
    fn ts408b_background_not_learned_as_new_language() {
        let e_zh = unit(0, 2);
        let e_bg = unit(1, 2); // 与 zh cos=0（<0.3）
        let mut vp = Voiceprint::default();
        for _ in 0..3 {
            vp.offer(&e_zh, 4.0, None, Some("zh"));
        }
        vp.offer(&e_bg, 5.0, None, Some("en"));
        let n = vp
            .profiles
            .get("en")
            .map(|p| p.candidates.len())
            .unwrap_or(0);
        assert_eq!(n, 0, "背景段不得进新语种候选");
    }

    /// 新语种注册放行：有 zh 就绪，与 zh 最高分 ≥0.3 的 en 段 ⇒ 收进 en 候选。
    #[test]
    fn ts408b_bona_fide_new_language_candidate_accepted() {
        let e_zh = unit(0, 2);
        let e_en = vec![0.5, (1.0f32 - 0.25).sqrt()]; // 与 zh cos=0.5 ≥0.3
        let mut vp = Voiceprint::default();
        for _ in 0..3 {
            vp.offer(&e_zh, 4.0, None, Some("zh"));
        }
        vp.offer(&e_en, 5.0, None, Some("en"));
        assert!(vp
            .profiles
            .get("en")
            .is_some_and(|p| !p.candidates.is_empty()));
    }

    /// ja 语言门：judge(ja) ⇒ KeepLanguage（保留）；judge_voiceprint（无语言门，408B 核心）按分判。
    #[test]
    fn ts408b_ja_language_gate_vs_core() {
        let e = unit(0, 2);
        let mut vp = Voiceprint::default();
        for _ in 0..3 {
            vp.offer(&e, 4.0, None, Some("zh"));
        }
        assert_eq!(
            judge(&vp, Some(&e), 3.0, Some("ja")),
            SegVerdict::KeepLanguage
        );
        assert!(matches!(
            judge_voiceprint(&vp, Some(&e), 3.0),
            SegVerdict::KeepUser(_)
        ));
    }

    /// 版本迁移：v1 单档 JSON ⇒ 迁移到 `und`，不丢弃。
    #[test]
    fn ts408b_v1_archive_migrates_not_discarded() {
        let dir = std::env::temp_dir().join(format!(
            "voice-ime-spk-408b-mig-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("voiceprint.json");
        let v1 = format!(
            "{{\"version\":1,\"model\":\"{MODEL_TAG}\",\"centroid\":[1.0,0.0],\"total_secs\":20.0,\"segments\":5}}"
        );
        std::fs::write(&path, v1).unwrap();
        let vp = Voiceprint::load(&path);
        assert!(vp.has_any_ready(), "v1 应迁移不丢弃");
        assert!(vp.lang_ready(MIGRATED_LANG), "迁移到 und");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 模型不符 ⇒ 重建（不迁移）。
    #[test]
    fn ts408b_model_mismatch_rebuilds() {
        let dir = std::env::temp_dir().join(format!(
            "voice-ime-spk-408b-mm-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("voiceprint.json");
        std::fs::write(
            &path,
            "{\"version\":2,\"model\":\"OTHER-MODEL\",\"profiles\":{\"zh\":{\"centroid\":[1.0,0.0],\"total_secs\":20.0,\"segments\":5}}}",
        )
        .unwrap();
        assert!(!Voiceprint::load(&path).has_any_ready());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `partition_ranges` 纯逻辑：<2s 保留、Drop 剔除、缺位保守保留。
    #[test]
    fn ts408b_partition_ranges_pure() {
        let ranges = [(0usize, 32_000usize), (32_000, 96_000), (96_000, 128_000)];
        let verdicts = [
            SegVerdict::KeepUser(0.9),
            SegVerdict::DropNonUser(0.2),
            SegVerdict::KeepShort,
        ];
        let (kept, kept_secs, dropped) = partition_ranges(&ranges, &verdicts);
        assert_eq!(kept, vec![(0, 32_000), (96_000, 128_000)]);
        assert!((kept_secs - 4.0).abs() < 1e-4);
        assert!((dropped - 4.0).abs() < 1e-4);
        let (kept2, _, dropped2) = partition_ranges(&ranges, &[SegVerdict::KeepUser(0.9)]);
        assert_eq!(kept2.len(), 3);
        assert_eq!(dropped2, 0.0);
    }
}
