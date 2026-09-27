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

/// 声纹**判定**最短时长（秒）：`≥` 此值才判，`<` ⇒ `KeepShort`（保留不剔）。
///
/// VOICEPRINT-JUDGE-1P5S-432（Gavin 2026-09-25「A，432进包」）：2.0 → **1.5**。依据 432 报告 R1：
/// AISHELL-1 6 人 1.5~1.8s 本人 min 0.542 > `DROP_THR` 0.45（误删 0%）、他人剔 96~100%；1.0~1.5s
/// 交叠不做；Gavin 点名 1.60s 旁语句 score 0.086~0.157。**同时用于**：412 短段**合并目标**（凑够
/// 即可判）与语种不支持判定（`judge_voiceprint` 内）。
///
/// VOICEPRINT-JUDGE-1S-446（Gavin 2026-09-26「B」）：1.5 → **1.0**。依据 POC-444（`collab/evidence/444/table.md`，
/// 4 段真录音）：本人 1.0~1.5s 22 条最低 0.582、<1.0s 3 条最低 0.587，均 > `DROP_THR` 0.45；旁放他人语音
/// 1.12s 得 0.177、0.67s 得 0.153（1.5s 门槛下全部 `KeepShort` 漏判，干扰进入最终文字）。<1.0s 样本少
/// 且 0.42s 片段得 0.234 ⇒ 1.0s 以下仍不判。⚠️ 仅一名使用人数据，404B 多人语料显示 1~2s 判别力整体偏弱，
/// 端测观察他人使用时的误剔。注册 / 漂移 offer 仍 [`MIN_OFFER_SECS`] 2.0s。
pub(crate) const MIN_JUDGE_SECS: f32 = 1.0;
/// **注册 / 漂移 offer** 最短时长（秒）：1.5~2.0s 片段嵌入质量差，**不得**进入声纹注册
/// （否则拉低本人档）。故 offer 门槛**独立**于判定门槛，维持 **2.0**。
///
/// 🔴 拆分原因（432 主控核对）：原 `MIN_JUDGE_SECS`(2.0) 身兼三职 —— ① 判定 ② 412 合并目标
/// ③ 注册/漂移 offer —— 不能整体下调。判定随 432 降 1.5s，offer 保持 2.0s。
pub(crate) const MIN_OFFER_SECS: f32 = 2.0;
/// 就绪所需的最少有效语音（秒）与段数（Gavin 定「攒够 ≥12s 且 ≥3 段」）。
pub(crate) const ENROLL_MIN_SECS: f32 = 12.0;
pub(crate) const ENROLL_MIN_SEGS: u32 = 3;
/// 注册离群阈：候选段与候选质心 cos 低于此值即判为「混入的他人」而剔除（防开头把声纹学歪）。
pub(crate) const ENROLL_OUTLIER_THR: f32 = 0.5;
/// 漂移更新阈：仅 `score >= 此值` 的段参与 EMA 更新（高置信本人段）。
pub(crate) const UPDATE_THR: f32 = 0.75;
/// 漂移更新单段权重上限：防被单段（哪怕很长）带偏。
pub(crate) const UPDATE_MAX_WEIGHT: f32 = 0.25;
/// 剔除阈：最高分 < 此值且段 ≥ [`MIN_JUDGE_SECS`](1.5s)（432 起）且该语种档就绪 ⇒ 高置信非本人 ⇒ 剔除。
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

/// 提取器**进程级**懒加载：外层 = 是否已尝试，内层 = 是否可用（失败记住不重试）。
/// LOCALRT-WARM-CACHE-450：原为精解线程级缓存，而精解线程每次录音新建 ⇒ 每次录音首窗重载 CAM++
///（日志 15 次录音加载 13 次、首窗多等约 0.5s）。提取器无状态（每次 `embed` 新建 stream），跨录音共用输出逐位不变。
static SPEAKER_EXTRACTOR: Mutex<Option<Option<SpeakerVerifier>>> = Mutex::new(None);

#[cfg(test)]
thread_local! {
    /// 测试专用：本线程模拟「模型缺失」（不动进程级缓存，避免干扰并行测试）。
    static TEST_EXTRACTOR_MISSING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 取提取器（首次调用加载）；`None` = 模型缺失 / 加载失败。
fn with_speaker_extractor<R>(f: impl FnOnce(Option<&SpeakerVerifier>) -> R) -> R {
    #[cfg(test)]
    if TEST_EXTRACTOR_MISSING.with(|c| c.get()) {
        return f(None);
    }
    let mut slot = SPEAKER_EXTRACTOR.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = Some(SpeakerVerifier::load(&super::model_dir()));
    }
    f(slot.as_ref().and_then(|o| o.as_ref()))
}

/// LOCALRT-WARM-CACHE-450：随本地实时模型加载预热提取器（首次录音首窗不再现场加载）。
pub(crate) fn warm_speaker_extractor() {
    with_speaker_extractor(|_| ());
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
    /// FIX-421-R1：被剔除区间（窗内坐标）。`kept ∪ dropped` = 全部语音区间（供兜底按语音轴映射）。
    pub dropped: Vec<(usize, usize)>,
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

/// SPEAKER-MERGE-SHORT-412：相邻短语音段合并成「连续说话单元」的最大间隔（秒）。
///
/// **0.8s = 端测实测校准（R1）**：BUILD-409 同窗相邻语音段间隔实测 0.29~1.96s，
/// **一半以上落在 0.5~0.8s**（0.51 / 0.74 / 0.48 / 0.55 / 0.64 / 0.77 / 0.54 / 0.61 /
/// 0.67 / 0.48 / 0.51 / 0.71 / 0.67 / 0.86 …）。原定 0.5s 会把窗 #4（1.02 / 1.70 /
/// 1.41 / 0.96s，间隔 0.64 / 0.77 / 0.54）**一段都拼不上** ⇒ 等于没改。
/// 间隔 ≥ 此值视为可能换人、不合并。
///
/// 🔴 安全性：合并只影响**判定与 offer**；拼错（把本人与背景并进同单元）只会让该单元
/// 声纹得分**居中**（本人与他人平均）而**达不到剔除阈** ⇒ **宁放过不误删**（见 [`merge_speech_units`]）。
pub(crate) const MERGE_GAP_SECS: f32 = 0.8;

/// 412：一个「连续说话单元」= 若干相邻语音区间（两两间隔 < [`MERGE_GAP_SECS`]），
/// 外加**语音总样本数**（只算语音、**不含间隔**）。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpeechUnit {
    pub members: Vec<(usize, usize)>,
    pub speech_samples: usize,
}

/// 412：把相邻语音区间（**已按时间序**、样本下标半开区间）合并为「连续说话单元」。纯函数。
///
/// 规则（R1 定稿）：
/// - ① 间隔 `< MERGE_GAP_SECS`(0.8s) 才**可能**同单元；`≥` 断开（可能换人）。
/// - ② **只为凑够判定门槛才拼**：当前单元语音已 ≥ [`MIN_JUDGE_SECS`](1.5s) 时，下一段另起新单元。
/// - ③ **收尾**：最后一个单元若 <1.5s 且与前一单元间隔 <0.8s ⇒ 并入前一单元（免得尾段落单不判）。
/// - ④ **≥1.5s 的单段各自独立**：本段自身 ≥1.5s 时不并入前单元（另起）。
/// - 🔴 退化区间（`s>=e`）作为**零长成员**并入当前单元 ⇒ 成员与输入区间**一一对应**（回填判定不失配）；
///   零长成员不计时长、自然落 `KeepShort`。
///
/// 安全性：拼错最多把本人与背景并成一个「混合单元」，其声纹得分居中（本人与他人平均）
/// ⇒ 通常仍 `KeepUser` / 不达 `DROP_THR` ⇒ **不误删**；宁放过不误删。
pub(crate) fn merge_speech_units(ranges: &[(usize, usize)]) -> Vec<SpeechUnit> {
    let gap = (MERGE_GAP_SECS * SAMPLE_RATE as f32) as usize;
    let judge = (MIN_JUDGE_SECS * SAMPLE_RATE as f32) as usize;
    let mut units: Vec<SpeechUnit> = Vec::new();
    for &(s, e) in ranges {
        let len = e.saturating_sub(s);
        match units.last_mut() {
            Some(u) => {
                let prev_end = u.members.last().unwrap().1;
                let near = s.saturating_sub(prev_end) < gap;
                // ② 仅当当前单元与**本段**都还不足 2s、且间隔够近，才并入（只为凑够 2s）；
                // ④ 已达 2s 的单元、或本身 ≥2s 的段，一律另起（≥2s 单段各自独立）。
                if near && u.speech_samples < judge && len < judge {
                    u.members.push((s, e));
                    u.speech_samples += len;
                } else {
                    units.push(SpeechUnit {
                        members: vec![(s, e)],
                        speech_samples: len,
                    });
                }
            }
            None => units.push(SpeechUnit {
                members: vec![(s, e)],
                speech_samples: len,
            }),
        }
    }
    // ③ 收尾：最后一个单元若 <2s 且与前一单元间隔 <0.8s ⇒ 并入前一单元（免得尾段落单不判）。
    if units.len() >= 2 {
        let last = units.len() - 1;
        let between = units[last]
            .members
            .first()
            .unwrap()
            .0
            .saturating_sub(units[last - 1].members.last().unwrap().1);
        if units[last].speech_samples < judge && between < gap {
            let tail = units.pop().unwrap();
            let prev = units.last_mut().unwrap();
            prev.speech_samples += tail.speech_samples;
            prev.members.extend(tail.members);
        }
    }
    units
}

/// 412：单元是否参与 offer —— 单元**起点**（`members[0].0`）≥ `new_slice_from`。
/// 跨界（起点在前文、延伸进本窗新片）的单元**保守不 offer**（防把窗口前文重复计入注册/漂移）。
fn unit_offer_allowed(unit: &SpeechUnit, new_slice_from: usize) -> bool {
    unit.members.first().map(|m| m.0).unwrap_or(0) >= new_slice_from
}

