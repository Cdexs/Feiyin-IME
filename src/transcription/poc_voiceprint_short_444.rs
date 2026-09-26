//! POC-VOICEPRINT-SHORT-444：离线测声纹对**短片段**与**混合单元**的判别力（只测不改生产）。
//!
//! Gavin 2026-09-26：旁放他人语音端测，声纹零剔除（干扰多为 <1.5s 短片 ⇒ `KeepShort` 不判，
//! 或与本人拼进同一单元 ⇒ 得分仍 ≥0.82）。「① 只做离线测试不改代码」。
//!
//! 数据：`collab/evidence/444/session-*.wav`（生产 `-debug` 自动存的 16k 录音）+ Gavin 口述原文。
//! 方法：生产同款 VAD 切语音段 → 每段 CAM++ 取 emb、对已注册声纹 `max_score_ready` 取分 → 1.7B 识别文字
//! → 与原文按二字组重合度自动标注（本人 / 他人 / 混合）→ 按时长分桶统计。另把本人长段切 1s 小窗补本人短样本。
//! 运行：`cargo test --bin feiyin-ime poc_voiceprint_short_444 -- --ignored --nocapture`

use super::speaker::{cosine, SpeakerVerifier, Voiceprint};
use super::vad::VadSegmenter;
use super::{
    create_qwen3_recognizer, decode_accuracy_allow_empty, max_new_tokens_for, ChineseScript,
};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::PathBuf;

const RATE: usize = 16000;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// (录音文件, Gavin 本人口述原文)。
const SESSIONS: &[(&str, &str)] = &[
    (
        "session-20260926-224100.wav",
        "最近有什么好看的电影？我比较喜欢看科幻片、恐怖片、惊悚片、动作片，还有喜剧片，特别是那种科幻惊悚片，我觉得特别好玩。然后去看看外面的世界，也可以一起出去旅游",
    ),
    (
        "session-20260926-224214.wav",
        "最近有什么好看的电影吗？我比较喜欢看科幻片、恐怖片、惊悚片。然后可以一起出去旅游，一起出去吃饭，然后去周边找找，到外面透透气。你可以约三五好友聚一聚",
    ),
    (
        "session-20260926-230039.wav",
        "最近有空吗？出来玩吗？一起出来看看电影。一起吃饭。一起逛逛商场，一起出去走走，都挺好啊。那怎。你觉得怎么样？这里逛逛，你觉得怎么样",
    ),
];

fn cjk(s: &str) -> Vec<char> {
    s.chars()
        .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
        .collect()
}

fn bigrams(cs: &[char]) -> Vec<(char, char)> {
    cs.windows(2).map(|w| (w[0], w[1])).collect()
}

/// 段文字的二字组落在原文二字组集合里的比例（段文字 <2 个汉字 ⇒ None，不标注）。
fn overlap(seg: &str, ref_set: &HashSet<(char, char)>) -> Option<f32> {
    let b = bigrams(&cjk(seg));
    if b.is_empty() {
        return None;
    }
    Some(b.iter().filter(|x| ref_set.contains(x)).count() as f32 / b.len() as f32)
}

fn label(ov: Option<f32>) -> &'static str {
    match ov {
        None => "无字",
        Some(r) if r >= 0.6 => "本人",
        Some(r) if r <= 0.2 => "他人",
        _ => "混合",
    }
}

fn bucket(secs: f32) -> &'static str {
    if secs < 1.0 {
        "<1.0s"
    } else if secs < 1.5 {
        "1.0-1.5s"
    } else {
        ">=1.5s"
    }
}

struct Row {
    file: String,
    kind: &'static str,
    start: f32,
    secs: f32,
    score: Option<f32>,
    label: &'static str,
    text: String,
}

