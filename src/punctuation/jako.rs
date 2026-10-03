//! PUNCT-JAKO-466（DEC-102）：日韩文专用标点 —— punct_cap_seg_47lang int8（47 语种 6 层 Transformer）。
//!
//! Gavin 2026-10-03：「中文英文继续用现在的标点模型，日韩文换用候选模型的压缩版。但是日韩文的标点模型
//! 要采用延迟加载，不要程序一启动就加载。如果探测到用户输入的是日文或者韩文，才会加载对应的标点模型」。
//!
//! - **只在判出日韩文之后调用**（[`super::ct_unsupported_script`] 为真的分支）；首次调用才加载模型
//!   （进程内一次，约 0.16s），加载失败只记一次日志、此后原样返回，不反复重试。
//! - 中英文仍走 CT-Transformer（实测中文该模型更差：位置 F1 84.7 → 76.1，见 `collab/evidence/465/`）。
//! - 推理照参考实现 `punctuators`：小写后 SentencePiece 分词 → 每窗 126 子词、重叠 16（首尾各丢 8）→
//!   取「子词后标点」预测。🔴 与参考实现不同：标点**按原文字节位置插回原文**，不从子词拼文本 ——
//!   原文大小写 / 空格逐字保留，也不会冒出 `<Unk>`。
//! - ONNX Runtime 用 `ort` 的 load-dynamic 复用 sherpa-onnx 自带的 onnxruntime（不另打包）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use sentencepiece::SentencePieceProcessor;

/// 模型目录（exe 同级 `models/` 下）。
pub(crate) const JAKO_PUNCT_SUBDIR: &str = "punct-cap-seg-47lang-int8";
const MODEL_FILE: &str = "model.int8.onnx";
const SPM_FILE: &str = "spe_unigram_64k_lowercase_47lang.model";
/// 模型最大序列长（含首尾标记，`config.yaml` 的 `max_length`）。
const MAX_LEN: usize = 128;
/// 相邻窗重叠的子词数（参考实现 `infer(overlap=16)`；拼接时两侧各丢一半）。
const OVERLAP: usize = 16;
/// `config.yaml` 的 `post_labels`（下标 = 模型输出类别；0 = 不加标点）。
const POST_LABELS: [&str; 16] = [
    "", ".", ",", "?", "？", "，", "。", "、", "・", "।", "؟", "،", ";", "።", "፣", "፧",
];

struct JaKoPunct {
    session: ort::session::Session,
    sp: SentencePieceProcessor,
    bos: i64,
    eos: i64,
}

fn ort_err<E: std::fmt::Display>(e: E) -> anyhow::Error {
    anyhow!("{e}")
}

/// sherpa-onnx 随包的 onnxruntime 动态库（进程启动时已被 sherpa 加载；按同名取到的是同一份）。
fn ort_dylib() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    #[cfg(target_os = "windows")]
    let name = "onnxruntime.dll";
    #[cfg(target_os = "macos")]
    let name = "libonnxruntime.dylib";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let name = "libonnxruntime.so";
    if let Some(dir) = exe_dir {
        let p = dir.join(name);
        if p.exists() {
            return p;
        }
        // macOS：sherpa 随包的文件名带版本号（libonnxruntime.1.x.y.dylib）。
        #[cfg(target_os = "macos")]
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let f = e.file_name().to_string_lossy().to_string();
                if f.starts_with("libonnxruntime") && f.ends_with(".dylib") {
                    return e.path();
                }
            }
        }
    }
    PathBuf::from(name)
}

impl JaKoPunct {
    fn load(dir: &Path) -> Result<Self> {
        let model = dir.join(MODEL_FILE);
        let spm = dir.join(SPM_FILE);
        for p in [&model, &spm] {
            if !p.exists() {
                anyhow::bail!("日韩标点模型文件缺失: {}", p.display());
            }
        }
        // 重复调用无害：已初始化时 ort 直接复用现有环境。
        let _ = ort::init_from(ort_dylib()).map_err(ort_err)?.commit();
        let sp = SentencePieceProcessor::open(&spm).context("加载日韩标点分词模型失败")?;
        let bos = sp.bos_id().context("分词模型缺 BOS")? as i64;
        let eos = sp.eos_id().context("分词模型缺 EOS")? as i64;
        let mut builder = ort::session::Session::builder()
            .map_err(ort_err)?
            .with_intra_threads(1)
            .map_err(ort_err)?;
        let session = builder.commit_from_file(&model).map_err(ort_err)?;
        Ok(Self {
            session,
            sp,
            bos,
            eos,
        })
    }

