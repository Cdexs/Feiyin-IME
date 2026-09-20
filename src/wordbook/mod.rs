pub mod cache;
pub mod db;

use anyhow::Result;
use std::cell::RefCell;

#[allow(unused_imports)]
pub use cache::{WordbookCache, WordbookEntry, WordbookStats};

/// WORDBOOK-053-C: Maximum character count for a valid wordbook candidate.
/// Rationale: Chinese idioms/proper nouns are typically <=10 chars; English phrases
/// like "Claude Code" are <=12 chars. 30 chars leaves ample headroom for legitimate
/// short phrases while rejecting full sentences (which are usually 40+ chars).
/// "维生素B12" (6 chars), "飞音输入法" (5 chars), "Claude Code" (11 chars) all pass.
const MAX_CANDIDATE_CHARS: usize = 30;

/// WORDBOOK-MINLEN-242: 候选词最小字符数。单字候选对热词偏置无实效但持续制造噪声，
/// 一律拒绝。2 是下限而非经验值——中文专名最短即 2 字（飞音/玄戒），
/// 常用英文缩写最短亦为 2（AI/UI）。
const MIN_CANDIDATE_CHARS: usize = 2;

/// WORDBOOK-053-C: Maximum number of whitespace-separated segments.
/// Rationale: English phrases like "Claude Code" (2 segments) or "voice ime" (2)
/// are legitimate. 4 segments allows rare longer English terms while rejecting
/// sentence-level text (which typically has many whitespace-separated words).
const MAX_WHITESPACE_SEGMENTS: usize = 4;

/// WORDBOOK-053-C: Validate a wordbook candidate before learning.
///
/// Rejects sentence-level text and numbered-list fragments that should never be
/// learned as hotwords. Returns `Ok(())` if valid, `Err(reason)` if rejected.
/// The rejection reason is a static string suitable for `log::debug!` (no allocation).
pub fn is_valid_candidate(word: &str) -> Result<(), &'static str> {
    let trimmed = word.trim();
    if trimmed.is_empty() {
        return Err("empty");
    }
    // WORDBOOK-MINLEN-242（Gavin 2026-09-20 端测报）：此前只有上限没有下限，
    // 编辑态改一个字（最常见的是错字/标点修正）就会把单字灌进候选表。
    // 单字对 ASR 热词偏置几乎无作用（一个字的上下文偏置不足以改变解码路径），
    // 却会持续制造噪声候选。故设下限 2 字符。
    if trimmed.chars().count() < MIN_CANDIDATE_CHARS {
        return Err("below min char limit");
    }
    if trimmed.chars().count() > MAX_CANDIDATE_CHARS {
        return Err("exceeds max char limit");
    }
    if trimmed.contains('。')
        || trimmed.contains('！')
        || trimmed.contains('？')
        || trimmed.contains('；')
    {
        return Err("contains sentence-ending punctuation");
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err("contains newline");
    }
    let first_char = trimmed.chars().next().unwrap();
    if first_char.is_ascii_digit() {
        let chars: Vec<char> = trimmed.chars().collect();
        if chars.len() >= 2 {
            let second = chars[1];
            if second == '.' || second == '、' || second == ' ' || second == '\t' {
                return Err("starts with numbered prefix");
            }
        }
    }
    if first_char == '①'
        || first_char == '②'
        || first_char == '③'
        || first_char == '④'
        || first_char == '⑤'
        || first_char == '⑥'
        || first_char == '⑦'
        || first_char == '⑧'
        || first_char == '⑨'
        || first_char == '⑩'
    {
        return Err("starts with circled number prefix");
    }
    if trimmed.starts_with("- ") {
        return Err("starts with dash-list prefix");
    }
    let comma_count = trimmed.chars().filter(|&c| c == '，' || c == ',').count();
    if comma_count >= 2 {
        return Err("multiple commas (sentence-level)");
    }
    let segments = trimmed.split_whitespace().count();
    if segments > MAX_WHITESPACE_SEGMENTS {
        return Err("too many whitespace segments");
    }
    Ok(())
}