/// 432：注册 / 漂移 offer 的**总闸** —— 单元语音时长 ≥ [`MIN_OFFER_SECS`](2.0s) **且**起点在窗内。
///
/// 🔴 **注册语义**，独立于判定门槛 [`MIN_JUDGE_SECS`](1.5s)：1.5~2.0s 单元可判定（甚至剔除），
/// 但**不注册**（嵌入质量差，防拉低本人档）。抽为纯函数便于单测与源码护栏。
pub(crate) fn unit_offer_eligible(
    unit_secs: f32,
    unit: &SpeechUnit,
    new_slice_from: usize,
) -> bool {
    unit_secs >= MIN_OFFER_SECS && unit_offer_allowed(unit, new_slice_from)
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
            dropped: Vec::new(),
            kept_secs: 0.0,
            dropped_secs: 0.0,
            any_ready: false,
            enrolled_secs: 0.0,
            details: Vec::new(),
            pending_offers: Vec::new(),
        };
    }
    with_speaker_extractor(|verifier| {
        let mut details: Vec<RangeJudgement> = Vec::new();
        let Some(verifier) = verifier else {
            return VoiceprintFilter {
                kept: ranges.to_vec(),
                dropped: Vec::new(),
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
        // 412：先按「连续说话单元」合并相邻短段（大量 <2s 段原本 KeepShort 漏判），再**按单元**判定。
        // 判定结果作用于单元**全部成员**（全留 / 全剔）；间隔 ≥ MERGE_GAP_SECS 不合并（可能换人）。
        let units = merge_speech_units(ranges);
        let mut range_verdicts: Vec<SegVerdict> = vec![SegVerdict::KeepShort; ranges.len()];
        let mut pending_offers: Vec<PendingOffer> = Vec::new();
        let mut ri = 0usize; // ranges 游标（unit 的成员按序对应 ranges）
        for unit in &units {
            let unit_secs = unit.speech_samples as f32 / SAMPLE_RATE as f32;
            let (verdict, emb) = if unit_secs < MIN_JUDGE_SECS {
                (SegVerdict::KeepShort, None)
            } else {
                // 拼接单元内**全部语音样本**（不含间隔）算一次声纹。
                let mut buf: Vec<f32> = Vec::with_capacity(unit.speech_samples);
                for &(s, e) in &unit.members {
                    buf.extend_from_slice(&samples[s..e]);
                }
                let emb = verifier.embed(&buf);
                let v = judge_voiceprint(&proc.vp, emb.as_deref(), unit_secs);
                (v, emb)
            };
            if log::log_enabled!(log::Level::Debug) {
                let first =
                    unit.members.first().map(|m| m.0).unwrap_or(0) as f32 / SAMPLE_RATE as f32;
                let last =
                    unit.members.last().map(|m| m.1).unwrap_or(0) as f32 / SAMPLE_RATE as f32;
                log::debug!(
                    "[LocalRT-DBG-412] unit: win=- members={} speech={:.2}s span={:.2}-{:.2}s verdict={:?} score={:.3}",
                    unit.members.len(),
                    unit_secs,
                    first,
                    last,
                    verdict,
                    verdict_score(verdict)
                );
            }
            for _ in &unit.members {
                if ri < range_verdicts.len() {
                    range_verdicts[ri] = verdict;
                    ri += 1;
                }
            }
            // 注册 / 漂移按**单元** offer（≥ `MIN_OFFER_SECS`=2.0s，**非判定门槛** `MIN_JUDGE_SECS`=1.5s；
            // 时长只算语音）；`new_slice_from` 之前不 offer（单元跨越时按**单元起点**判定：起点在前文 ⇒ 整个单元不 offer）。
            // 🔴 432：1.5~2.0s 的单元可**判定**（剔除）但**不注册**（嵌入质量差，防拉低本人档）。
            if unit_offer_eligible(unit_secs, unit, new_slice_from) {
                if let Some(e) = emb.as_deref() {
                    match verdict {
                        SegVerdict::KeepUser(s) => pending_offers.push(PendingOffer {
                            emb: e.to_vec(),
                            secs: unit_secs,
                            score_if_ready: Some(s),
                        }),
                        SegVerdict::KeepNotReady => pending_offers.push(PendingOffer {
                            emb: e.to_vec(),
                            secs: unit_secs,
                            score_if_ready: None,
                        }),
                        _ => {}
                    }
                }
            }
        }
        let (kept, kept_secs, dropped_secs) = partition_ranges(ranges, &range_verdicts);
        for (i, &(start, end)) in ranges.iter().enumerate() {
            let secs = (end - start) as f32 / SAMPLE_RATE as f32;
            details.push(RangeJudgement {
                start,
                end,
                secs,
                verdict: range_verdicts[i],
                score: verdict_score(range_verdicts[i]),
            });
        }
        // FIX-421-R1：剔除区间（供兜底按「语音轴」映射、只删 dropped）。
        let mut dropped_ranges: Vec<(usize, usize)> = Vec::new();
        for (i, &(s, e)) in ranges.iter().enumerate() {
            if matches!(range_verdicts[i], SegVerdict::DropNonUser(_)) {
                dropped_ranges.push((s, e));
            }
        }
        VoiceprintFilter {
            kept,
            dropped: dropped_ranges,
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
        // 446：判定门槛降到 1.0s ⇒ 0.99s 仍 KeepShort（短段门）；1.49s 现在进入判定。
        assert_eq!(
            judge(&vp, Some(&unit_with_cos(0.1)), 0.99, Some("zh")),
            SegVerdict::KeepShort
        );
        assert_eq!(
            judge(&vp, Some(&unit_with_cos(0.1)), 1.49, Some("zh")),
            SegVerdict::DropNonUser(0.1)
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
        // 446：短段门随判定门槛降到 1.0s ⇒ 临界下方取 0.99s（短段门仍最优先）。
        assert_eq!(
            judge(&vp, Some(&at_cos(0.0)), 0.99, Some("ja")),
            SegVerdict::KeepShort
        );
        assert_eq!(
            judge(&Voiceprint::default(), None, 0.99, Some("ja")),
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

// =====================================================================
// TEST-SYNC-406-408B（阶段三 · 非作者护栏 · coder-1）：408B 声纹接入契约，
// 不与作者 408B（`ts408b_*` / `fix408b_tests`）重复。契约出处：408B 任务书 + `logs/20260924.md`。
// =====================================================================
#[cfg(test)]
mod testsync408b_guard_tests {
    use super::{
        judge_voiceprint, partition_ranges, LangProfile, SegVerdict, Voiceprint, DROP_THR,
        ENROLL_MIN_SECS, ENROLL_MIN_SEGS, MIN_JUDGE_SECS, MODEL_TAG,
    };

    fn ready(lang: &str, centroid: Vec<f32>) -> Voiceprint {
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

    /// 契约 6：多档就绪取**最高分**（本人换语言但对另一档高分 ⇒ 保留）；两档皆不像 ⇒ 剔除。
    #[test]
    fn ts408b_judge_voiceprint_max_score_across_profiles() {
        let mut vp = ready("zh", vec![1.0, 0.0]);
        vp.profiles.insert(
            "en".to_string(),
            LangProfile {
                centroid: vec![0.0, 1.0],
                total_secs: ENROLL_MIN_SECS,
                segments: ENROLL_MIN_SEGS,
                candidates: Vec::new(),
            },
        );
        // 对 en 满分、对 zh 零分 ⇒ 最高分 1.0 ⇒ KeepUser（换语言仍保留）。
        match judge_voiceprint(&vp, Some(&[0.0, 1.0]), 3.0) {
            SegVerdict::KeepUser(s) => assert!((s - 1.0).abs() < 1e-6, "最高分应为 1.0，实测 {s}"),
            other => panic!("应 KeepUser，实测 {other:?}"),
        }
        // 与 en 正交（最高分 0 < DROP）⇒ Drop。
        match judge_voiceprint(&vp, Some(&[-1.0, 0.0]), 3.0) {
            SegVerdict::DropNonUser(s) => assert!(s < DROP_THR, "应低于剔除阈"),
            other => panic!("应 DropNonUser，实测 {other:?}"),
        }
    }

    /// 契约 6：无任何就绪档 ⇒ 不剔；无 emb（模型缺失）⇒ 保留；<2s ⇒ 永不剔。
    #[test]
    fn ts408b_judge_voiceprint_no_ready_and_short() {
        let none = Voiceprint::default();
        assert_eq!(
            judge_voiceprint(&none, Some(&[1.0, 0.0]), 3.0),
            SegVerdict::KeepNotReady,
            "无就绪档 ⇒ 保留"
        );
        let vp = ready("zh", vec![1.0, 0.0]);
        assert_eq!(judge_voiceprint(&vp, None, 3.0), SegVerdict::KeepNotReady);
        assert_eq!(
            judge_voiceprint(&vp, Some(&[-1.0, 0.0]), MIN_JUDGE_SECS - 0.001),
            SegVerdict::KeepShort,
            "< MIN_JUDGE_SECS ⇒ 永不剔"
        );
        assert!(
            matches!(
                judge_voiceprint(&vp, Some(&[-1.0, 0.0]), MIN_JUDGE_SECS),
                SegVerdict::DropNonUser(_)
            ),
            "恰 MIN_JUDGE_SECS 进入判定"
        );
    }

    /// 契约 6/4：`partition_ranges` 划分 keep/drop（判定数组缺位 ⇒ 保守保留）。
    #[test]
    fn ts408b_partition_ranges_conservative() {
        // 区间长度按 `SAMPLE_RATE` 换算，**不硬编码样本数**（32000 样本 = 2.0s @16k）。
        let sec = super::SAMPLE_RATE as usize;
        // 三段各 2s。
        let ranges = [(0usize, 2 * sec), (2 * sec, 4 * sec), (4 * sec, 6 * sec)];
        let (kept, kept_secs, dropped) = partition_ranges(
            &ranges,
            &[
                SegVerdict::KeepShort,
                SegVerdict::KeepUser(0.9),
                SegVerdict::DropNonUser(0.1),
            ],
        );
        assert_eq!(kept, vec![(0, 2 * sec), (2 * sec, 4 * sec)]);
        // 保留 2 段 = 4.0s；剔除 1 段 = 2.0s。
        assert!(
            (kept_secs - 4.0).abs() < 1e-6,
            "kept_secs 应 4.0，实测 {kept_secs}"
        );
        assert!(
            (dropped - 2.0).abs() < 1e-6,
            "dropped 应 2.0，实测 {dropped}"
        );
        // 判定数组缺位 ⇒ 缺位区间按 KeepShort 保守保留。
        let (kept2, kept2_secs, dropped2) =
            partition_ranges(&ranges, &[SegVerdict::DropNonUser(0.1)]);
        assert_eq!(kept2.len(), 2, "缺位区间必须保留");
        assert!((kept2_secs - 4.0).abs() < 1e-6 && (dropped2 - 2.0).abs() < 1e-6);
    }

    /// 契约 9：新语种候选「对所有就绪档最高分 <0.3 ⇒ 不收」；≥0.3 ⇒ 可注册；lang 空/None ⇒ 不注册。
    #[test]
    fn ts408b_offer_new_language_gate() {
        // 已有 zh 就绪；背景人（与 zh 正交、最高分 0）反复 offer en ⇒ 不收。
        let mut vp = ready("zh", vec![1.0, 0.0]);
        for _ in 0..4 {
            vp.offer(&[0.0, 1.0], 5.0, None, Some("en"));
        }
        assert!(!vp.lang_ready("en"), "最高分<0.3 ⇒ 不得学成新语种档");
        // 本人换语言的候选（与 zh 最高分 1.0 ≥0.3）⇒ 收；12s/3 段后 en 就绪。
        for _ in 0..3 {
            vp.offer(&[1.0, 0.0], 4.0, None, Some("en"));
        }
        assert!(vp.lang_ready("en"), "合格候选应注册并达成就绪");
        // lang 未知 / 空 ⇒ 不注册。
        let mut vp2 = Voiceprint::default();
        vp2.offer(&[1.0, 0.0], 20.0, None, None);
        vp2.offer(&[1.0, 0.0], 20.0, None, Some(""));
        assert!(!vp2.has_any_ready(), "无语种 ⇒ 无处归档，不注册");
    }

    /// 契约 9（408B，412 延续）：`new_slice_from` 之前的区间**不 offer**（防窗口前文重复计入）。
    /// 412 起判定/offer 按「连续说话单元」进行 ⇒ 锚点改为单元级判据 `unit_offer_allowed`
    /// （**单元起点** ≥ `new_slice_from`）；行为契约不变，另有 `ts412_unit_offer_boundary` 行为用例。
    /// 锚点用 `concat!` 拆段防自匹配。
    #[test]
    fn ts408b_new_slice_from_contract_anchor() {
        let src = include_str!("speaker.rs");
        let anchor = concat!(
            "unit.members.first().map(|m| m.0)",
            ".unwrap_or(0) >= new_slice_from"
        );
        assert!(
            src.contains(anchor),
            "契约 9 锚点缺失：new_slice_from 之前的单元不得 offer"
        );
    }

    /// 契约 10：v1→v2 迁移不丢；模型标签不符 ⇒ 重建；损坏文件不 panic；v2 往返。
    #[test]
    fn ts408b_persistence_v1_migrate_rebuild_corrupt() {
        use std::io::Write as _;
        let uniq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("voice-ime-ts408b-{uniq}.json"));
        // v1 单档 ⇒ 迁移到 und（保留就绪，不丢弃）。
        let v1 = format!(
            "{{\"version\":1,\"model\":\"{MODEL_TAG}\",\"centroid\":[1.0,0.0],\"total_secs\":20.0,\"segments\":5}}"
        );
        std::fs::File::create(&path)
            .unwrap()
            .write_all(v1.as_bytes())
            .unwrap();
        assert!(
            Voiceprint::load(&path).lang_ready("und"),
            "v1 档必须迁移到 und 且保留就绪态"
        );
        // 模型标签不符 ⇒ 重建（空）。
        let bad = "{\"version\":2,\"model\":\"OLD\",\"profiles\":{\"zh\":{\"centroid\":[1.0,0.0],\"total_secs\":20.0,\"segments\":5}}}";
        std::fs::File::create(&path)
            .unwrap()
            .write_all(bad.as_bytes())
            .unwrap();
        assert!(!Voiceprint::load(&path).has_any_ready(), "模型不符须重建");
        // 损坏文件 ⇒ 空、不 panic。
        std::fs::File::create(&path)
            .unwrap()
            .write_all(b"not json at all")
            .unwrap();
        assert!(!Voiceprint::load(&path).has_any_ready(), "坏档须静默重建");
        // v2 往返：就绪 zh 存后读回仍就绪。
        let vp = ready("zh", vec![1.0, 0.0]);
        vp.save(&path);
        assert!(
            Voiceprint::load(&path).lang_ready("zh"),
            "v2 往返应保留 zh 档"
        );
        let _ = std::fs::remove_file(&path);
    }
}

// =====================================================================
// SPEAKER-MERGE-SHORT-412：相邻短段合并成「连续说话单元」再判 —— 纯逻辑护栏 + 影响面。
// =====================================================================
#[cfg(test)]
mod testsync412_merge_tests {
    use super::{
        cosine, filter_ranges_by_voiceprint, judge_voiceprint, merge_speech_units,
        partition_ranges, unit_offer_allowed, LangProfile, SegVerdict, SpeechUnit, Voiceprint,
        DROP_THR, ENROLL_MIN_SECS, ENROLL_MIN_SEGS, MERGE_GAP_SECS, MIN_JUDGE_SECS, SAMPLE_RATE,
        UPDATE_MAX_WEIGHT,
    };

    fn secs(x: f32) -> usize {
        (x * SAMPLE_RATE as f32) as usize
    }

    /// 已就绪 zh 档（质心 `[1,0]`）——供漂移封顶用例。
    fn ready_vp() -> Voiceprint {
        let mut vp = Voiceprint::default();
        vp.profiles.insert(
            "zh".to_string(),
            LangProfile {
                centroid: vec![1.0, 0.0],
                total_secs: ENROLL_MIN_SECS,
                segments: ENROLL_MIN_SEGS,
                candidates: Vec::new(),
            },
        );
        vp
    }

    /// 与 `[1,0]` 余弦为 `c` 的单位向量。
    fn at_cos(c: f32) -> Vec<f32> {
        let s = (1.0 - c * c).max(0.0).sqrt();
        vec![c, s]
    }

    /// 契约 1（R1）：间隔阈值 0.8s —— 0.3s 合并、0.9s 断开；三段链式；空/单段边界。
    #[test]
    fn ts412_merge_gap_threshold() {
        assert_eq!(MERGE_GAP_SECS, 0.8, "R1：端测实测校准 0.8s");
        // 446：判定门槛 1.0s ⇒ 用 <1.0s 短段测合并。0.6s + (0.3s 间隔) + 0.5s ⇒ 一个单元（语音 1.1s，不含间隔）。
        let merged = merge_speech_units(&[
            (0, secs(0.6)),
            (secs(0.6) + secs(0.3), secs(0.6) + secs(0.3) + secs(0.5)),
        ]);
        assert_eq!(merged.len(), 1, "0.3s 间隔应合并");
        assert_eq!(merged[0].members.len(), 2);
        assert!(
            (merged[0].speech_samples as f32 / SAMPLE_RATE as f32 - 1.1).abs() < 1e-3,
            "语音总时长应 1.1s（不含间隔）"
        );
        // 0.9s 间隔（≥0.8）⇒ 两个单元（各自 <1.0s ⇒ KeepShort）。
        let split = merge_speech_units(&[(0, secs(0.6)), (secs(1.5), secs(1.5) + secs(0.5))]);
        assert_eq!(split.len(), 2, "0.9s 间隔应断开");
        for u in &split {
            assert!((u.speech_samples as f32 / SAMPLE_RATE as f32) < MIN_JUDGE_SECS);
        }
        // 三段链式（各 0.8s、间隔 0.25s）：前两段 1.6s<2 继续拼，第三段 ⇒ 一个单元 3 成员 2.4s。
        let chain = merge_speech_units(&[
            (0, secs(0.8)),
            (secs(1.05), secs(1.85)),
            (secs(2.10), secs(2.90)),
        ]);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].members.len(), 3);
        assert!((chain[0].speech_samples as f32 / SAMPLE_RATE as f32 - 2.4).abs() < 1e-3);
        // 空 / 单段。
        assert!(merge_speech_units(&[]).is_empty());
        let one = merge_speech_units(&[(0, secs(2.0))]);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].members.len(), 1);
    }

    /// 契约 2（R1·规则② / 432）：**只为凑够判定门槛才拼** —— 单元一旦 ≥ [`MIN_JUDGE_SECS`]（432 起 1.5s），
    /// 下一段另起新单元；合并目标随判定门槛（1.5s），**非**注册门槛 [`MIN_OFFER_SECS`](2.0s)。
    #[test]
    fn ts412_merge_only_to_reach_judge_secs() {
        // 446：0.8 + 0.5 = 1.3（≥1.0 封口）；第三段 0.5 另起；第四段 0.5（间隔 0.8 不近邻）另起。
        let units = merge_speech_units(&[
            (0, secs(0.8)),
            (secs(1.0), secs(1.5)),
            (secs(1.7), secs(2.2)),
            (secs(3.0), secs(3.5)),
        ]);
        assert_eq!(units.len(), 3, "凑够 1.0s 后应封口、其后两段各自另起");
        assert_eq!(units[0].members.len(), 2, "前两段拼到 1.3s");
        assert!((units[0].speech_samples as f32 / SAMPLE_RATE as f32 - 1.3).abs() < 1e-3);
        assert_eq!(units[1].members.len(), 1, "第三段独立（前一单元已封口）");
        assert_eq!(units[2].members.len(), 1, "第四段独立（间隔 0.8 不近邻）");
        assert!((units[2].speech_samples as f32 / SAMPLE_RATE as f32 - 0.5).abs() < 1e-3);
    }

    /// 契约 2b（432 边界）：单段恰为 [`MIN_JUDGE_SECS`](1.5s) 时本身**已可判**，不再被并入前一单元（规则④）。
    #[test]
    fn ts412_merge_exact_judge_secs_segment_independent() {
        let units = merge_speech_units(&[(0, secs(0.8)), (secs(1.0), secs(2.5))]);
        assert_eq!(units.len(), 2, "1.5s 单段自身可判 ⇒ 不并入前单元");
        assert_eq!(units[0].members.len(), 1);
        assert!((units[1].speech_samples as f32 / SAMPLE_RATE as f32 - 1.5).abs() < 1e-3);
    }

    /// 契约 4（R1·规则④）：**≥2s 的单段各自独立**，不与相邻短段拼接。
    #[test]
    fn ts412_merge_long_segment_independent() {
        // 1.0s 短段 + 2.5s 长段（间隔 0.2s）⇒ 长段独立成单元，短段不并入。
        let units = merge_speech_units(&[(0, secs(1.0)), (secs(1.2), secs(3.7))]);
        assert_eq!(units.len(), 2, "≥2s 单段应独立");
        assert_eq!(units[0].members.len(), 1);
        assert!((units[0].speech_samples as f32 / SAMPLE_RATE as f32 - 1.0).abs() < 1e-3);
        assert_eq!(units[1].members.len(), 1);
        assert!((units[1].speech_samples as f32 / SAMPLE_RATE as f32 - 2.5).abs() < 1e-3);
        assert!(units[1].speech_samples as f32 / SAMPLE_RATE as f32 >= MIN_JUDGE_SECS);
    }

    /// 契约 3（R1·规则③）：**收尾** —— 最后一个 <2s 单元若与前一单元间隔 <0.8s ⇒ 并入前一单元。
    #[test]
    fn ts412_tail_merges_into_prev() {
        // 446（门槛 1.0s）：0.6+0.5 ⇒ 1.1（封口）；尾段 0.4 与前一单元间隔 0.2<0.8 ⇒ 并入 ⇒ 一个单元 1.5s。
        let merged = merge_speech_units(&[
            (0, secs(0.6)),
            (secs(0.8), secs(1.3)),
            (secs(1.5), secs(1.9)),
        ]);
        assert_eq!(merged.len(), 1, "尾段应并入前一单元");
        assert_eq!(merged[0].members.len(), 3);
        assert!((merged[0].speech_samples as f32 / SAMPLE_RATE as f32 - 1.5).abs() < 1e-3);
        // 尾段与前一单元间隔 0.9≥0.8 ⇒ 不并（尾段留独立、<1.0s ⇒ KeepShort）。
        let kept = merge_speech_units(&[
            (0, secs(0.6)),
            (secs(0.8), secs(1.3)),
            (secs(2.2), secs(2.6)),
        ]);
        assert_eq!(kept.len(), 2, "间隔 ≥0.8s 尾段不并");
        assert!((kept[1].speech_samples as f32 / SAMPLE_RATE as f32 - 0.4).abs() < 1e-3);
    }

    /// 契约（R1·真实数据 / 432 更新）：窗 #4 实测区间（1.02/1.70/1.41/0.96s，间隔 0.64/0.77/0.54）。
    /// 432 合并目标随判定门槛降 1.5s ⇒ 1.70s 单段**自身可判**独立，故结果 **3 个单元**
    /// （1.02 `<1.5` KeepShort；1.70 独立可判；1.41+0.96=2.37 合并可判）；不再是 2.0s 时代的 2 单元。
    #[test]
    fn ts412_merge_window4_real_intervals() {
        let sr = SAMPLE_RATE as f32;
        let at = |start: f32, len: f32| ((start * sr) as usize, ((start + len) * sr) as usize);
        let g1 = 0.64f32;
        let g2 = 0.77f32;
        let g3 = 0.54f32;
        let ranges = [
            at(0.0, 1.02),
            at(1.02 + g1, 1.70),
            at(1.02 + g1 + 1.70 + g2, 1.41),
            at(1.02 + g1 + 1.70 + g2 + 1.41 + g3, 0.96),
        ];
        let units = merge_speech_units(&ranges);
        assert_eq!(
            units.len(),
            3,
            "窗#4 应拼成 3 个单元（446·1.0s 门槛，分组与 432 时相同）"
        );
        // 1.02 独立（≥1.0 自身可判）；1.70 独立（自身可判）；1.41 独立后 0.96 尾段并入。
        assert_eq!(units[0].members.len(), 1, "1.02s 独立（≥1.0s 自身可判）");
        assert_eq!(units[1].members.len(), 1, "1.70s 自身 ≥1.5s ⇒ 独立");
        assert_eq!(units[2].members.len(), 2, "1.41s+0.96s ⇒ 2.37s");
        let s0 = units[0].speech_samples as f32 / sr;
        let s1 = units[1].speech_samples as f32 / sr;
        let s2 = units[2].speech_samples as f32 / sr;
        assert!(
            (s0 - 1.02).abs() < 2e-2 && (s1 - 1.70).abs() < 2e-2 && (s2 - 2.37).abs() < 2e-2,
            "实测 {s0}/{s1}/{s2}"
        );
        // 446：可判单元（≥ 判定门槛 1.0s）恰 3 个：1.02、1.70、2.37。
        let judgeable = units
            .iter()
            .filter(|u| u.speech_samples as f32 / sr >= MIN_JUDGE_SECS)
            .count();
        assert_eq!(judgeable, 3, "恰 3 个可判单元");
    }

    /// 契约 1：`speech_samples` 只数语音（相邻两段 1.2s+1.1s、间隔 0.3s ⇒ 2.3s 而非 2.6s）。
    #[test]
    fn ts412_merge_speech_only_excludes_gaps() {
        // 446：<1.0s 短段才拼（判定门槛 1.0s）。
        let u = merge_speech_units(&[(0, secs(0.6)), (secs(0.9), secs(1.4))]);
        assert_eq!(u.len(), 1);
        assert_eq!(
            u[0].speech_samples,
            secs(0.6) + (secs(1.4) - secs(0.9)),
            "只数语音、不含 0.3s 间隔"
        );
    }

    /// 契约 2：判定作用于**单元全部成员**，且 kept/dropped 秒数按成员语音时长累计（与单段口径一致）。
    #[test]
    fn ts412_unit_verdict_scope_and_seconds() {
        let ranges = [(0usize, secs(1.2)), (secs(1.5), secs(2.6))]; // 单元语音 2.3s
                                                                    // 全剔：两成员都 dropped，dropped_secs = 1.2 + 1.1。
        let (kept, kept_secs, dropped) = partition_ranges(
            &ranges,
            &[SegVerdict::DropNonUser(0.1), SegVerdict::DropNonUser(0.1)],
        );
        assert!(kept.is_empty(), "单元 Drop ⇒ 全部成员剔除");
        assert_eq!(kept_secs, 0.0);
        assert!(
            (dropped - 2.3).abs() < 1e-3,
            "dropped 秒数 = 成员语音之和，实测 {dropped}"
        );
        // 全留：两成员都 kept。
        let (kept2, kept2_secs, dropped2) = partition_ranges(
            &ranges,
            &[SegVerdict::KeepUser(0.9), SegVerdict::KeepUser(0.9)],
        );
        assert_eq!(kept2.len(), 2, "单元 Keep ⇒ 全部保留");
        assert!((kept2_secs - 2.3).abs() < 1e-3);
        assert_eq!(dropped2, 0.0);
    }

    /// 契约 1（影响①）：`kept` 返回的是**原各段**（合并只用于判定，不得把间隔并进剪静音区间）。
    #[test]
    fn ts412_kept_ranges_are_original_segments() {
        let ranges = [(0usize, secs(1.2)), (secs(1.5), secs(2.6))]; // 间隔 0.3s
        let (kept, _, _) = partition_ranges(&ranges, &[SegVerdict::KeepUser(0.9); 2]);
        assert_eq!(
            kept,
            ranges.to_vec(),
            "保留区间必须仍是原各段（不合并间隔）"
        );
    }

    /// 契约 3（影响⑤）：单元 offer 边界 —— 起点 ≥ `new_slice_from` 才 offer；**跨界单元不 offer**。
    #[test]
    fn ts412_unit_offer_boundary() {
        let at = SpeechUnit {
            members: vec![(secs(1.0), secs(3.0))],
            speech_samples: secs(2.0),
        };
        assert!(unit_offer_allowed(&at, secs(1.0)), "起点 == 边界 ⇒ offer");
        assert!(unit_offer_allowed(&at, secs(0.5)));
        assert!(
            !unit_offer_allowed(&at, secs(2.0)),
            "起点早于边界 ⇒ 不 offer"
        );
        // 跨界单元（起点在前文、延伸进新片）⇒ 按起点判 ⇒ 不 offer。
        let crossing = SpeechUnit {
            members: vec![(secs(0.5), secs(1.5)), (secs(2.0), secs(3.5))],
            speech_samples: secs(2.5),
        };
        assert!(
            !unit_offer_allowed(&crossing, secs(1.5)),
            "跨界单元保守不 offer"
        );
    }

    /// 契约（影响⑥）：无任何就绪档 ⇒ 各单元 KeepNotReady ⇒ 全部保留、dropped 0（与改前逐位一致）。
    #[test]
    fn ts412_no_ready_all_kept() {
        let vp = Voiceprint::default();
        assert!(!vp.has_any_ready());
        // 两段各 2s（各自成单元）⇒ 均 KeepNotReady ⇒ 全保留。
        assert_eq!(
            judge_voiceprint(&vp, Some(&[1.0, 0.0]), 2.0),
            SegVerdict::KeepNotReady
        );
        let ranges = [(0usize, secs(2.0)), (secs(3.0), secs(5.0))];
        let (kept, kept_secs, dropped) = partition_ranges(
            &ranges,
            &[SegVerdict::KeepNotReady, SegVerdict::KeepNotReady],
        );
        assert_eq!(kept, ranges.to_vec());
        assert!((kept_secs - 4.0).abs() < 1e-3 && dropped == 0.0);
    }

    /// 契约（对齐不变量）：合并后**成员扁平序列 == 输入区间序列**（含退化零长区间），
    /// 保证 `filter_ranges_by_voiceprint` 按成员顺序回填逐区间判定时不失配（防误剔/漏剔）。
    #[test]
    fn ts412_members_align_one_to_one_with_ranges() {
        let ranges = [
            (0, secs(0.8)),
            (secs(1.0), secs(1.0)), // 退化零长（中间）
            (secs(1.1), secs(1.9)),
        ];
        let units = merge_speech_units(&ranges);
        let flat: Vec<(usize, usize)> = units.iter().flat_map(|u| u.members.clone()).collect();
        assert_eq!(flat, ranges.to_vec(), "成员必须与输入区间一一对应");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].members.len(), 3);
        assert!(
            (units[0].speech_samples as f32 / SAMPLE_RATE as f32 - 1.6).abs() < 1e-3,
            "零长成员不计入语音时长"
        );
    }

    /// 契约（影响④）：单元变长只抬高**未触顶前**的漂移步长；单步位移恒受
    /// [`UPDATE_MAX_WEIGHT`](0.25) 封顶 ⇒ 8s 与 100s 单元位移**相同**（均触顶）。
    #[test]
    fn ts412_drift_long_unit_still_capped() {
        let target = at_cos(0.95);
        let mut vp8 = ready_vp();
        let before = vp8.profiles.get("zh").unwrap().centroid.clone();
        vp8.offer(&target, 8.0, Some(0.95), Some("zh"));
        let moved8 = cosine(&before, &vp8.profiles.get("zh").unwrap().centroid);
        let mut vp100 = ready_vp();
        vp100.offer(&target, 100.0, Some(0.95), Some("zh"));
        let moved100 = cosine(&before, &vp100.profiles.get("zh").unwrap().centroid);
        assert!(
            moved8 < 1.0,
            "单步位移必须 <1（受 0.25 拖拽），实测 {moved8}"
        );
        assert!(
            (moved8 - moved100).abs() < 1e-5,
            "8s 与 100s 单元均触 0.25 上限 ⇒ 位移应一致（{moved8} vs {moved100}）"
        );
        assert!(
            UPDATE_MAX_WEIGHT == 0.25,
            "412 不得改权重上限（仍 0.25），实测 {UPDATE_MAX_WEIGHT}"
        );
    }

    /// 契约（影响⑦）：声纹模型缺失 ⇒ 功能整体静默关闭：**原样保留全部语音**、dropped=0、
    /// 不产 offer（与改前逐位一致）。450 起用测试专用线程开关模拟「已尝试且不可用」。
    #[test]
    fn ts412_missing_model_feature_off_unchanged() {
        let ranges = [(0usize, secs(1.0)), (secs(1.2), secs(3.4))];
        super::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
        let f = filter_ranges_by_voiceprint(&[], &ranges, 0);
        super::TEST_EXTRACTOR_MISSING.with(|c| c.set(false));
        assert_eq!(f.kept, ranges.to_vec(), "模型缺失 ⇒ 原样保留全部区间");
        assert_eq!(f.dropped_secs, 0.0);
        assert!(
            f.pending_offers.is_empty(),
            "模型缺失 ⇒ 不产注册/漂移 offer"
        );
        assert!(!f.any_ready, "模型缺失 ⇒ any_ready=false");
    }

    /// `#[ignore]` 真模型：本人两段各 1.2s、间隔 0.3s 合并成的「单元」声纹，应与本人 2.4s
    /// 连续长段高度相似（证明合并足以判定），且不差于单段 1.2s。
    #[test]
    #[ignore = "requires CAM++ model + wav; cargo test --bin feiyin-ime -- --ignored --nocapture ts412_real_merge_short_segments"]
    fn ts412_real_merge_short_segments() {
        use std::path::PathBuf;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let Some(v) = super::SpeakerVerifier::load(&root.join("models")) else {
            eprintln!("skip: 模型不在位");
            return;
        };
        let p = root.join("collab/research/audio-real-gavin/processed/para1.wav");
        let Some(w) = sherpa_onnx::Wave::read(p.to_str().unwrap()) else {
            eprintln!("skip: wav 不在位（{}）", p.display());
            return;
        };
        let s = w.samples();
        let sr = SAMPLE_RATE as usize;
        let seg = (1.2 * sr as f32) as usize;
        let gap = (0.3 * sr as f32) as usize;
        let start = sr; // 从 1.0s 起，避开起始静音
        let need = start + 2 * seg + gap;
        if s.len() < need {
            eprintln!("skip: wav 太短（{} < {}），跳过", s.len(), need);
            return;
        }
        let a = (start, start + seg);
        let b = (start + seg + gap, start + 2 * seg + gap);
        let units = merge_speech_units(&[a, b]);
        assert_eq!(units.len(), 1, "0.3s 间隔应合并成一个单元");
        let mut buf = Vec::with_capacity(units[0].speech_samples);
        for &(s0, e0) in &units[0].members {
            buf.extend_from_slice(&s[s0..e0]);
        }
        let e_unit = v.embed(&buf).expect("embed unit");
        let e_long = v.embed(&s[start..start + 2 * seg]).expect("embed long");
        let e_short = v.embed(&s[a.0..a.1]).expect("embed short");
        let cos_ul = cosine(&e_unit, &e_long);
        let cos_us = cosine(&e_unit, &e_short);
        eprintln!(
            "[412] unit(1.2+1.1s, gap 0.3s) vs long(2.4s)={cos_ul:.3}, vs short(1.2s)={cos_us:.3}"
        );
        assert!(
            cos_ul > DROP_THR,
            "合并单元应像本人（>{DROP_THR}），实测 {cos_ul}"
        );
        assert!(cos_ul >= cos_us - 1e-3, "合并单元不应差于单短段");
    }
}

// =====================================================================
// TEST-SYNC-412（阶段三 · 非作者护栏，coder-2）：短段拼接 —— 独立性质/边界/影响面
//   作者 coder-1（`testsync412_merge_tests`）；本模块只补独立护栏，不照抄。
// =====================================================================
#[cfg(test)]
mod testsync412_guard_tests {
    use super::{
        judge_voiceprint, merge_speech_units, partition_ranges, unit_offer_allowed,
        unit_offer_eligible, SegVerdict, SpeechUnit, Voiceprint, MERGE_GAP_SECS, MIN_JUDGE_SECS,
        MIN_OFFER_SECS, SAMPLE_RATE,
    };

    fn secs(x: f32) -> usize {
        (x * SAMPLE_RATE as f32) as usize
    }
    fn gap_samples() -> usize {
        (MERGE_GAP_SECS * SAMPLE_RATE as f32) as usize
    }

    /// 契约1 边界：间隔**恰 0.8s**（== 阈值）不拼（`<` 严格）；0.799s 拼。
    #[test]
    fn ts412b_gap_exactly_threshold_not_merged() {
        // 446：判定门槛 1.0s ⇒ 用 <1.0s 的短段测「间隔」规则（≥1.0s 的段本身已可判、不再拼）。
        let a = (0usize, secs(0.6));
        let b_start = a.1 + gap_samples(); // 恰 0.8s
        let b = (b_start, b_start + secs(0.5));
        assert_eq!(
            merge_speech_units(&[a, b]).len(),
            2,
            "恰 0.8s 间隔应断开（严格 <）"
        );
        let c_start = a.1 + gap_samples() - 1; // 0.8s 差 1 样本
        let c = (c_start, c_start + secs(0.5));
        assert_eq!(merge_speech_units(&[a, c]).len(), 1, "0.8s 内应拼");
    }

    /// 契约1/2（随机性质，500 组）：成员扁平序==输入（一一对应、无丢无重）；`speech_samples`==成员语音之和；
    /// 单元内相邻成员间隔恒 < 0.8s；含退化零长段不 panic。
    #[test]
    fn ts412b_group_property_random() {
        struct Lcg(u64);
        impl Lcg {
            fn n(&mut self, lo: usize, hi: usize) -> usize {
                self.0 = self
                    .0
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (lo as u64 + (self.0 >> 33) % ((hi - lo + 1) as u64)) as usize
            }
        }
        let mut rng = Lcg(0x412_5EED);
        let gap_thr = gap_samples();
        for case in 0..500 {
            let n = rng.n(0, 8);
            let mut ranges: Vec<(usize, usize)> = Vec::new();
            let mut t = 0usize;
            for _ in 0..n {
                let len_ms = rng.n(0, 3000); // 0..=3s（含退化 0）
                let gap_ms = rng.n(0, 1200); // 0..=1.2s
                let s = t;
                let e = t + len_ms * 16; // ms→samples
                ranges.push((s, e));
                t = e + gap_ms * 16;
            }
            let units = merge_speech_units(&ranges);
            let flat: Vec<(usize, usize)> = units.iter().flat_map(|u| u.members.clone()).collect();
            assert_eq!(
                flat, ranges,
                "case {case}: 成员须与输入一一对应（无丢无重）"
            );
            for u in &units {
                assert!(!u.members.is_empty(), "case {case}: 单元不得为空");
                let sum: usize = u.members.iter().map(|(s, e)| e - s).sum();
                assert_eq!(
                    sum, u.speech_samples,
                    "case {case}: speech_samples==成员语音之和"
                );
                for w in u.members.windows(2) {
                    let gap = w[1].0.saturating_sub(w[0].1);
                    assert!(
                        gap < gap_thr,
                        "case {case}: 单元内间隔须 < 0.8s，实测 {gap}"
                    );
                }
            }
        }
    }

    /// 契约3（真实分布）：BUILD-409 各处间隔（0.29~1.96s）构造序列不 panic、成员一一对应、
    /// 间隔 ≥0.8s 处必然断开为不同单元。
    #[test]
    fn ts412b_build409_gaps_no_panic_and_split_at_threshold() {
        let gaps = [
            0.29f32, 0.51, 0.74, 0.48, 0.55, 0.64, 0.77, 0.54, 0.61, 0.67, 0.86, 1.21, 1.96,
        ];
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        let mut t = 0usize;
        for (i, g) in gaps.iter().enumerate() {
            let len = secs(0.6 + (i % 3) as f32 * 0.4); // 0.6~1.4s 短段
            let s = t;
            let e = t + len;
            ranges.push((s, e));
            t = e + secs(*g);
        }
        let units = merge_speech_units(&ranges);
        let flat: Vec<(usize, usize)> = units.iter().flat_map(|u| u.members.clone()).collect();
        assert_eq!(flat, ranges, "不 panic 且成员一一对应（无丢无重）");
        // 单元内相邻成员间隔恒 <0.8s（拼只在 <0.8s 发生；收尾并入亦然）。
        for u in &units {
            for w in u.members.windows(2) {
                let gap = w[1].0.saturating_sub(w[0].1);
                assert!(gap < gap_samples(), "单元内间隔须 <0.8s，实测 {gap}");
            }
        }
        assert!(!units.is_empty(), "非空输入应有单元");
    }

    /// 契约4（作用域，3 成员单元）：单元 Drop ⇒ **全部 3 成员**剔除，dropped==成员语音之和；
    /// Keep ⇒ kept==原各段（间隔不并入）。
    #[test]
    fn ts412b_unit_scope_three_members() {
        let ranges = [
            (0usize, secs(0.8)),
            (secs(1.0), secs(1.9)),
            (secs(2.1), secs(3.0)),
        ];
        // 期望 = 各段时长之和（由数据导出，勿手算常量）。
        let total: f32 = ranges
            .iter()
            .map(|(s, e)| (e - s) as f32 / SAMPLE_RATE as f32)
            .sum();
        let (kept, ksec, drop) = partition_ranges(&ranges, &[SegVerdict::DropNonUser(0.1); 3]);
        assert!(kept.is_empty(), "单元 Drop ⇒ 全成员剔除");
        assert_eq!(ksec, 0.0);
        assert!(
            (drop - total).abs() < 1e-3,
            "dropped==成员语音之和 {total}，实测 {drop}"
        );
        let (kept2, ksec2, drop2) = partition_ranges(&ranges, &[SegVerdict::KeepUser(0.9); 3]);
        assert_eq!(
            kept2,
            ranges.to_vec(),
            "保留=原各段（间隔不并入剪静音区间）"
        );
        assert!((ksec2 - total).abs() < 1e-3 && drop2 == 0.0);
    }

    /// 契约5：无就绪档 ⇒ 判定 KeepNotReady（与改前一致）；缺模型 ⇒ 生产早退还**原样保留**（源码护栏）。
    #[test]
    fn ts412b_no_ready_and_missing_model_source_guard() {
        let vp = Voiceprint::default();
        assert!(!vp.has_any_ready());
        assert_eq!(
            judge_voiceprint(&vp, Some(&[1.0, 0.0]), MIN_JUDGE_SECS),
            SegVerdict::KeepNotReady
        );
        // 源码护栏：filter 在提取器不可用时 `kept: ranges.to_vec()`（原样保留，功能整体关闭）。
        let src = include_str!("speaker.rs");
        let i = src
            .find("pub(crate) fn filter_ranges_by_voiceprint(")
            .expect("filter_ranges_by_voiceprint 锚点缺失");
        let body: String = src[i..].chars().take(1400).collect();
        assert!(
            body.contains("kept: ranges.to_vec()"),
            "缺模型 / 提取器不可用 ⇒ 必须原样保留 ranges（功能关闭）"
        );
    }

    /// 契约6（边界补充）：空成员合成单元 —— `members.first().unwrap_or(0)` ⇒ 仅 `new_slice_from==0` 时 offer。
    #[test]
    fn ts412b_unit_offer_allowed_empty_members() {
        let empty = SpeechUnit {
            members: Vec::new(),
            speech_samples: 0,
        };
        assert!(unit_offer_allowed(&empty, 0), "空成员起点视作 0");
        assert!(!unit_offer_allowed(&empty, 1));
    }

    /// 契约6（注册 / 432）：只有 **≥ [`MIN_OFFER_SECS`]（2.0s）且起点 ≥ `new_slice_from`** 的单元 offer；
    /// 1.5~2.0s（可判但嵌入差）**不 offer**；跨 `new_slice_from`（起点在前文）的单元不 offer。
    /// `unit_offer_allowed` 只管起点；时长门由 [`unit_offer_eligible`] 承担 —— 一并以源码护栏锁死。
    #[test]
    fn ts412b_offer_requires_offer_secs_and_in_window() {
        let u = SpeechUnit {
            members: vec![(secs(1.0), secs(3.2))],
            speech_samples: secs(2.2),
        };
        assert!(unit_offer_allowed(&u, secs(1.0)), "起点 == 边界 ⇒ offer");
        assert!(
            !unit_offer_allowed(&u, secs(1.5)),
            "起点早于边界 ⇒ 不 offer"
        );
        // 跨界单元（起点在前文、延伸进新片）⇒ 按起点判 ⇒ 不 offer。
        let cross = SpeechUnit {
            members: vec![(secs(0.5), secs(1.5)), (secs(2.0), secs(3.5))],
            speech_samples: secs(2.5),
        };
        assert!(!unit_offer_allowed(&cross, secs(1.0)), "跨界单元不 offer");
        assert!(unit_offer_allowed(&cross, secs(0.5)), "起点在窗内 ⇒ offer");
        // 432：注册门 = MIN_OFFER_SECS(2.0s)，**独立**于判定门 MIN_JUDGE_SECS(1.5s)。
        assert!(
            !unit_offer_eligible(MIN_JUDGE_SECS, &u, secs(1.0)),
            "1.5s（可判）仍**不注册**"
        );
        assert!(
            !unit_offer_eligible(MIN_OFFER_SECS - 0.001, &u, secs(1.0)),
            "略低于 2.0s ⇒ 不 offer"
        );
        assert!(
            unit_offer_eligible(MIN_OFFER_SECS, &u, secs(1.0)),
            "恰 2.0s 且起点在窗内 ⇒ offer"
        );
        // 源码护栏：调用处用 `unit_offer_eligible`（内置 2.0s 门 + 起点门）。
        let src = include_str!("speaker.rs");
        assert!(
            src.contains("if unit_offer_eligible(unit_secs, unit, new_slice_from) {"),
            "offer 调用处必须用 unit_offer_eligible（≥ MIN_OFFER_SECS 与起点门）"
        );
    }

    /// 契约3（窗 #4 真实区间 / 432 更新）：1.02/1.70/1.41/0.96s，间隔 0.64/0.77/0.54。
    /// 432 判定/合并门槛 1.5s ⇒ **恰 3 个单元**（1.02 / 1.70 / 2.37），其中 2 个（1.70、2.37）可判。
    #[test]
    fn ts412b_window4_exact_three_units_two_judgeable() {
        let g1 = 0.64f32;
        let g2 = 0.77f32;
        let g3 = 0.54f32;
        let at = |s: f32, l: f32| (secs(s), secs(s + l));
        let ranges = [
            at(0.0, 1.02),
            at(1.02 + g1, 1.70),
            at(1.02 + g1 + 1.70 + g2, 1.41),
            at(1.02 + g1 + 1.70 + g2 + 1.41 + g3, 0.96),
        ];
        let units = merge_speech_units(&ranges);
        assert_eq!(
            units.len(),
            3,
            "窗#4 应恰 3 个单元（446·1.0s 门槛，分组与 432 时相同）"
        );
        assert_eq!(units[0].members.len(), 1, "1.02s 独立");
        assert_eq!(units[1].members.len(), 1, "1.70s 独立");
        assert_eq!(
            units[2].members.len(),
            2,
            "1.41+0.96 ⇒ 2.37s（0.96 <1.0 尾段并入）"
        );
        let judgeable = units
            .iter()
            .filter(|u| u.speech_samples as f32 / SAMPLE_RATE as f32 >= MIN_JUDGE_SECS)
            .count();
        assert_eq!(
            judgeable, 3,
            "446：1.02 / 1.70 / 2.37 三个单元均可判（≥1.0s）"
        );
    }

    /// 契约2（前向累积阶段）：≥2s 单段**不被**后续**非近邻**（间隔 ≥0.8s）段拼接；单独 ≥2s 段自成
    /// 1 成员单元。**③ 收尾例外另测**（见下）。
    #[test]
    fn ts412b_ge2s_single_segment_independent_when_gap_ge_thr() {
        // 2.1s 单段 + 0.9s（≥0.8）间隔的 0.5s 段 ⇒ 两个单元。
        let units = merge_speech_units(&[(0, secs(2.1)), (secs(3.0), secs(3.0) + secs(0.5))]);
        assert_eq!(units.len(), 2, "间隔 ≥0.8s ⇒ ≥2s 单段独立");
        assert_eq!(units[0].members.len(), 1);
        assert!(units[0].speech_samples as f32 / SAMPLE_RATE as f32 >= MIN_JUDGE_SECS);
        assert_eq!(merge_speech_units(&[(0, secs(2.1))]).len(), 1);
    }

    /// ③ 收尾例外（主控裁量 A：有意行为，非缺陷）：**最后一个** <2s 且近邻（<0.8s）的段，无论前一单元
    /// 是否已 ≥2s，都并入前一单元（防尾段落单不判）。
    ///
    /// - `[2.1s, 0.3gap, 0.5s]`（0.5s 为**最后**一段）⇒ **1** 个单元（尾段并入 2.1s）。
    /// - `[2.1s, 0.3gap, 0.5s, 0.3gap, 2.5s]` ⇒ 中间 0.5s **不是**尾段 ⇒ 不并入 2.1s 单元，且末 2.5s 独立 ⇒ **3** 个单元。
    #[test]
    fn ts412b_tail_merge_exception_explicit() {
        let merged = merge_speech_units(&[(0, secs(2.1)), (secs(2.4), secs(2.4) + secs(0.5))]);
        assert_eq!(
            merged.len(),
            1,
            "③：2.1s + 近邻**尾段** 0.5s ⇒ 1 个单元（尾段并入）"
        );
        assert_eq!(merged[0].members.len(), 2, "成员 = [2.1s, 0.5s]");
        assert!(
            (merged[0].speech_samples as f32 / SAMPLE_RATE as f32 - 2.6).abs() < 1e-3,
            "语音总时长 2.6s（不含 0.3s 间隔）"
        );
        // 中间 0.5s（后面还有 2.5s）⇒ 非尾段 ⇒ 不并入 2.1s 单元。
        let three = merge_speech_units(&[
            (0, secs(2.1)),
            (secs(2.4), secs(2.4) + secs(0.5)),
            (secs(3.2), secs(3.2) + secs(2.5)),
        ]);
        assert_eq!(three.len(), 3, "中间 0.5s 自成单元、末 2.5s 独立");
        for u in &three {
            assert_eq!(
                u.members.len(),
                1,
                "三单元各 1 成员（前向 ≥2s 单元不吸收中间段）"
            );
        }
    }
}

// =====================================================================
// DIAG-SHORT-VOICEPRINT-432：1~2s 短句声纹能否区分本人 / 旁人（只读诊断，不改生产）
//   运行：cargo test --bin feiyin-ime -- --ignored --nocapture diag432
//   参考：target/release/voiceprint.bin（已注册 Gavin 声纹，**只读**）
// =====================================================================
#[cfg(test)]
mod diag432_tests {
    use super::{SpeakerVerifier, Voiceprint};
    use std::path::{Path, PathBuf};

    const SR: usize = 16000;

    fn manifest() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn read_wav(p: &Path) -> Vec<f32> {
        sherpa_onnx::Wave::read(p.to_str().expect("utf8 path"))
            .expect("read wav")
            .samples()
            .to_vec()
    }

    #[test]
    #[ignore = "diag432: cargo test --bin feiyin-ime -- --ignored --nocapture diag432"]
    fn diag432_short_voiceprint_scores() {
        let ver = SpeakerVerifier::load(&manifest().join("models")).expect("CAM++ 模型缺失");
        let vp = Voiceprint::load(&manifest().join("target/release/voiceprint.bin"));
        eprintln!(
            "[DIAG432] ref any_ready={} enrolled_secs={:.1}",
            vp.has_any_ready(),
            vp.total_ready_secs()
        );
        let g = manifest().join("collab/evidence/gavin-sessions");
        let score = |wav: &[f32], start_s: f32, secs: f32| -> Option<(f32, f32)> {
            let s = (start_s * SR as f32) as usize;
            let n = (secs * SR as f32) as usize;
            if n == 0 || s + n > wav.len() {
                return None;
            }
            let seg = &wav[s..s + n];
            let rms = (seg.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
            let emb = ver.embed(seg)?;
            vp.max_score_ready(&emb).map(|sc| (sc, rms))
        };
        let lens = [0.5f32, 0.7, 1.0, 1.5, 1.8, 2.0];
        // 本人：Gavin 照稿/自述录音。
        for f in [
            "session-20260925-175022.wav",
            "session-20260925-175535.wav",
            "session-20260925-173634.wav",
            "session-20260925-211203.wav",
        ] {
            let wav = read_wav(&g.join(f));
            let dur = wav.len() as f32 / SR as f32;
            for &l in &lens {
                for k in 0..10 {
                    let start = (dur * (k as f32 + 1.0) / 12.0).min((dur - l).max(0.0));
                    if let Some((s, r)) = score(&wav, start, l) {
                        eprintln!("[DIAG432] OWN file={f} secs={l:.2} rms={r:.4} score={s:.4}");
                    }
                }
            }
        }
        // 旁人：150350（他人音频）整体。
        {
            let f = "session-20260925-150350.wav";
            let wav = read_wav(&g.join(f));
            let dur = wav.len() as f32 / SR as f32;
            for &l in &lens {
                for k in 0..10 {
                    let start = (dur * (k as f32 + 1.0) / 12.0).min((dur - l).max(0.0));
                    if let Some((s, r)) = score(&wav, start, l) {
                        eprintln!("[DIAG432] OTHER file={f} secs={l:.2} rms={r:.4} score={s:.4}");
                    }
                }
            }
        }
        // 具体旁人短句（211641 窗#2 range 8.45-10.05s=1.60s；绝对位置按 win#1 pcm 388790 估算 ~32.7s）。
        {
            let wav = read_wav(&g.join("session-20260925-211641.wav"));
            for (start, l) in [(32.70f32, 1.60f32), (32.90, 1.40), (33.10, 1.00)] {
                if let Some((s, r)) = score(&wav, start, l) {
                    eprintln!("[DIAG432] OTHER file=211641-w2-range start={start} secs={l:.2} rms={r:.4} score={s:.4}");
                }
            }
        }
    }
}

// =====================================================================
// DIAG-SHORT-VOICEPRINT-432 R1：多说话人验证（AISHELL-1 6 人，轮流当使用人）
//   运行：cargo test --bin feiyin-ime -- --ignored --nocapture diag432r1
//   语料：models/speaker-432-scratch/train/S0002..S0007（AISHELL-1，Apache-2.0，只读临时）
// =====================================================================
#[cfg(test)]
mod diag432r1_tests {
    use super::{SpeakerVerifier, Voiceprint};
    use std::path::{Path, PathBuf};

    const SR: usize = 16000;

    fn manifest() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn read_wav(p: &Path) -> Vec<f32> {
        sherpa_onnx::Wave::read(p.to_str().unwrap())
            .expect("wav")
            .samples()
            .to_vec()
    }

    fn list_wavs(dir: &Path) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "wav").unwrap_or(false))
            .collect();
        v.sort();
        v
    }

    #[test]
    #[ignore = "diag432r1: cargo test --bin feiyin-ime -- --ignored --nocapture diag432r1"]
    fn diag432r1_multispeaker() {
        let root = manifest().join("models/speaker-432-scratch/train");
        let spk = ["S0002", "S0003", "S0004", "S0005", "S0006", "S0007"];
        let ver = SpeakerVerifier::load(&manifest().join("models")).expect("CAM++ 模型缺失");

        let mut enroll: Vec<Vec<(f32, Vec<f32>)>> = vec![Vec::new(); spk.len()];
        let mut tests: Vec<(usize, f32, Vec<f32>)> = Vec::new();
        for (si, name) in spk.iter().enumerate() {
            for (i, f) in list_wavs(&root.join(name)).iter().enumerate() {
                let wav = read_wav(f);
                let d = wav.len() as f32 / SR as f32;
                if i < 8 {
                    if d >= 2.0 {
                        let take = ((d.min(5.0)) * SR as f32) as usize;
                        if let Some(e) = ver.embed(&wav[..take.min(wav.len())]) {
                            enroll[si].push((take as f32 / SR as f32, e));
                        }
                    }
                    continue;
                }
                if i >= 8 + 20 {
                    break;
                }
                for l in [1.0f32, 1.5, 1.8] {
                    if d + 1e-3 >= l {
                        let n = (l * SR as f32) as usize;
                        if let Some(e) = ver.embed(&wav[..n]) {
                            tests.push((si, l, e));
                        }
                    }
                }
            }
        }
        eprintln!(
            "[DIAG432R1] speakers={} enroll={:?} tests={}",
            spk.len(),
            enroll.iter().map(|v| v.len()).collect::<Vec<_>>(),
            tests.len()
        );
        let empty = manifest().join("target/debug/__nonexistent_vp_432.bin");
        let thrs = [0.30f32, 0.35, 0.40, 0.45];
        let mut pooled_own: Vec<(f32, f32)> = Vec::new();
        let mut pooled_other: Vec<(f32, f32)> = Vec::new();
        for u in 0..spk.len() {
            let mut vp = Voiceprint::load(&empty);
            for (secs, e) in &enroll[u] {
                vp.offer(e, *secs, None, Some("zh"));
            }
            if !vp.has_any_ready() {
                eprintln!("[DIAG432R1] USER {} NOT_READY", spk[u]);
                continue;
            }
            let (mut own, mut other): (Vec<(f32, f32)>, Vec<(f32, f32)>) = (Vec::new(), Vec::new());
            for (si, l, e) in &tests {
                if let Some(sc) = vp.max_score_ready(e) {
                    if *si == u {
                        own.push((*l, sc));
                    } else {
                        other.push((*l, sc));
                    }
                }
            }
            let stat = |v: &[(f32, f32)]| -> (usize, f32, f32, f32) {
                if v.is_empty() {
                    return (0, 0.0, 0.0, 0.0);
                }
                let mut s: Vec<f32> = v.iter().map(|x| x.1).collect();
                s.sort_by(|a, b| a.partial_cmp(b).unwrap());
                (s.len(), s[0], s[s.len() / 2], s[s.len() - 1])
            };
            let (on, omn, omed, omx) = stat(&own);
            let (tn, tmn, tmed, tmx) = stat(&other);
            eprintln!("[DIAG432R1] USER {} OWN n={} min={:.3} med={:.3} max={:.3} | OTH n={} min={:.3} med={:.3} max={:.3}", spk[u], on, omn, omed, omx, tn, tmn, tmed, tmx);
            for &t in &thrs {
                let frr = own.iter().filter(|x| x.1 < t).count() as f32 / on.max(1) as f32;
                let dr = other.iter().filter(|x| x.1 < t).count() as f32 / tn.max(1) as f32;
                eprintln!(
                    "[DIAG432R1] USER {} thr={:.2} own_FRR={:.4} other_drop={:.4}",
                    spk[u], t, frr, dr
                );
            }
            for (lo, hi, tag) in [(1.0f32, 1.49f32, "1.0-1.5"), (1.49f32, 1.81f32, "1.5-1.8")] {
                let ob: Vec<f32> = own
                    .iter()
                    .filter(|x| lo <= x.0 && x.0 < hi)
                    .map(|x| x.1)
                    .collect();
                let tb: Vec<f32> = other
                    .iter()
                    .filter(|x| lo <= x.0 && x.0 < hi)
                    .map(|x| x.1)
                    .collect();
                if !ob.is_empty() || !tb.is_empty() {
                    let omin = ob.iter().cloned().fold(f32::INFINITY, f32::min);
                    let omax = tb.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    eprintln!(
                        "[DIAG432R1] USER {} bucket {} own_n={} own_min={:.3} oth_n={} oth_max={:.3}",
                        spk[u],
                        tag,
                        ob.len(),
                        omin,
                        tb.len(),
                        omax
                    );
                }
            }
            pooled_own.extend(own.iter().cloned());
            pooled_other.extend(other.iter().cloned());
        }
        let pool = |v: &[(f32, f32)]| {
            for &t in &thrs {
                let frr = v.iter().filter(|x| x.1 < t).count() as f32 / v.len().max(1) as f32;
                eprintln!(
                    "[DIAG432R1] POOLED n={} thr={:.2} below={:.4}",
                    v.len(),
                    t,
                    frr
                );
            }
            let mut s: Vec<f32> = v.iter().map(|x| x.1).collect();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if !s.is_empty() {
                eprintln!(
                    "[DIAG432R1] POOLED min={:.3} med={:.3} max={:.3}",
                    s[0],
                    s[s.len() / 2],
                    s[s.len() - 1]
                );
            }
        };
        eprintln!("[DIAG432R1] == POOLED OWN ==");
        pool(&pooled_own);
        eprintln!("[DIAG432R1] == POOLED OTHERS ==");
        pool(&pooled_other);
    }
}