    /// 给整段文本加标点（原文逐字保留，只插入标点）。
    fn punctuate(&mut self, text: &str) -> Result<String> {
        let (lower, end_map) = lower_with_map(text);
        let pieces = self.sp.encode(&lower).map_err(ort_err)?;
        if pieces.is_empty() {
            return Ok(text.to_string());
        }
        let ids: Vec<i64> = pieces.iter().map(|p| p.id as i64).collect();
        let mut labels = vec![0usize; ids.len()];
        let wins = windows(ids.len(), MAX_LEN - 2, OVERLAP);
        let last = wins.len() - 1;
        for (w, &(a, b)) in wins.iter().enumerate() {
            let mut input = Vec::with_capacity(b - a + 2);
            input.push(self.bos);
            input.extend_from_slice(&ids[a..b]);
            input.push(self.eos);
            let n = input.len();
            let tensor = ort::value::Tensor::from_array(([1usize, n], input)).map_err(ort_err)?;
            let outputs = self
                .session
                .run(ort::inputs!["input_ids" => tensor])
                .map_err(ort_err)?;
            let (_, post) = outputs["post_preds"]
                .try_extract_tensor::<i64>()
                .map_err(ort_err)?;
            // 去掉首尾标记；非首窗丢前 OVERLAP/2、非末窗丢后 OVERLAP/2（与参考实现拼接一致）。
            let keep_from = if w > 0 { OVERLAP / 2 } else { 0 };
            let keep_to = if w < last {
                (b - a) - OVERLAP / 2
            } else {
                b - a
            };
            for k in keep_from..keep_to {
                labels[a + k] = post.get(k + 1).copied().unwrap_or(0).max(0) as usize;
            }
        }
        let marks: Vec<(usize, &'static str)> = pieces
            .iter()
            .zip(&labels)
            .filter(|(p, &l)| {
                l > 0 && l < POST_LABELS.len() && !p.piece.trim_start_matches('\u{2581}').is_empty()
            })
            .filter_map(|(p, &l)| {
                let e = p.span.1 as usize;
                (e > 0 && e <= end_map.len()).then(|| (end_map[e - 1], POST_LABELS[l]))
            })
            .collect();
        Ok(insert_marks(text, &marks))
    }
}

/// 小写化并记下「小写串每个字节 → 原文中所属字符的结束字节」，供把子词结束位置映射回原文。
fn lower_with_map(text: &str) -> (String, Vec<usize>) {
    let mut lower = String::with_capacity(text.len());
    let mut end_map = Vec::with_capacity(text.len());
    for (i, ch) in text.char_indices() {
        let end = i + ch.len_utf8();
        for lc in ch.to_lowercase() {
            lower.push(lc);
            end_map.extend(std::iter::repeat_n(end, lc.len_utf8()));
        }
    }
    (lower, end_map)
}

/// 分窗（参考实现 `TextInferenceDataset._tokenize_inputs`）：每窗 `win` 个子词，除首窗外起点回退 `overlap`。
fn windows(n: usize, win: usize, overlap: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < n {
        let a = if out.is_empty() {
            start
        } else {
            start - overlap
        };
        let b = (a + win).min(n);
        out.push((a, b));
        start = a + win;
    }
    out
}

/// 在原文字节位置插入标点；同一位置只插一个，位置前后已有标点则不插（不叠标点）。
fn insert_marks(text: &str, marks: &[(usize, &str)]) -> String {
    let mut at: BTreeMap<usize, &str> = BTreeMap::new();
    for &(pos, m) in marks {
        if pos > text.len() || !text.is_char_boundary(pos) {
            continue;
        }
        let before = text[..pos].chars().next_back();
        let after = text[pos..].chars().next();
        if before.is_some_and(super::is_punctuation) || after.is_some_and(super::is_punctuation) {
            continue;
        }
        at.entry(pos).or_insert(m);
    }
    let mut out = String::with_capacity(text.len() + at.len() * 3);
    let mut prev = 0usize;
    for (&pos, m) in &at {
        out.push_str(&text[prev..pos]);
        out.push_str(m);
        prev = pos;
    }
    out.push_str(&text[prev..]);
    out
}

static JAKO: OnceLock<Mutex<Option<JaKoPunct>>> = OnceLock::new();

fn init_cell(dir: &Path) -> Mutex<Option<JaKoPunct>> {
    let t0 = Instant::now();
    match JaKoPunct::load(dir) {
        Ok(m) => {
            log::info!(
                "[PUNCT-JAKO-466] ja/ko punctuation model loaded on first use in {:.0}ms",
                t0.elapsed().as_secs_f64() * 1000.0
            );
            Mutex::new(Some(m))
        }
        Err(e) => {
            log::warn!(
                "[PUNCT-JAKO-466] ja/ko punctuation model load failed (text kept as-is): {e:#}"
            );
            Mutex::new(None)
        }
    }
}

fn default_dir() -> PathBuf {
    crate::transcription::model_dir().join(JAKO_PUNCT_SUBDIR)
}

/// 给日韩文加标点（最终输出用：首次调用**同步**加载模型，约 0.5s，只此一次）。
/// 🔴 只在 [`super::ct_unsupported_script`] 为真之后调用。
/// 模型缺失 / 加载失败 / 推理出错 ⇒ `None`（调用方原样保留文本）。
pub(crate) fn punctuate(text: &str) -> Option<String> {
    let cell = JAKO.get_or_init(|| init_cell(&default_dir()));
    run_on(cell, text)
}

/// 预览用：模型还没加载 ⇒ **后台**起加载、本次返回 `None`（原样显示），不让首次出现的日韩文卡住预览；
/// 加载完成后的刷新照常带标点。🔴 同样只在判出日韩文之后调用（后台加载也只在此刻触发）。
pub(crate) fn punctuate_if_ready(text: &str) -> Option<String> {
    match JAKO.get() {
        Some(cell) => run_on(cell, text),
        None => {
            if !LOADING.swap(true, Ordering::AcqRel) {
                std::thread::spawn(|| {
                    let _ = JAKO.get_or_init(|| init_cell(&default_dir()));
                });
            }
            None
        }
    }
}

/// 后台加载只起一次。
static LOADING: AtomicBool = AtomicBool::new(false);

fn run_on(cell: &Mutex<Option<JaKoPunct>>, text: &str) -> Option<String> {
    let mut guard = cell.lock().ok()?;
    let model = guard.as_mut()?;
    match model.punctuate(text) {
        Ok(s) => Some(s),
        Err(e) => {
            log::warn!("[PUNCT-JAKO-466] ja/ko punctuation failed (text kept as-is): {e:#}");
            None
        }
    }
}

/// 模型是否已加载过（含加载失败）—— 延迟加载的可观测点。
#[allow(dead_code)]
pub(crate) fn initialized() -> bool {
    JAKO.get().is_some()
}

/// 测试入口：从指定目录预先初始化（测试进程 exe 旁没有 `models/`）。
#[cfg(test)]
pub(crate) fn init_for_test(models_dir: &Path) {
    JAKO.get_or_init(|| init_cell(&models_dir.join(JAKO_PUNCT_SUBDIR)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jako466_windows_match_reference_overlap() {
        assert_eq!(windows(0, 126, 16), vec![]);
        assert_eq!(windows(50, 126, 16), vec![(0, 50)]);
        assert_eq!(windows(126, 126, 16), vec![(0, 126)]);
        // 第二窗起点回退 16；拼接时首窗丢尾 8、次窗丢头 8 ⇒ 正好在 118 接上。
        assert_eq!(windows(200, 126, 16), vec![(0, 126), (110, 200)]);
        assert_eq!(
            windows(300, 126, 16),
            vec![(0, 126), (110, 236), (220, 300)]
        );
    }

    #[test]
    fn jako466_lower_map_points_to_original_char_end() {
        let (lower, map) = lower_with_map("Ab한");
        assert_eq!(lower, "ab한");
        assert_eq!(map, vec![1, 2, 5, 5, 5]);
    }

    #[test]
    fn jako466_insert_marks_keeps_text_and_no_double_punct() {
        let t = "어제는 비가 와서 오늘은 맑아요";
        let e1 = "어제는 비가 와서".len();
        let e2 = t.len();
        assert_eq!(
            insert_marks(t, &[(e1, ","), (e2, ".")]),
            "어제는 비가 와서, 오늘은 맑아요."
        );
        // 已有标点处不再插；同一位置只插一个；越界 / 非字符边界忽略。
        assert_eq!(insert_marks("好。是", &[(3, "，"), (6, "，")]), "好。是");
        assert_eq!(insert_marks("ab", &[(1, "."), (1, ",")]), "a.b");
        assert_eq!(insert_marks("한", &[(1, "."), (9, ".")]), "한");
    }

    /// 466-4 护栏（延迟加载）：日韩标点只在判出日韩文的分支里调用 —— 每个 `jako::punctuate*` 调用点
    /// 前 4 行内必须有 `ct_unsupported_script(`。（预加载入口 `init_for_test` 带 cfg-test，生产代码调不到。）
    #[test]
    fn jako466_only_called_after_jako_detection() {
        for (name, src) in [
            ("main.rs", include_str!("../main.rs")),
            (
                "local_stream.rs",
                include_str!("../transcription/local_stream.rs"),
            ),
        ] {
            let lines: Vec<&str> = src.lines().collect();
            let calls: Vec<usize> = (0..lines.len())
                .filter(|&i| {
                    let l = lines[i].trim_start();
                    !l.starts_with("//") && lines[i].contains("jako::punctuate")
                })
                .collect();
            assert!(!calls.is_empty(), "{name}: 应接入日韩标点");
            for i in calls {
                let lo = i.saturating_sub(4);
                assert!(
                    lines[lo..=i]
                        .iter()
                        .any(|l| l.contains("ct_unsupported_script(")),
                    "{name}:{} 日韩标点调用前须先判日韩文（延迟加载的唯一入口）",
                    i + 1
                );
            }
        }
    }

    /// 真模型（--ignored）：与参考实现（punctuators，int8）的标点位置一致；原文大小写 / 空格保留、无 `<Unk>`。
    /// 运行：`cargo test --bin feiyin-ime -- --ignored --nocapture jako466_real_model`
    #[test]
    #[ignore = "需日韩标点模型；--ignored 运行"]
    fn jako466_real_model() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("models")
            .join(JAKO_PUNCT_SUBDIR);
        let t0 = Instant::now();
        let mut m = JaKoPunct::load(&dir).expect("日韩标点模型须在位");
        println!("[466] 加载 {:.0}ms", t0.elapsed().as_secs_f64() * 1000.0);
        let cases = [
            (
                "어제는 비가 와서 집에서 영화를 봤는데 오늘은 날씨가 좋아서 공원에 갈 생각이에요 내일은 뭐 할까요",
                "어제는 비가 와서 집에서 영화를 봤는데, 오늘은 날씨가 좋아서 공원에 갈 생각이에요. 내일은 뭐 할까요?",
            ),
            (
                "조금만 생각을 하면서 살면 훨씬 편할 거야",
                "조금만 생각을 하면서 살면 훨씬 편할 거야.",
            ),
        ];
        for (input, want) in cases {
            let t1 = Instant::now();
            let got = m.punctuate(input).expect("推理");
            println!(
                "[466] {:.1}ms {input}\n[466]   ⇒ {got}",
                t1.elapsed().as_secs_f64() * 1000.0
            );
            assert_eq!(got, want);
        }
        // 日文：分词与参考实现逐个相同（35 个子词 id 一致），35 处标点判断 34 处相同；
        // 唯一差异「晴れているので」后 ORT 1.28.2 判「。」、Python ORT 1.30 判「、」—— int8 近似并列分数的
        // 运行库数值差，非移植偏差 ⇒ 只断言稳定部分。
        let ja = m
            .punctuate("昨日は雨が降っていたので家で映画を見ましたが今日は晴れているので公園に行くつもりです明日は何をしましょうか")
            .expect("推理");
        println!("[466]   ⇒ {ja}");
        assert!(
            ja.starts_with("昨日は雨が降っていたので。家で映画を見ましたが、今日は晴れているので")
        );
        assert!(ja.ends_with("公園に行くつもりです。明日は何をしましょうか。"));
        // OOV 字符不产生 <Unk>、英文大小写原样保留。
        let got = m
            .punctuate("Zoom 会議は1/8に延期になりました")
            .expect("推理");
        println!("[466]   ⇒ {got}");
        assert!(!got.contains("<Unk>") && got.contains("Zoom") && got.contains("1/8"));
    }
}