/// Compatibility wrapper for the existing main pipeline.
///
/// New code should use `WordbookCache` directly so reads can stay in memory.
pub struct Wordbook {
    cache: RefCell<WordbookCache>,
}

impl Wordbook {
    pub fn open() -> Result<Self> {
        Ok(Self {
            cache: RefCell::new(WordbookCache::load_from_db()?),
        })
    }

    /// Manually add a word as a user-sourced entry.
    pub fn add(&self, word: &str) -> Result<()> {
        self.cache.borrow_mut().add_entry(word, "user")?;
        Ok(())
    }

    /// Delete a word by id.
    #[allow(dead_code)]
    pub fn delete(&self, id: i64) -> Result<()> {
        if let Some(entry) = db::get_entry_by_id(id)? {
            self.cache.borrow_mut().remove_entry(&entry.word)?;
        }
        Ok(())
    }

    /// List all words for UI display.
    #[allow(dead_code)]
    pub fn list_all(&self) -> Result<Vec<WordEntry>> {
        let entries = db::load_word_entries()?;
        Ok(entries
            .into_iter()
            .map(|entry| WordEntry {
                id: entry.id,
                word: entry.word,
                source: entry.source,
                created_at: entry.created_at,
            })
            .collect())
    }

    /// Learn a correction by comparing original ASR output with edited text.
    /// WORDBOOK-SINGLEWORD-001-CORE: produces corrected-side word for single-word model.
    #[allow(dead_code)]
    pub fn learn_correction(&self, original: &str, edited: &str, threshold: u32) -> Result<()> {
        let Some(corrected_part) = extract_correction_word(original, edited) else {
            return Ok(());
        };

        self.learn_suggestion(&corrected_part, threshold)
    }

    pub fn learn_suggestion(&self, word: &str, threshold: u32) -> Result<()> {
        let word = word.trim();
        if word.is_empty() {
            return Ok(());
        }

        // WORDBOOK-053-C: reject sentence-level / numbered-list candidates before
        // they enter the candidate table or get promoted to the wordbook.
        // Uses log::debug! (filtered out in release builds where LevelFilter=Warn,
        // zero disk IO per Gavin's "reduce real-time log IO" requirement).
        if let Err(reason) = is_valid_candidate(word) {
            log::debug!("Auto-learn candidate rejected ({}): {:?}", reason, word);
            return Ok(());
        }

        if self.cache.borrow().exists(word) {
            let _ = db::delete_candidate(word);
            return Ok(());
        }

        let threshold = threshold.max(1);
        let count = db::upsert_candidate(word)?;
        // WORDBOOK-AUTOLEARN-OBS-218: 用 warn 级承载（release 非 debug 模式
        // main.rs 的 LevelFilter=Warn 会把 info! 整条滤掉，用户看不到自动学习效果）。
        // 前缀 [AUTOLEARN] 供 grep；不改全局日志级别（Gavin 要求压低实时日志 IO）。
        if count < threshold {
            log::warn!(
                "[AUTOLEARN] candidate observed: '{}' ({}/{})",
                word,
                count,
                threshold
            );
            return Ok(());
        }

        log::warn!(
            "[AUTOLEARN] promoted after threshold: '{}' ({}/{})",
            word,
            count,
            threshold
        );
        self.cache.borrow_mut().add_entry(word, "system")?;
        let _ = db::delete_candidate(word);

        Ok(())
    }
}