// DIAG-432-R1 补充：在 211641 末尾按能量找最后一段语音（1.60s 窗）并打分。
#[cfg(test)]
mod diag432r1_locate_tests {
    use super::{SpeakerVerifier, Voiceprint};
    use std::path::PathBuf;
    const SR: usize = 16000;
    fn manifest() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }
    #[test]
    #[ignore = "diag432r1: cargo test --bin feiyin-ime -- --ignored --nocapture diag432r1_locate"]
    fn diag432r1_locate_sentence() {
        let p = manifest().join("collab/evidence/gavin-sessions/session-20260925-211641.wav");
        let wav = sherpa_onnx::Wave::read(p.to_str().unwrap())
            .unwrap()
            .samples()
            .to_vec();
        let ver = SpeakerVerifier::load(&manifest().join("models")).unwrap();
        let vp = Voiceprint::load(&manifest().join("target/release/voiceprint.bin"));
        let dur = wav.len() as f32 / SR as f32;
        eprintln!(
            "[DIAG432R1-LOC] wav dur={dur:.2}s ref_ready={}",
            vp.has_any_ready()
        );
        let n = (1.6 * SR as f32) as usize;
        let mut t = 0.0;
        while t + 1.6 <= dur {
            let s = (t * SR as f32) as usize;
            let seg = &wav[s..s + n];
            let rms = (seg.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
            let sc = ver
                .embed(seg)
                .and_then(|e| vp.max_score_ready(&e))
                .unwrap_or(-1.0);
            if rms > 0.01 {
                eprintln!("[DIAG432R1-LOC] start={t:.2} rms={rms:.4} score={sc:.4}");
            }
            t += 0.2;
        }
    }
}