#[test]
#[ignore = "POC-444：需真模型 + evidence 录音；--ignored 运行"]
fn poc_voiceprint_short_444() {
    let models = root().join("models");
    let verifier = SpeakerVerifier::load(&models).expect("CAM++ 声纹模型须在位");
    let vp = Voiceprint::load(&root().join("target/release/voiceprint.bin"));
    assert!(
        vp.has_any_ready(),
        "target/release/voiceprint.bin 须有已就绪声纹档"
    );
    let vad = VadSegmenter::try_new_for_local_trim(&models).expect("VAD 须在位");
    let acc = create_qwen3_recognizer(&models).expect("1.7B 须在位");
    let decode = |s: &[f32]| -> String {
        if s.len() < RATE / 5 {
            return String::new();
        }
        let cap = max_new_tokens_for(s.len() as f32 / RATE as f32);
        decode_accuracy_allow_empty(&acc, s, None, ChineseScript::Simplified, Some(cap))
            .map(|(t, _)| t)
            .unwrap_or_default()
    };
    // 等价复刻生产 （私有）：对所有已就绪语种档取最高余弦。
    const LANGS: &[&str] = &["zh", "en", "ko", "ja", "und"];
    let score = |s: &[f32]| -> Option<f32> {
        let e = verifier.embed(s)?;
        LANGS
            .iter()
            .filter_map(|l| vp.ready_centroid(l).map(|c| cosine(c, &e)))
            .fold(None, |b: Option<f32>, x| Some(b.map_or(x, |b| b.max(x))))
    };

    let dir = root().join("collab/evidence/444");
    let mut rows: Vec<Row> = Vec::new();
    for (file, ref_text) in SESSIONS {
        let path = dir.join(file);
        let Some(w) = sherpa_onnx::Wave::read(path.to_str().unwrap()) else {
            println!("[444] ⚠ 读不到 {file}，跳过");
            continue;
        };
        assert_eq!(w.sample_rate() as usize, RATE, "{file} 须为 16k");
        let audio = w.samples().to_vec();
        let ref_set: HashSet<(char, char)> = bigrams(&cjk(ref_text)).into_iter().collect();
        for (s, e) in vad.speech_ranges(&audio) {
            let seg = &audio[s..e.min(audio.len())];
            let secs = seg.len() as f32 / RATE as f32;
            if secs < 0.3 {
                continue;
            }
            let text = decode(seg);
            let lab = label(overlap(&text, &ref_set));
            rows.push(Row {
                file: file.to_string(),
                kind: "vad段",
                start: s as f32 / RATE as f32,
                secs,
                score: score(seg),
                label: lab,
                text: text.clone(),
            });
            // 本人长段切 1s 小窗（步进 1s）⇒ 补「本人短片段」样本，看短片判别力。
            if lab == "本人" && secs >= 2.0 {
                let mut p = 0usize;
                while p + RATE <= seg.len() {
                    rows.push(Row {
                        file: file.to_string(),
                        kind: "本人1s窗",
                        start: (s + p) as f32 / RATE as f32,
                        secs: 1.0,
                        score: score(&seg[p..p + RATE]),
                        label: "本人",
                        text: String::new(),
                    });
                    p += RATE;
                }
            }
        }
    }

    // ---- 明细表 ----
    let mut md = String::from("# POC-VOICEPRINT-SHORT-444 · 声纹短片段判别力（离线）\n\n");
    let _ = writeln!(
        md,
        "声纹档 `target/release/voiceprint.bin`；剔除门槛 DROP_THR = {}；生产判定最短 {}s。\n",
        super::speaker::DROP_THR,
        super::speaker::MIN_JUDGE_SECS
    );
    md.push_str("| 录音 | 类型 | 起点 s | 时长 s | 得分 | 标注 | 识别文字 |\n| --- | --- | ---: | ---: | ---: | --- | --- |\n");
    for r in &rows {
        let _ = writeln!(
            md,
            "| {} | {} | {:.2} | {:.2} | {} | {} | {} |",
            &r.file[8..23],
            r.kind,
            r.start,
            r.secs,
            r.score.map(|v| format!("{v:.3}")).unwrap_or("-".into()),
            r.label,
            r.text.replace('|', "/")
        );
    }

    // ---- 分桶统计（本人 vs 他人）----
    md.push_str("\n## 分桶统计（得分：最小 / 中位 / 最大；<0.45 会被剔除）\n\n| 标注 | 时长桶 | 样本数 | 最小 | 中位 | 最大 | <0.45 条数 |\n| --- | --- | ---: | ---: | ---: | ---: | ---: |\n");
    for lab in ["本人", "他人", "混合"] {
        for b in ["<1.0s", "1.0-1.5s", ">=1.5s"] {
            let mut v: Vec<f32> = rows
                .iter()
                .filter(|r| r.label == lab && bucket(r.secs) == b)
                .filter_map(|r| r.score)
                .collect();
            if v.is_empty() {
                continue;
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let below = v.iter().filter(|x| **x < super::speaker::DROP_THR).count();
            let _ = writeln!(
                md,
                "| {lab} | {b} | {} | {:.3} | {:.3} | {:.3} | {below} |",
                v.len(),
                v[0],
                v[v.len() / 2],
                v[v.len() - 1]
            );
        }
    }
    let out = dir.join("table.md");
    std::fs::write(&out, &md).expect("写 table.md");
    println!("{md}");
    println!("[444] 写入 {}", out.display());
}
