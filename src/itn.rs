//! ITN-SMART-001 智能数字规整模块（DEC-030）
//!
//! 自研中文数字→阿拉伯数字转换：多位数字/计量语境转数字，单字数字无单位保留汉字。
//! 算法与规则数据分离：规则在 itn-rules.toml（include_str! 内置默认，运行时可覆盖）。
//!
//! 核心流程：扫描文本 → 识别中文数字段 → 判定语境（单位/日期/序数/百分比/分数/小数）→
//!   决定是否转换 → 保护白名单优先 → 输出转换结果。

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::Deserialize;

/// 内置默认规则文件
const BUILTIN_RULES: &str = include_str!("../itn-rules.toml");

// ============================================================
// 规则数据结构
// ============================================================

#[derive(Debug, Clone, Deserialize, Default)]
struct Rules {
    #[serde(default)]
    switches: Switches,
    #[serde(default)]
    units: Units,
    #[serde(default)]
    date_time: DateTime,
    #[serde(default)]
    ordinal: Ordinal,
    #[serde(default)]
    percentage: Percentage,
    #[serde(default)]
    fraction: Fraction,
    #[serde(default)]
    protect: Protect,
}

#[derive(Debug, Clone, Deserialize)]
struct Switches {
    #[serde(default)]
    convert_single_digit_with_classifier: bool,
    #[serde(default = "default_below_zero_style")]
    below_zero_style: String,
    #[serde(default = "default_large_amount_keep")]
    large_amount_keep_wan_yi: bool,
}

impl Default for Switches {
    fn default() -> Self {
        Self {
            convert_single_digit_with_classifier: false,
            below_zero_style: "text".to_string(),
            large_amount_keep_wan_yi: true,
        }
    }
}

fn default_below_zero_style() -> String {
    "text".to_string()
}