/// VOICEPRINT-JUDGE-1P5S-432：判定门槛 2.0→1.5s，注册/漂移 offer 保持 2.0s（拆两常数）。
#[cfg(test)]
mod fix432_tests {
    use super::{
        judge, judge_voiceprint, unit_offer_eligible, LangProfile, SegVerdict, SpeechUnit,
        Voiceprint, DROP_THR, ENROLL_MIN_SECS, ENROLL_MIN_SEGS, MIN_JUDGE_SECS, MIN_OFFER_SECS,
        SAMPLE_RATE,
    };

    fn secs(x: f32) -> usize {
        (x * SAMPLE_RATE as f32) as usize
    }
    fn at_cos(c: f32) -> Vec<f32> {
        vec![c, (1.0 - c * c).max(0.0).sqrt()]
    }
    fn ready_zh() -> Voiceprint {
        let mut vp = Voiceprint::default();
        vp.profiles.insert(
            "zh".to_string(),
            LangProfile {
                centroid: vec![1.0, 0.0],
                total_secs: ENROLL_MIN_SECS,
                segments: ENROLL_MIN_SEGS,
                candidates: Vec::new(),
            },
        );
        vp
    }

    /// 常数契约：判定 1.5s / 注册 offer 2.0s / DROP_THR 不动。
    #[test]
    fn fix432_constants_split() {
        assert_eq!(
            MIN_JUDGE_SECS, 1.0,
            "446：判定门槛 1.5s→1.0s（POC-444 数据）"
        );
        assert_eq!(MIN_OFFER_SECS, 2.0, "432：注册 offer 门槛保持 2.0s");
        assert_eq!(DROP_THR, 0.45, "DROP_THR 不动");
        assert!(MIN_OFFER_SECS > MIN_JUDGE_SECS, "注册门须严于判定门");
        let src = include_str!("speaker.rs");
        assert!(
            src.contains("pub(crate) const MIN_JUDGE_SECS: f32 = 1.0;"),
            "MIN_JUDGE_SECS 定义应为 1.0（446）"
        );
        assert!(
            src.contains("pub(crate) const MIN_OFFER_SECS: f32 = 2.0;"),
            "MIN_OFFER_SECS 定义应为 2.0"
        );
    }