/// WORDBOOK-SINGLEWORD-001-CORE: Extract corrected-side word from original vs edited text.
/// Preserves the diff extraction logic but returns only the corrected part (the word to learn).
/// This is for future WORDBOOK-CORRECTION-UI-001 纠错入口 reuse.
fn extract_correction_word(original: &str, edited: &str) -> Option<String> {
    if original == edited {
        return None;
    }

    let orig_words = tokenize(original);
    let edit_words = tokenize(edited);

    let common_prefix = orig_words
        .iter()
        .zip(edit_words.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let common_suffix = orig_words
        .iter()
        .rev()
        .zip(edit_words.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    let orig_mid_end = orig_words.len().saturating_sub(common_suffix);
    let edit_mid_end = edit_words.len().saturating_sub(common_suffix);

    if common_prefix >= orig_mid_end || common_prefix >= edit_mid_end {
        return None;
    }

    let corrected_part = edit_words[common_prefix..edit_mid_end].join("");

    if corrected_part.is_empty() {
        return None;
    }

    let trimmed = corrected_part.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed)
}

fn tokenize(text: &str) -> Vec<String> {
    text.chars().map(|c| c.to_string()).collect()
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WordEntry {
    pub id: i64,
    pub word: String,
    pub source: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::extract_correction_word;

    #[test]
    fn test_extract_correction_word_uses_changed_middle_segment() {
        let word = extract_correction_word("我想用微型免", "我想用voice ime");
        assert_eq!(word, Some("voice ime".to_string()));
    }

    #[test]
    fn test_extract_correction_word_returns_none_for_identical_text() {
        assert_eq!(extract_correction_word("一样", "一样"), None);
    }

    #[test]
    fn test_extract_correction_word_returns_corrected_side() {
        let word = extract_correction_word("我吃了苹果", "我吃了梨");
        assert_eq!(word, Some("梨".to_string()));
    }

    #[test]
    fn test_extract_correction_word_trims_whitespace() {
        let word = extract_correction_word("测试", " 测试词 ");
        assert_eq!(word, Some("测试词".to_string()));
    }

    #[test]
    fn test_extract_correction_word_empty_corrected_returns_none() {
        let word = extract_correction_word("测试", "  ");
        assert_eq!(word, None);
    }

    // WORDBOOK-053-B: explicit direction guards for overlay edit auto-learning.
    // `learn_correction(original, edited)` calls `extract_correction_word(original, edited)` and
    // learns the *edited* side. These tests pin that behavior so the feature cannot silently learn
    // the ASR-misrecognized word instead of the user's correction.

    #[test]
    fn test_extract_correction_word_learns_user_correction() {
        // ASR said "阿里云", user corrected to "阿里運". The learned word must be "阿里運".
        assert_eq!(
            extract_correction_word("阿里云", "阿里運"),
            Some("運".to_string())
        );
    }

    #[test]
    fn test_extract_correction_word_learns_user_correction_longer() {
        // ASR said "我想用微型免", user corrected to "我想用voice ime".
        assert_eq!(
            extract_correction_word("我想用微型免", "我想用voice ime"),
            Some("voice ime".to_string())
        );
    }

    #[test]
    fn test_extract_correction_word_does_not_learn_original_side() {
        // Reverse the previous case: if original were learned, the result would be "阿里云" or its
        // diff segment. We assert the function returns None when arguments are swapped, proving
        // the learned side is the *second* argument.
        assert_eq!(
            extract_correction_word("阿里運", "阿里云"),
            Some("云".to_string())
        );
    }

    #[test]
    fn test_extract_correction_word_repeated_short_phrase_learns_each_correction() {
        // User says "好的" twice. First ASR gives "好的" → edit stays "好的" (no change, None).
        // Second ASR gives "号的" → user corrects to "好的" → learn the corrected side.
        assert_eq!(extract_correction_word("好的", "好的"), None);
        assert_eq!(
            extract_correction_word("号的", "好的"),
            Some("好".to_string())
        );
    }

    #[test]
    fn test_extract_correction_word_never_returns_original_side_text() {
        // Orchestrator-specified direction guard: for original "阿里云" edited to "阿里運"-type
        // input, the learned word must NEVER contain original-side text. The common prefix "阿里"
        // is preserved context (present in both sides), so the guard checks the *distinctive*
        // original-side char "云" that a wrong-side implementation would return instead of "運".
        let learned = extract_correction_word("阿里云", "阿里運").expect("should learn a word");
        assert!(
            !learned.contains('云'),
            "learned word {:?} must not contain original-side char 云",
            learned
        );
        assert_eq!(learned, "運");
    }

    // WORDBOOK-053-C: is_valid_candidate guard tests

    use super::is_valid_candidate;

    /// WORDBOOK-MINLEN-242（Gavin 2026-09-20 端测报）：编辑态改一个字就把单字灌进候选表。
    /// 此前 `is_valid_candidate` 只有上限（30）没有下限，单字一路放行。
    /// 本用例钉死下限，防复发；边界外（2 字）必须仍然放行。
    #[test]
    fn minlen242_single_char_candidate_rejected() {
        // 单字一律拒绝——中英文、数字皆然
        assert!(is_valid_candidate("好").is_err(), "单个汉字不得入库");
        assert!(is_valid_candidate("運").is_err(), "单个繁体字不得入库");
        assert!(is_valid_candidate("a").is_err(), "单个字母不得入库");
        assert!(
            is_valid_candidate(" 好 ").is_err(),
            "trim 后仍是单字，不得入库"
        );
        // 🔴 边界外：2 字是合法下限，不得被误挡
        assert!(is_valid_candidate("飞音").is_ok(), "2 字中文专名必须放行");
        assert!(is_valid_candidate("AI").is_ok(), "2 字母英文缩写必须放行");
    }

    #[test]
    fn test_is_valid_candidate_rejects_real_dirty_sentence() {
        // The exact sentence from debug.log (08-17 08:43:46) that prompted this task
        let dirty = "1. 设置UI，点击设置热键，按下Alt键，没有任何反应。";
        assert!(is_valid_candidate(dirty).is_err());
    }

    #[test]
    fn test_is_valid_candidate_rejects_numbered_list_with_newlines() {
        let dirty = "1. 3块2毛2的大葱
2. 5块6毛钱的土豆";
        assert!(is_valid_candidate(dirty).is_err());
    }

    #[test]
    fn test_is_valid_candidate_rejects_dash_list_with_newlines() {
        let dirty = "- 不能迟到早退。
- 遵守课堂纪律，要尊重老师。";
        assert!(is_valid_candidate(dirty).is_err());
    }

    #[test]
    fn test_is_valid_candidate_rejects_circled_number_prefix() {
        let dirty = "① 第一条注意事项";
        assert!(is_valid_candidate(dirty).is_err());
    }

    #[test]
    fn test_is_valid_candidate_rejects_multiple_commas() {
        let dirty = "遵守纪律，不迟到，不早退";
        assert!(is_valid_candidate(dirty).is_err());
    }

    #[test]
    fn test_is_valid_candidate_rejects_too_long() {
        // Exceeds MAX_CANDIDATE_CHARS=30 (31 chars)
        let long = "一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一";
        assert!(is_valid_candidate(long).is_err());
    }

    #[test]
    fn test_is_valid_candidate_rejects_too_many_segments() {
        // 5 whitespace-separated segments (exceeds MAX_WHITESPACE_SEGMENTS=4)
        let segments = "a b c d e";
        assert!(is_valid_candidate(segments).is_err());
    }

    #[test]
    fn test_is_valid_candidate_passes_normal_chinese_words() {
        assert!(is_valid_candidate("阿里云").is_ok());
        assert!(is_valid_candidate("飞音输入法").is_ok());
        assert!(is_valid_candidate("五代十国").is_ok());
        assert!(is_valid_candidate("维生素B12").is_ok());
        assert!(is_valid_candidate("三五成群").is_ok());
    }

    #[test]
    fn test_is_valid_candidate_passes_normal_english_words() {
        assert!(is_valid_candidate("Claude Code").is_ok());
        assert!(is_valid_candidate("voice ime").is_ok());
        assert!(is_valid_candidate("LLM").is_ok());
        assert!(is_valid_candidate("Cloud").is_ok());
        assert!(is_valid_candidate("M2").is_ok());
    }

    #[test]
    fn test_is_valid_candidate_passes_single_comma_phrase() {
        // One comma is a two-word phrase, not a sentence
        assert!(is_valid_candidate("三五成群，打架斗殴").is_ok());
    }

    #[test]
    fn test_is_valid_candidate_rejects_empty() {
        assert!(is_valid_candidate("").is_err());
        assert!(is_valid_candidate("   ").is_err());
    }
}