fn default_large_amount_keep() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Units {
    #[serde(default)]
    currency: UnitList,
    #[serde(default)]
    length: UnitList,
    #[serde(default)]
    weight: UnitList,
    #[serde(default)]
    volume: UnitList,
    #[serde(default)]
    temperature: UnitList,
    #[serde(default)]
    pressure: UnitList,
    #[serde(default)]
    electrical: UnitList,
    #[serde(default)]
    frequency: UnitList,
    #[serde(default)]
    acoustic: UnitList,
    #[serde(default)]
    data: UnitList,
    #[serde(default)]
    other: UnitList,
    #[serde(default)]
    geo_prefix: UnitList,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct UnitList {
    #[serde(default)]
    words: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct DateTime {
    #[serde(default)]
    triggers: DateTimeTriggers,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct DateTimeTriggers {
    #[serde(default)]
    suffix: Vec<String>,
    #[serde(default)]
    special: Vec<String>,
    #[serde(default)]
    prefix: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Ordinal {
    #[serde(default)]
    prefix: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Percentage {
    #[serde(default)]
    prefix: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Fraction {
    #[serde(default)]
    pattern: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Protect {
    #[serde(default)]
    idioms: ProtectList,
    #[serde(default)]
    proper_nouns: ProtectList,
    #[serde(default)]
    function_words: ProtectList,
    #[serde(default)]
    classifiers: ProtectList,
    /// ITN-SMART-002: 含数字的历史/文化/民俗词汇（"五代十国""三皇五帝"等）。
    /// 与 proper_nouns 功能相同（整词保护不转），独立分组便于维护与注释。
    #[serde(default)]
    historical: ProtectList,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ProtectList {
    #[serde(default)]
    words: Vec<String>,
}

// ============================================================
// 编译后规则（HashSet 加速查询）
// ============================================================

#[derive(Debug, Clone)]
struct CompiledRules {
    all_units: HashSet<String>,
    geo_prefixes: HashSet<String>,
    date_suffixes: HashSet<String>,
    date_specials: HashSet<String>,
    date_prefixes: HashSet<String>,
    ordinal_prefix: String,
    percentage_prefix: String,
    fraction_pattern: String,
    idiom_set: HashSet<String>,
    proper_noun_set: HashSet<String>,
    function_word_set: HashSet<String>,
    classifier_set: HashSet<String>,
    convert_single_digit_with_classifier: bool,
    below_zero_style: String,
    large_amount_keep_wan_yi: bool,
    /// ITN-SMART-002: 含数字的历史/文化/民俗词汇保护集
    historical_set: HashSet<String>,
}

impl CompiledRules {
    fn from_rules(r: Rules) -> Self {
        let mut all_units = HashSet::new();
        for unit_list in [
            &r.units.currency,
            &r.units.length,
            &r.units.weight,
            &r.units.volume,
            &r.units.temperature,
            &r.units.pressure,
            &r.units.electrical,
            &r.units.frequency,
            &r.units.acoustic,
            &r.units.data,
            &r.units.other,
        ] {
            for w in &unit_list.words {
                all_units.insert(w.clone());
            }
        }

        let geo_prefixes: HashSet<String> = r.units.geo_prefix.words.iter().cloned().collect();
        let date_suffixes: HashSet<String> = r.date_time.triggers.suffix.iter().cloned().collect();
        let date_specials: HashSet<String> = r.date_time.triggers.special.iter().cloned().collect();
        let date_prefixes: HashSet<String> = r.date_time.triggers.prefix.iter().cloned().collect();

        Self {
            all_units,
            geo_prefixes,
            date_suffixes,
            date_specials,
            date_prefixes,
            ordinal_prefix: r.ordinal.prefix,
            percentage_prefix: r.percentage.prefix,
            fraction_pattern: r.fraction.pattern,
            idiom_set: r.protect.idioms.words.iter().cloned().collect(),
            proper_noun_set: r.protect.proper_nouns.words.iter().cloned().collect(),
            function_word_set: r.protect.function_words.words.iter().cloned().collect(),
            classifier_set: r.protect.classifiers.words.iter().cloned().collect(),
            convert_single_digit_with_classifier: r.switches.convert_single_digit_with_classifier,
            below_zero_style: r.switches.below_zero_style,
            large_amount_keep_wan_yi: r.switches.large_amount_keep_wan_yi,
            historical_set: r.protect.historical.words.iter().cloned().collect(),
        }
    }

    fn is_unit(&self, s: &str) -> bool {
        self.all_units.iter().any(|u| s.starts_with(u))
    }

    fn is_date_suffix(&self, s: &str) -> bool {
        self.date_suffixes.iter().any(|u| s.starts_with(u))
    }

    fn is_geo_prefix(&self, s: &str) -> bool {
        self.geo_prefixes.iter().any(|u| s.starts_with(u))
    }

    /// 查找最长匹配的单位词长度
    fn match_unit_len(&self, s: &str) -> Option<usize> {
        let mut best: Option<usize> = None;
        for u in &self.all_units {
            if s.starts_with(u.as_str()) {
                let len = u.chars().count();
                if best.is_none() || len > best.unwrap() {
                    best = Some(len);
                }
            }
        }
        best
    }

    /// TEMP-CELSIUS-001: 查找最长匹配的单位词，返回 (字符长度, 单位词原文)。
    /// 供调用方判定是否含"摄氏"关键词触发 ℃ 符号替换。
    fn match_unit_word<'a>(&self, chars: &'a [char], pos: usize) -> Option<(usize, String)> {
        if pos >= chars.len() {
            return None;
        }
        let rest: String = chars[pos..].iter().collect();
        let len = self.match_unit_len(&rest)?;
        let word: String = chars[pos..pos + len].iter().collect();
        Some((len, word))
    }

    /// 查找最长匹配的日期后缀长度
    fn match_date_suffix_len(&self, s: &str) -> Option<usize> {
        let mut best: Option<usize> = None;
        for u in &self.date_suffixes {
            if s.starts_with(u.as_str()) {
                let len = u.chars().count();
                if best.is_none() || len > best.unwrap() {
                    best = Some(len);
                }
            }
        }
        best
    }

    /// 检查字符串末尾是否为已知单位（用于"五块八"类小数模式）
    fn is_unit_preceded(&self, before: &str) -> bool {
        self.all_units.iter().any(|u| before.ends_with(u.as_str()))
    }
}

// ============================================================
// 规则加载（OnceLock 缓存）
// ============================================================

static RULES: OnceLock<CompiledRules> = OnceLock::new();

fn load_rules() -> CompiledRules {
    // 尝试从 exe 同级目录加载
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("itn-rules.toml");
            if let Ok(content) = std::fs::read_to_string(&path) {
                match toml::from_str::<Rules>(&content) {
                    Ok(r) => {
                        log::info!("ITN rules loaded from {:?}", path);
                        return CompiledRules::from_rules(r);
                    }
                    Err(e) => {
                        log::warn!(
                            "ITN rules parse error in {:?}, falling back to builtin: {}",
                            path,
                            e
                        );
                    }
                }
            }
        }
    }

    // 内置默认
    match toml::from_str::<Rules>(BUILTIN_RULES) {
        Ok(r) => CompiledRules::from_rules(r),
        Err(e) => {
            log::warn!("ITN builtin rules parse error: {}", e);
            CompiledRules::from_rules(Rules::default())
        }
    }
}

fn rules() -> &'static CompiledRules {
    RULES.get_or_init(load_rules)
}

/// 重置规则缓存（仅测试用）
#[cfg(test)]
fn reset_rules_for_test() {
    // OnceLock 不支持 reset，测试用独立函数
}

/// 从指定内容加载规则（测试用）
#[cfg(test)]
fn compile_rules_from_content(content: &str) -> CompiledRules {
    match toml::from_str::<Rules>(content) {
        Ok(r) => CompiledRules::from_rules(r),
        Err(_) => CompiledRules::from_rules(Rules::default()),
    }
}

// ============================================================
// 中文数字解析
// ============================================================

const DIGIT_MAP: &[(&str, char)] = &[
    ("零", '0'),
    ("〇", '0'),
    ("一", '1'),
    ("幺", '1'),
    ("二", '2'),
    ("两", '2'),
    ("三", '3'),
    ("四", '4'),
    ("五", '5'),
    ("六", '6'),
    ("七", '7'),
    ("八", '8'),
    ("九", '9'),
];

const UNIT_MAP: &[(&str, u64)] = &[
    ("十", 10),
    ("百", 100),
    ("千", 1000),
    ("万", 10000),
    ("亿", 100000000),
];

fn is_digit_char(ch: char) -> bool {
    ch.is_ascii_digit()
}

fn is_chinese_digit(s: &str) -> bool {
    DIGIT_MAP.iter().any(|(cn, _)| s.starts_with(*cn))
}

/// 尝试将单个中文字符映射为数字（0-9）
fn chinese_digit_char(ch: char) -> Option<char> {
    let s = ch.to_string();
    for (cn, d) in DIGIT_MAP {
        if s == *cn {
            return Some(*d);
        }
    }
    None
}

/// 判定字符是否为中文数字字符（含单位 十/百/千/万/亿）
fn is_cn_num_char(ch: char) -> bool {
    chinese_digit_char(ch).is_some() || is_cn_unit_char(ch)
}

fn is_cn_unit_char(ch: char) -> bool {
    let s = ch.to_string();
    UNIT_MAP.iter().any(|(cn, _)| s == *cn)
}

/// 解析中文数字字符串为数值 + 消耗的字符数
/// 支持进位组合（三百二十五 = 325）、小数点（三点一四 = 3.14）、逐位串（幺三八 = 138）
/// rules = Some(r) 时启用 large_amount_keep_wan_yi 检查：万/亿后紧跟单位时不消费万/亿
/// 返回 (数字字符串, 消费字符数)
fn parse_cn_number(
    chars: &[char],
    start: usize,
    rules: Option<&CompiledRules>,
) -> Option<(String, usize)> {
    if start >= chars.len() {
        return None;
    }

    // 先检测逐位串模式：连续≥3个纯数字字符（零/〇/一/二/三/四/五/六/七/八/九/幺/两）
    // 且后面不跟进位单位（十百千万亿）
    let mut serial_len = 0usize;
    for k in start..chars.len() {
        if chinese_digit_char(chars[k]).is_some() {
            serial_len += 1;
        } else {
            break;
        }
    }
    // 检查逐位串后面是否跟进位单位
    let serial_end = start + serial_len;
    let next_is_unit = serial_end < chars.len() && is_cn_unit_char(chars[serial_end]);

    if serial_len >= 2 && !next_is_unit {
        let digits: Vec<char> = (start..serial_end)
            .map(|k| chinese_digit_char(chars[k]).unwrap())
            .collect();
        return Some((digits.into_iter().collect(), serial_len));
    }

    // 进位组合模式解析
    // 算法：
    //   digit  — 上一个看到的中文数字值（无单位时暂存）
    //   section — 当前千/百/十以内的段值
    //   result  — 已通过 万/亿 结算的累计值
    //   last_big — 记录最近的大单位（万/亿），用于末尾单字的隐式单位处理
    let mut idx = start;
    let mut result: u64 = 0;
    let mut section: u64 = 0;
    let mut digit: u64 = 0;
    let mut has_digit = false;
    // false=刚处理完大单位后没看到零，true=看到零后大单位隐式失效
    let mut zero_since_big = false;
    // 最近看到的大单位
    let mut big_unit_seen = false;

    while idx < chars.len() {
        let ch = chars[idx];
        if let Some(d) = chinese_digit_char(ch) {
            digit = d.to_digit(10).unwrap() as u64;
            has_digit = true;
            idx += 1;
            continue;
        }
        match ch {
            '十' | '拾' => {
                section += (if has_digit { digit } else { 1 }) * 10;
                digit = 0;
                has_digit = false;
            }
            '百' | '佰' => {
                section += digit * 100;
                digit = 0;
                has_digit = false;
            }
            '千' | '仟' => {
                section += digit * 1000;
                digit = 0;
                has_digit = false;
            }
            '万' | '萬' => {
                // large_amount_keep_wan_yi：万/亿后紧跟单位时不消费万/亿
                if let Some(rules) = rules {
                    if rules.large_amount_keep_wan_yi && idx + 1 < chars.len() {
                        let after_big: String = chars[idx + 1..].iter().collect();
                        if rules.is_unit(&after_big) {
                            break;
                        }
                    }
                }
                section += digit;
                result = (result + section) * 10000;
                section = 0;
                digit = 0;
                has_digit = false;
                big_unit_seen = true;
                zero_since_big = false;
            }
            '亿' | '億' => {
                if let Some(rules) = rules {
                    if rules.large_amount_keep_wan_yi && idx + 1 < chars.len() {
                        let after_big: String = chars[idx + 1..].iter().collect();
                        if rules.is_unit(&after_big) {
                            break;
                        }
                    }
                }
                section += digit;
                result = (result + section) * 100000000;
                section = 0;
                digit = 0;
                has_digit = false;
                big_unit_seen = true;
                zero_since_big = false;
            }
            '零' | '〇' => {
                digit = 0;
                has_digit = false;
                zero_since_big = true;
            }
            '点' | '．' => {
                // 时间语境检测：点后数字含进位单位（如"五十"含十） 或 点后跟分/秒/半/刻
                if let Some(rules) = rules {
                    let after_point = idx + 1;
                    if let Some((_, num_consumed)) =
                        parse_cn_number(chars, after_point, Some(rules))
                    {
                        let after_num = after_point + num_consumed;
                        let trailing: String = chars[after_num..].iter().collect();
                        if rules.is_date_suffix(&trailing)
                            || rules
                                .date_specials
                                .iter()
                                .any(|s| trailing.starts_with(s.as_str()))
                        {
                            break;
                        }
                        let consumed_text: String = chars[after_point..after_num].iter().collect();
                        if consumed_text.contains('十')
                            || consumed_text.contains('百')
                            || consumed_text.contains('千')
                        {
                            break;
                        }
                    } else {
                        let trailing: String = chars[after_point..].iter().collect();
                        if rules
                            .date_specials
                            .iter()
                            .any(|s| trailing.starts_with(s.as_str()))
                        {
                            break;
                        }
                    }
                }
                // 点后无数字 → 不是小数点（句尾或时间单位）
                if idx + 1 >= chars.len() || chinese_digit_char(chars[idx + 1]).is_none() {
                    break;
                }
                section += digit;
                result += section;
                idx += 1;
                let mut dec_digits: Vec<char> = Vec::new();
                while idx < chars.len() {
                    if let Some(d) = chinese_digit_char(chars[idx]) {
                        dec_digits.push(d);
                        idx += 1;
                    } else {
                        break;
                    }
                }
                let int_str = result.to_string();
                if dec_digits.is_empty() {
                    return Some((int_str, idx - start));
                }
                let dec_str: String = dec_digits.iter().collect();
                return Some((format!("{}.{}", int_str, dec_str), idx - start));
            }
            _ => break,
        }
        idx += 1;
    }

    // 循环结束：处理末尾挂起的数字
    if has_digit {
        if big_unit_seen && !zero_since_big {
            // 大单位后单字数字 → 隐式单位
            // "两万五" = 25000（五→五千）
            // "三亿五" = 350000000（五→五千万）
            section += digit * 1000;
        } else {
            section += digit;
        }
        has_digit = false;
    }

    result += section;

    // 检查是否解析到了有效数字
    if result == 0 {
        let zero_count = (start..idx)
            .filter(|&k| chars[k] == '零' || chars[k] == '〇')
            .count();
        if zero_count > 0 {
            let zeros: String = (0..zero_count).map(|_| '0').collect();
            return Some((zeros, idx - start));
        }
        return None;
    }

    Some((result.to_string(), idx - start))
}

/// 解析"零下X"或"负X"前缀
fn parse_negative_prefix(chars: &[char], start: usize) -> Option<(bool, usize)> {
    // "零下" → negative
    if start + 1 < chars.len() && chars[start] == '零' && chars[start + 1] == '下' {
        return Some((true, 2));
    }
    // "负" → negative
    if start < chars.len() && chars[start] == '负' {
        return Some((true, 1));
    }
    None
}

// ============================================================
// 主转换函数
// ============================================================

/// 智能数字规整：将中文数字转为阿拉伯数字，保护单字/成语/专有名词。
///
/// 位置：转录后/LLM 前，三模型统一生效。
/// 幂等：已是阿拉伯数字的输入逐字节不变。
pub fn normalize_numbers(text: &str) -> String {
    let r = rules();
    normalize_with_rules(text, r)
}

fn normalize_with_rules(text: &str, r: &CompiledRules) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        // 检查保护白名单（最长匹配优先）
        if let Some(skip) = check_protection(&chars, i, r) {
            // 输出受保护的原文
            for ch in &chars[i..i + skip] {
                result.push(*ch);
            }
            i += skip;
            continue;
        }

        // 检查百分比 "百分之X"
        if !r.percentage_prefix.is_empty() {
            let prefix_chars: Vec<char> = r.percentage_prefix.chars().collect();
            if chars_match(&chars, i, &prefix_chars) {
                let after = i + prefix_chars.len();
                if let Some((num_str, consumed)) = parse_cn_number(&chars, after, Some(r)) {
                    if consumed > 0 {
                        result.push_str(&num_str);
                        result.push('%');
                        i = after + consumed;
                        continue;
                    }
                }
            }
        }

        // 检查分数 "X分之Y"
        if !r.fraction_pattern.is_empty() {
            if let Some(frac_info) = try_parse_fraction(&chars, i, r) {
                let (numerator, denominator, total_consumed) = frac_info;
                result.push_str(&numerator);
                result.push('/');
                result.push_str(&denominator);
                i += total_consumed;
                continue;
            }
        }

        // 检查序数 "第X..."
        if !r.ordinal_prefix.is_empty() && chars[i] == r.ordinal_prefix.chars().next().unwrap() {
            // "第" 后面跟数字
            let after = i + 1;
            if let Some((num_str, consumed)) = parse_cn_number(&chars, after, Some(r)) {
                if consumed > 0 {
                    result.push(r.ordinal_prefix.chars().next().unwrap());
                    result.push_str(&num_str);
                    i = after + consumed;
                    continue;
                }
            }
        }

        // 检查经纬度前缀 "东经X度"
        if let Some(prefix_len) = match_geo_prefix(&chars, i, r) {
            // 前缀后跟数字+度
            let after = i + prefix_len;
            if let Some((num_str, consumed)) = parse_cn_number(&chars, after, Some(r)) {
                if consumed > 0 {
                    // 输出前缀
                    for ch in &chars[i..after] {
                        result.push(*ch);
                    }
                    result.push_str(&num_str);
                    i = after + consumed;
                    continue;
                }
            }
        }

        // 检查日期时间前缀 "上午X点" "下午X点"
        if let Some(prefix_len) = match_date_prefix(&chars, i, r) {
            let after = i + prefix_len;
            // 前缀后跟数字+时间后缀
            if let Some((num_str, consumed)) = parse_cn_number(&chars, after, Some(r)) {
                if consumed > 0 {
                    // 检查后面是否有时间后缀
                    let after_num = after + consumed;
                    if after_num < chars.len()
                        && r.is_date_suffix(&chars[after_num..].iter().collect::<String>())
                    {
                        for ch in &chars[i..after] {
                            result.push(*ch);
                        }
                        result.push_str(&num_str);
                        i = after + consumed;
                        continue;
                    }
                    // 前缀后面不是时间后缀 → 不转换
                    for ch in &chars[i..after] {
                        result.push(*ch);
                    }
                    i = after;
                    continue;
                }
            }
        }

        // 检查负数前缀 "零下X度"
        if let Some((is_neg, prefix_consumed)) = parse_negative_prefix(&chars, i) {
            let after = i + prefix_consumed;
            if let Some((num_str, consumed)) = parse_cn_number(&chars, after, Some(r)) {
                if consumed > 0 {
                    // 检查后面是否有单位
                    let after_num = after + consumed;
                    let after_str: String = chars[after_num..].iter().collect();
                    if r.is_unit(&after_str) || r.is_date_suffix(&after_str) {
                        // 输出负号或文本风格
                        if r.below_zero_style == "minus" {
                            result.push('-');
                            result.push_str(&num_str);
                        } else {
                            // "text" 风格：零下10度
                            for ch in &chars[i..after] {
                                result.push(*ch);
                            }
                            result.push_str(&num_str);
                        }
                        // TEMP-CELSIUS-001: 摄氏关键词 → 输出 ℃ 符号
                        // 与 below_zero_style 联动（minus→"-10℃"，text→"零下10℃"）
                        if let Some((unit_len, unit_word)) = r.match_unit_word(&chars, after_num) {
                            if unit_word.contains("摄氏") {
                                result.push_str("℃");
                                i = after_num + unit_len;
                                continue;
                            }
                        }
                        i = after + consumed;
                        continue;
                    }
                    // 无单位 → 不转换
                    for ch in &chars[i..after] {
                        result.push(*ch);
                    }
                    i = after;
                    continue;
                }
            }
        }

        // 检查中文数字 + 单位语境
        if is_cn_num_char(chars[i]) || (chars[i] == '零' || chars[i] == '〇') {
            // 尝试解析数字
            if let Some((num_str, consumed)) = parse_cn_number(&chars, i, Some(r)) {
                if consumed > 0 {
                    let after_num = i + consumed;
                    let after_str: String = if after_num < chars.len() {
                        chars[after_num..].iter().collect()
                    } else {
                        String::new()
                    };

                    // 判定语境
                    let should_convert =
                        decide_conversion(&num_str, consumed, &chars, i, after_num, &after_str, r);

                    if should_convert {
                        result.push_str(&num_str);
                        // TEMP-CELSIUS-001: 摄氏关键词 → 输出 ℃ 符号
                        // 仅当匹配单位词含"摄氏"时替换（"三十摄氏度"→"30℃"，
                        // "三十度"→"30度" 不变）。after_num 处即单位起点。
                        if let Some((unit_len, unit_word)) = r.match_unit_word(&chars, after_num) {
                            if unit_word.contains("摄氏") {
                                result.push_str("℃");
                                i = after_num + unit_len;
                                continue;
                            }
                        }
                        i = after_num;
                        continue;
                    }
                }
            }
        }

        // 普通字符，原样输出
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// 判定是否应该转换
/// ITN-SMART-002: `num_str` 不再用于位数判定（改用 consumed 源汉字数），
/// 保留参数以维持调用签名稳定，便于未来扩展（如逐位串特殊处理）。
fn decide_conversion(
    _num_str: &str,
    consumed: usize,
    chars: &[char],
    start: usize,
    after_num: usize,
    after_str: &str,
    r: &CompiledRules,
) -> bool {
    // 如果后面有单位 → 转
    if r.is_unit(after_str) {
        return true;
    }
    // 如果后面有日期后缀 → 转
    if r.is_date_suffix(after_str) {
        return true;
    }
    // 如果前面有日期前缀 → 转
    // (已在前面处理)

    // 数字长度 > 1（多位数）→ 转
    // ITN-SMART-002: 改用源汉字消耗数（consumed）而非输出位数
    // （num_str digit_count）。原实现用输出位数会让单字"十"→"10"（两位输出）
    // 误判为多位数，绕过下方单字保护，导致"五代十国"→"五代10国"。
    // consumed 统计的是源文本被 parse_cn_number 吃掉的汉字数：
    //   "十"        → consumed=1（单字，走单字保护路径）
    //   "二十五"    → consumed=2（多位，转）
    //   "三百二"    → consumed=3（多位，转）
    //   "幺三八"    → consumed=3（逐位串，转）
    // 符合 DEC-030"单字数字无单位语境保留汉字"原则。
    if consumed >= 2 {
        return true;
    }

    // 单字数字：检查后面是否是通用量词
    if consumed == 1 {
        if r.convert_single_digit_with_classifier {
            return true;
        }
        // 检查是否跟通用量词 → 不转
        if let Some(cls) = r
            .classifier_set
            .iter()
            .find(|c| after_str.starts_with(c.as_str()))
        {
            let _ = cls;
            return false; // 保留"三个人"
        }
        // 检查前面是否是单位（货币/量纲后小数模式："五块八" → "5块8"）
        if start > 0 {
            let before_text: String = chars[..start].iter().collect();
            if r.is_unit_preceded(&before_text) {
                return true;
            }
        }
        // 无量词的单字数字 → 不转（保护"说七出七"）
        return false;
    }

    false
}

/// 检查保护白名单，返回匹配长度（0=未匹配）
fn check_protection(chars: &[char], start: usize, r: &CompiledRules) -> Option<usize> {
    let rest: String = chars[start..].iter().collect();

    // 成语（最长匹配优先，4字）
    for idiom in &r.idiom_set {
        if rest.starts_with(idiom.as_str()) {
            return Some(idiom.chars().count());
        }
    }

    // 专有名词
    for noun in &r.proper_noun_set {
        if rest.starts_with(noun.as_str()) {
            return Some(noun.chars().count());
        }
    }

    // ITN-SMART-002: 历史/文化/民俗词汇（"五代十国"等）
    for hist in &r.historical_set {
        if rest.starts_with(hist.as_str()) {
            return Some(hist.chars().count());
        }
    }

    // 虚词"一"的搭配
    for fw in &r.function_word_set {
        if rest.starts_with(fw.as_str()) {
            return Some(fw.chars().count());
        }
    }

    None
}

/// 尝试解析分数 "X分之Y"（中文：Y 为分子，X 为分母。如"三分之一"=1/3）
fn try_parse_fraction(
    chars: &[char],
    start: usize,
    r: &CompiledRules,
) -> Option<(String, String, usize)> {
    let pattern_chars: Vec<char> = r.fraction_pattern.chars().collect();

    // 先找 "分之" 的位置
    let mut pos = start;
    while pos < chars.len() {
        if chars_match(&chars, pos, &pattern_chars) {
            // 分母字在 start..pos（分之左侧：三），分子在 pos+pattern_chars.len()..（分之右侧：一）
            let denom_chars = &chars[start..pos];
            if denom_chars.is_empty() {
                return None;
            }
            if let Some((denom_val, d_consumed)) = parse_cn_number(denom_chars, 0, Some(r)) {
                if d_consumed == denom_chars.len() {
                    let num_start = pos + pattern_chars.len();
                    if let Some((num_val, n_consumed)) = parse_cn_number(chars, num_start, Some(r))
                    {
                        if n_consumed > 0 {
                            let total = num_start + n_consumed - start;
                            return Some((num_val, denom_val, total));
                        }
                    }
                }
            }
            return None;
        }
        // 不是 pattern 且不是数字字符 → 停止搜索
        if !is_cn_num_char(chars[pos]) && chars[pos] != '零' && chars[pos] != '〇' {
            break;
        }
        pos += 1;
    }
    None
}

/// 字符匹配
fn chars_match(chars: &[char], pos: usize, pattern: &[char]) -> bool {
    if pos + pattern.len() > chars.len() {
        return false;
    }
    chars[pos..pos + pattern.len()] == *pattern
}

/// 匹配经纬度前缀，返回匹配长度
fn match_geo_prefix(chars: &[char], pos: usize, r: &CompiledRules) -> Option<usize> {
    let rest: String = chars[pos..].iter().collect();
    for prefix in &r.geo_prefixes {
        if rest.starts_with(prefix.as_str()) {
            return Some(prefix.chars().count());
        }
    }
    None
}

/// 匹配日期时间前缀，返回匹配长度
fn match_date_prefix(chars: &[char], pos: usize, r: &CompiledRules) -> Option<usize> {
    let rest: String = chars[pos..].iter().collect();
    for prefix in &r.date_prefixes {
        if rest.starts_with(prefix.as_str()) {
            return Some(prefix.chars().count());
        }
    }
    None
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 使用内置规则测试
    fn normalize_test(text: &str) -> String {
        let r = rules();
        normalize_with_rules(text, r)
    }

    /// 使用自定义规则测试（模拟文件覆盖）
    fn normalize_with(content: &str, text: &str) -> String {
        let r = compile_rules_from_content(content);
        normalize_with_rules(text, &r)
    }

    // ============================================================
    // 1. 通用多位数
    // ============================================================

    #[test]
    fn multi_digit_325() {
        assert_eq!(normalize_test("三百二十五"), "325");
    }

    #[test]
    fn multi_digit_1020() {
        assert_eq!(normalize_test("一千零二十"), "1020");
    }

    #[test]
    fn multi_digit_25000() {
        assert_eq!(normalize_test("两万五"), "25000");
    }

    #[test]
    fn multi_digit_in_sentence() {
        assert_eq!(normalize_test("总共三百二十五个"), "总共325个");
    }

    // ============================================================
    // 2. 逐位串≥3位
    // ============================================================

    #[test]
    fn serial_phone_138() {
        assert_eq!(normalize_test("幺三八零零"), "13800");
    }

    #[test]
    fn serial_room_302() {
        assert_eq!(normalize_test("三零二房间"), "302房间");
    }

    #[test]
    fn serial_year_2026() {
        assert_eq!(normalize_test("二零二六"), "2026");
    }

    // ============================================================
    // 3. 日期
    // ============================================================

    #[test]
    fn date_full() {
        assert_eq!(normalize_test("二零二六年七月十日"), "2026年7月10日");
    }

    #[test]
    fn date_short_year() {
        assert_eq!(normalize_test("九八年"), "98年");
    }

    // ============================================================
    // 4. 时间
    // ============================================================

    #[test]
    fn time_afternoon() {
        assert_eq!(normalize_test("下午三点五十分"), "下午3点50分");
    }

    #[test]
    fn time_half() {
        assert_eq!(normalize_test("八点半"), "8点半");
    }

    // ============================================================
    // 5. 小数
    // ============================================================

    #[test]
    fn decimal_pi() {
        assert_eq!(normalize_test("三点一四"), "3.14");
    }

    // ============================================================
    // 6. 百分比
    // ============================================================

    #[test]
    fn percentage_50() {
        assert_eq!(normalize_test("百分之五十"), "50%");
    }

    #[test]
    fn percentage_3_5() {
        assert_eq!(normalize_test("百分之三点五"), "3.5%");
    }

    // ============================================================
    // 7. 分数
    // ============================================================

    #[test]
    fn fraction_one_third() {
        assert_eq!(normalize_test("三分之一"), "1/3");
    }

    // ============================================================
    // 8. 金额
    // ============================================================

    #[test]
    fn money_yuan() {
        assert_eq!(normalize_test("三百五十元"), "350元");
    }

    #[test]
    fn money_kuai() {
        assert_eq!(normalize_test("五块八"), "5块8");
    }

    #[test]
    fn money_wan() {
        assert_eq!(normalize_test("三千五百万元"), "3500万元");
    }

    // ============================================================
    // 9. 计量单位
    // ============================================================

    #[test]
    fn unit_temp() {
        assert_eq!(normalize_test("二十五度"), "25度");
    }

    #[test]
    fn unit_meter() {
        assert_eq!(normalize_test("五米"), "5米");
    }

    #[test]
    fn unit_volt() {
        assert_eq!(normalize_test("二百二十伏"), "220伏");
    }

    #[test]
    fn unit_below_zero_text() {
        assert_eq!(normalize_test("零下十度"), "零下10度");
    }

    // ============================================================
    // ITN-SMART-002: 单字 十/百/千 无语境保留（算法根治验证）
    // ============================================================

    #[test]
    fn itn_smart_002_single_shi_no_context_preserved() {
        // "十" 单字无单位/日期/序数语境 → 保留（原实现会误转"10"）
        assert_eq!(normalize_test("十"), "十");
    }

    #[test]
    fn itn_smart_002_single_bai_no_context_preserved() {
        assert_eq!(normalize_test("百"), "百");
    }

    #[test]
    fn itn_smart_002_single_qian_no_context_preserved() {
        assert_eq!(normalize_test("千"), "千");
    }

    #[test]
    fn itn_smart_002_single_shi_with_unit_converts() {
        // "十元" 有货币单位 → 转
        assert_eq!(normalize_test("十元"), "10元");
    }

    #[test]
    fn itn_smart_002_single_shi_with_date_suffix_converts() {
        // "十点" 有日期后缀 → 转
        assert_eq!(normalize_test("十点"), "10点");
    }

    #[test]
    fn itn_smart_002_single_shi_with_classifier_preserved() {
        // "十个人" 单字+量词，开关 false → 保留
        assert_eq!(normalize_test("十个人"), "十个人");
    }

    #[test]
    fn itn_smart_002_multi_digit_still_converts() {
        // "二十五""十五""三百二" consumed≥2 → 转
        assert_eq!(normalize_test("二十五"), "25");
        assert_eq!(normalize_test("十五"), "15");
        assert_eq!(normalize_test("三百二"), "302");
    }

    #[test]
    fn itn_smart_002_multi_digit_with_unit_unchanged() {
        assert_eq!(normalize_test("二十五块"), "25块");
    }

    // ============================================================
    // ITN-SMART-002: 历史/文化/民俗词汇保护
    // ============================================================

    #[test]
    fn itn_smart_002_wudai_shiguo_preserved() {
        // Gavin 端测 bug 原 case："五代十国"→"五代10国"
        assert_eq!(normalize_test("五代十国"), "五代十国");
    }

    #[test]
    fn itn_smart_002_wudai_shiguo_in_sentence() {
        assert_eq!(
            normalize_test("历史上五代十国时期战乱频仍"),
            "历史上五代十国时期战乱频仍"
        );
    }

    #[test]
    fn itn_smart_002_sanhuang_wudi_preserved() {
        assert_eq!(normalize_test("三皇五帝"), "三皇五帝");
    }

    #[test]
    fn itn_smart_002_wuhu_shiliuguo_preserved() {
        assert_eq!(normalize_test("五胡十六国"), "五胡十六国");
    }

    #[test]
    fn itn_smart_002_sanshiliuji_preserved() {
        assert_eq!(normalize_test("三十六计"), "三十六计");
    }

    #[test]
    fn itn_smart_002_ershisi_jieqi_preserved() {
        assert_eq!(normalize_test("二十四节气"), "二十四节气");
    }

    #[test]
    fn itn_smart_002_sishu_wujing_preserved() {
        assert_eq!(normalize_test("四书五经"), "四书五经");
    }

    #[test]
    fn itn_smart_002_historical_rules_file_loaded() {
        // 验证 historical 分组从 itn-rules.toml 正确加载
        let r = rules();
        assert!(r.historical_set.contains("五代十国"));
        assert!(r.historical_set.contains("三皇五帝"));
        assert!(r.historical_set.contains("二十四节气"));
        assert!(r.historical_set.contains("四书五经"));
    }

    #[test]
    fn itn_smart_002_historical_in_mixed_sentence() {
        // 混合句：历史词保护 + 普通数字仍转
        assert_eq!(
            normalize_test("他研究五代十国历史，写了三百五十页"),
            "他研究五代十国历史，写了350页"
        );
    }

    // ============================================================
    // TEMP-CELSIUS-001: 摄氏度符号 ℃（仅"摄氏"关键词触发）
    // ============================================================

    #[test]
    fn temp_celsius_basic() {
        // "三十摄氏度" → "30℃"
        assert_eq!(normalize_test("三十摄氏度"), "30℃");
    }

    #[test]
    fn temp_celsius_in_sentence() {
        // "今天三十摄氏度" → "今天30℃"
        assert_eq!(normalize_test("今天三十摄氏度"), "今天30℃");
    }

    #[test]
    fn temp_bare_degree_not_converted() {
        // 裸"度"不转 ℃（避免误判角度等其他语境）
        assert_eq!(normalize_test("今天三十度"), "今天30度");
    }

    #[test]
    fn temp_celsius_below_zero_text_style() {
        // "零下十摄氏度" + text 风格 → "零下10℃"
        assert_eq!(normalize_test("零下十摄氏度"), "零下10℃");
    }

    #[test]
    fn temp_celsius_below_zero_minus_style() {
        let custom = r#"
[switches]
below_zero_style = "minus"
[units.temperature]
words = ["度", "摄氏度"]
"#;
        // "零下十摄氏度" + minus 风格 → "-10℃"
        assert_eq!(normalize_with(custom, "零下十摄氏度"), "-10℃");
    }

    #[test]
    fn temp_bare_degree_below_zero_unchanged() {
        // "零下十度" 仍为"零下10度"，不受摄氏逻辑影响
        assert_eq!(normalize_test("零下十度"), "零下10度");
    }

    #[test]
    fn temp_celsius_single_digit() {
        // "五摄氏度" → "5℃"（单字+温度单位 → 转）
        assert_eq!(normalize_test("五摄氏度"), "5℃");
    }

    // ============================================================
    // 10. 经纬度
    // ============================================================

    #[test]
    fn geo_longitude() {
        assert_eq!(normalize_test("东经一百一十六点四度"), "东经116.4度");
    }

    // ============================================================
    // 11. 序数
    // ============================================================

    #[test]
    fn ordinal_chapter() {
        assert_eq!(normalize_test("第三十五章"), "第35章");
    }

    #[test]
    fn ordinal_rank() {
        assert_eq!(normalize_test("第八名"), "第8名");
    }

    // ============================================================
    // 12. 岁数楼层
    // ============================================================

    #[test]
    fn age() {
        assert_eq!(normalize_test("二十五岁"), "25岁");
    }

    #[test]
    fn floor() {
        assert_eq!(normalize_test("十八楼"), "18楼");
    }

    // ============================================================
    // 保护：裸单字数字
    // ============================================================

    #[test]
    fn protect_single_digit() {
        assert_eq!(normalize_test("说七出七"), "说七出七");
    }

    #[test]
    fn protect_single_digit_in_sentence() {
        assert_eq!(normalize_test("我想吃三个苹果"), "我想吃三个苹果");
    }

    // ============================================================
    // 保护：单字+通用量词
    // ============================================================

    #[test]
    fn protect_classifier_three_people() {
        assert_eq!(normalize_test("三个人"), "三个人");
    }

    #[test]
    fn protect_classifier_five_things() {
        assert_eq!(normalize_test("五件事"), "五件事");
    }

    #[test]
    fn protect_classifier_two_teachers() {
        assert_eq!(normalize_test("两位老师"), "两位老师");
    }

    // ============================================================
    // 保护：成语白名单
    // ============================================================

    #[test]
    fn protect_idiom() {
        assert_eq!(normalize_test("三心二意"), "三心二意");
    }

    #[test]
    fn protect_idiom_in_sentence() {
        assert_eq!(normalize_test("他做事三心二意的"), "他做事三心二意的");
    }

    // ============================================================
    // 保护：专有名词白名单
    // ============================================================

    #[test]
    fn protect_proper_noun_sanya() {
        assert_eq!(normalize_test("去三亚旅游"), "去三亚旅游");
    }

    #[test]
    fn protect_proper_noun_wuyi() {
        assert_eq!(normalize_test("五一放假"), "五一放假");
    }

    // ============================================================
    // 保护：虚词"一"
    // ============================================================

    #[test]
    fn protect_function_word() {
        assert_eq!(normalize_test("我们一起去"), "我们一起去");
        assert_eq!(normalize_test("看一下"), "看一下");
        assert_eq!(normalize_test("一直走"), "一直走");
    }

    // ============================================================
    // 幂等性
    // ============================================================

    #[test]
    fn idempotent_pure_digits() {
        assert_eq!(normalize_test("325"), "325");
        assert_eq!(normalize_test("2026年7月10日"), "2026年7月10日");
    }

    #[test]
    fn idempent_mixed_text() {
        // 已含阿拉伯数字的文本不变
        assert_eq!(normalize_test("价格350元"), "价格350元");
        assert_eq!(normalize_test("温度25度"), "温度25度");
    }

    #[test]
    fn idempotent_qwen3_output() {
        // qwen3 自带标点的输出保持不变
        assert_eq!(normalize_test("你好，世界！"), "你好，世界！");
    }

    // ============================================================
    // 规则文件缺失/损坏降级
    // ============================================================

    #[test]
    fn fallback_empty_rules() {
        let empty_rules = "";
        // 空规则 → 默认值，单字不转但多位数字仍可解析
        let result = normalize_with(empty_rules, "三百二十五");
        // 空规则时 units 为空，无单位触发 → 但 digit_count >= 2 仍转
        assert_eq!(result, "325");
    }

    #[test]
    fn fallback_invalid_toml() {
        let bad_toml = "this is not valid toml {{{{";
        let r = compile_rules_from_content(bad_toml);
        let result = normalize_with_rules("三百二十五", &r);
        // 无单位但有进位组合 ≥2位 → 仍转
        assert_eq!(result, "325");
    }

    #[test]
    fn fallback_missing_rules_file() {
        // 模拟规则文件不存在 → 使用内置默认
        let result = normalize_test("三百二十五");
        assert_eq!(result, "325");
    }

    // ============================================================
    // 混合文本只转命中片段
    // ============================================================

    #[test]
    fn mixed_text_partial() {
        assert_eq!(
            normalize_test("我买了三百二十五元的五米布"),
            "我买了325元的5米布"
        );
    }

    #[test]
    fn mixed_text_preserve_non_numbers() {
        assert_eq!(
            normalize_test("今天下午三点开会，三心二意不行"),
            "今天下午3点开会，三心二意不行"
        );
    }

    // ============================================================
    // 零下温度样式开关
    // ============================================================

    #[test]
    fn below_zero_text_style() {
        assert_eq!(normalize_test("零下十度"), "零下10度");
    }

    #[test]
    fn below_zero_minus_style() {
        let custom = r#"
[switches]
below_zero_style = "minus"
[units.temperature]
words = ["度"]
"#;
        assert_eq!(normalize_with(custom, "零下十度"), "-10度");
    }
}