    /// 验收①：1.49s ⇒ KeepShort（低于判定门槛一律保留）。
    #[test]
    fn fix432_1p49s_is_keep_short() {
        // 446：门槛 1.0s ⇒ 0.99s 仍 KeepShort；1.49s（432 时代的临界下方）现在进入判定并被剔。
        assert_eq!(
            judge(&ready_zh(), Some(&at_cos(0.1)), 0.99, Some("zh")),
            SegVerdict::KeepShort
        );
        assert_eq!(
            judge(&ready_zh(), Some(&at_cos(0.1)), 1.49, Some("zh")),
            SegVerdict::DropNonUser(0.1)
        );
    }

    /// 验收②：1.5s 且最高分 <0.45 ⇒ DropNonUser（判定下限生效）。
    #[test]
    fn fix432_1p5s_low_score_drops() {
        match judge(&ready_zh(), Some(&at_cos(0.3)), 1.5, Some("zh")) {
            SegVerdict::DropNonUser(s) => assert!(s < DROP_THR),
            other => panic!("1.5s 低分应 DropNonUser，实测 {other:?}"),
        }
        assert!(matches!(
            judge_voiceprint(&ready_zh(), Some(&at_cos(0.3)), 1.5),
            SegVerdict::DropNonUser(_)
        ));
    }

    /// 1.5s 且高分 ⇒ KeepUser（降门不误删本人）。
    #[test]
    fn fix432_1p5s_high_score_keeps() {
        match judge(&ready_zh(), Some(&at_cos(0.9)), 1.5, Some("zh")) {
            SegVerdict::KeepUser(s) => assert!(s >= DROP_THR),
            other => panic!("1.5s 高分应 KeepUser，实测 {other:?}"),
        }
    }

    /// 验收③：1.5~2.0s 单元**可判但不 offer 注册**；≥2.0s offer 不变。
    #[test]
    fn fix432_offer_gate_1p5_to_2s_excluded() {
        let unit = SpeechUnit {
            members: vec![(0, secs(2.0))],
            speech_samples: secs(2.0),
        };
        assert!(
            !unit_offer_eligible(MIN_JUDGE_SECS, &unit, 0),
            "1.5s（可判）不注册"
        );
        assert!(!unit_offer_eligible(1.99, &unit, 0), "1.99s 仍不注册");
        assert!(
            unit_offer_eligible(MIN_OFFER_SECS, &unit, 0),
            "恰 2.0s 恢复正常 offer"
        );
        assert!(unit_offer_eligible(2.5, &unit, 0), "≥2.0s offer 不变");
    }

    /// 验收④：412 合并目标随判定门槛 ⇒ 凑够 1.5s 即封口（下一段另起）。
    #[test]
    fn fix432_merge_seals_at_1p5s() {
        let units = super::merge_speech_units(&[
            (0, secs(0.8)),
            (secs(1.0), secs(1.5)),
            (secs(1.7), secs(2.2)),
            (secs(3.0), secs(3.5)),
        ]);
        // 446：封口点随判定门槛 1.0s ⇒ 前两段 0.8+0.5=1.3s 即封口，其后两段各自独立。
        let s0 = units[0].speech_samples as f32 / SAMPLE_RATE as f32;
        assert!(
            s0 >= MIN_JUDGE_SECS && units[0].members.len() == 2,
            "前两段拼至 1.3s 封口"
        );
        assert_eq!(
            units.len(),
            3,
            "封口后第三段另起、第四段间隔 0.8 不近邻也另起"
        );
    }
}
