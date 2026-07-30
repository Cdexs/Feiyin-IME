//! ITN-SMART-001 智能数字规整模块（DEC-030）
//!
//! 自研中文数字→阿拉伯数字转换：多位数字/计量语境转数字，单字数字无单位保留汉字。
//! 算法与规则数据分离：规则在 itn-rules.toml（include_str! 内置默认，运行时可覆盖）。
//!
//! 核心流程：扫描文本 → 识别中文数字段 → 判定语境（单位/日期/序数/百分比/分数/小数）→
//!   决定是否转换 → 保护白名单优先 → 输出转换结果。

use std::collections::{HashMap, HashSet};
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
    #[serde(default)]
    unit_symbols: UnitSymbols,
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
    /// ITN-COLLISION-TYPEA-002: 单位前缀碰撞保护词（机器派生，jieba+THUOCL）
    #[serde(default)]
    unit_collisions: ProtectList,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ProtectList {
    #[serde(default)]
    words: Vec<String>,
}

/// ITN-CELSIUS-003: 单位符号规整规则（独立于中文数字转换路径）。
/// 对已是阿拉伯数字的文本生效：数字 + trigger → 数字 + replacement。
#[derive(Debug, Clone, Deserialize, Default)]
struct UnitSymbols {
    #[serde(default)]
    rules: Vec<UnitSymbolRule>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct UnitSymbolRule {
    trigger: String,
    replacement: String,
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
    /// ITN-COLLISION-TYPEA-002: 单位前缀碰撞保护词，按首字分桶
    /// 桶内按字符数降序排列（保证确定性最长匹配）
    unit_collision_map: HashMap<char, Vec<String>>,
    /// ITN-CELSIUS-003: 单位符号规整规则（阿拉伯数字 + trigger → replacement）
    /// 按 trigger 字符长度降序排列（最长匹配优先）
    unit_symbol_rules: Vec<(String, String)>,
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
            unit_collision_map: {
                let mut map: HashMap<char, Vec<String>> = HashMap::new();
                for w in &r.protect.unit_collisions.words {
                    if let Some(first) = w.chars().next() {
                        map.entry(first).or_default().push(w.clone());
                    }
                }
                // 桶内按字符数降序排序，确保最长匹配优先
                for bucket in map.values_mut() {
                    bucket.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
                }
                map
            },
            unit_symbol_rules: {
                let mut rules: Vec<(String, String)> = r
                    .unit_symbols
                    .rules
                    .iter()
                    .map(|rule| (rule.trigger.clone(), rule.replacement.clone()))
                    .collect();
                // Longest trigger first for correct matching
                rules.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));
                rules
            },
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
/// 位置：三分支产出后、本地标点前（ITN-REORDER-001，2026-07-30 反转，理由见 DEC-035）。
/// 幂等：`f(f(x)) == f(x)`。
///
/// 注意：自 ITN-CELSIUS-003 起，已是阿拉伯数字的文本仍会做单位符号规整
/// （如 `40摄氏度` → `40℃`），故「逐字节不变」不再字面成立。幂等性保持
/// （`40℃` 再跑一遍仍 `40℃`，因 `℃` 不匹配任何 unit_symbols trigger）。
pub fn normalize_numbers(text: &str) -> String {
    let r = rules();
    let after_cn = normalize_with_rules(text, r);
    // ITN-CELSIUS-003: 独立通道——对已是阿拉伯数字的文本做单位符号规整。
    // 在中文数字转换之后运行，不依赖中文数字解析路径。
    normalize_unit_symbols(&after_cn, r)
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

/// ITN-CELSIUS-003: 单位符号规整（独立通道，对已是阿拉伯数字的文本生效）。
///
/// 扫描文本中的阿拉伯数字序列（含可选负号/小数点），当其后紧跟一个
/// `unit_symbols` trigger（如「摄氏度」「摄氏」「°C」）时，将 trigger
/// 替换为对应的 replacement（如「℃」），数字部分原样保留。
///
/// 幂等：`40℃` 不含任何 trigger → 不变；`40摄氏度` → `40℃` → 再跑仍 `40℃`。
/// 「度」单独不在此列（角度/温度同形，Gavin 2026-07-27 拍板）。
///
/// 负数前缀联动 below_zero_style：
/// - `-10摄氏度` → minus 风格 `-10℃` / text 风格 `-10℃`（负号已是阿拉伯形式，保留）
/// - `零下10摄氏度` → minus 风格 `-10℃` / text 风格 `零下10℃`
fn normalize_unit_symbols(text: &str, r: &CompiledRules) -> String {
    if r.unit_symbol_rules.is_empty() {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(chars.len());
    let mut i = 0;

    while i < chars.len() {
        // ITN-CELSIUS-003: try to match Arabic digits (with optional 零下/- prefix)
        // followed by a unit symbol trigger (摄氏度/摄氏/°C → ℃)
        if let Some((output, input_len)) = try_match_arabic_symbol(&chars, i, r) {
            result.push_str(&output);
            i += input_len;
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// ITN-CELSIUS-003: Match Arabic digits (optionally preceded by 零下/-) followed by
/// a unit symbol trigger. Returns (output_string, input_chars_consumed) on match.
/// Handles below_zero_style for 零下 prefix.
fn try_match_arabic_symbol(
    chars: &[char],
    pos: usize,
    r: &CompiledRules,
) -> Option<(String, usize)> {
    try_match_neg_prefix_arabic_symbol(chars, pos, r)
        .or_else(|| try_match_plain_arabic_symbol(chars, pos, r))
}

/// Match 零下 + Arabic digits + symbol trigger
fn try_match_neg_prefix_arabic_symbol(
    chars: &[char],
    pos: usize,
    r: &CompiledRules,
) -> Option<(String, usize)> {
    let neg_prefix: Vec<char> = "零下".chars().collect();
    if pos + neg_prefix.len() > chars.len() {
        return None;
    }
    if &chars[pos..pos + neg_prefix.len()] != neg_prefix.as_slice() {
        return None;
    }
    let after_prefix = pos + neg_prefix.len();
    let (digits_str, digits_len) = scan_arabic_digits(&chars[after_prefix..])?;
    if digits_len == 0 {
        return None;
    }
    let after_digits = after_prefix + digits_len;
    let (trigger_len, replacement) = match_symbol_trigger(&chars[after_digits..], r)?;
    let output = if r.below_zero_style == "minus" {
        format!("-{}{}", digits_str, replacement)
    } else {
        format!("零下{}{}", digits_str, replacement)
    };
    let input_len = neg_prefix.len() + digits_len + trigger_len;
    Some((output, input_len))
}

/// Match plain Arabic digits (+ optional leading -) + symbol trigger
fn try_match_plain_arabic_symbol(
    chars: &[char],
    pos: usize,
    r: &CompiledRules,
) -> Option<(String, usize)> {
    let mut start = pos;
    let mut has_minus = false;
    // Optional leading minus sign
    if pos < chars.len() && chars[pos] == '-' {
        has_minus = true;
        start = pos + 1;
    }
    let (digits_str, digits_len) = scan_arabic_digits(&chars[start..])?;
    if digits_len == 0 {
        return None;
    }
    let after_digits = start + digits_len;
    let (trigger_len, replacement) = match_symbol_trigger(&chars[after_digits..], r)?;
    let output = if has_minus {
        format!("-{}{}", digits_str, replacement)
    } else {
        format!("{}{}", digits_str, replacement)
    };
    let input_len = (start - pos) + digits_len + trigger_len;
    Some((output, input_len))
}

/// Scan a run of ASCII digits (optionally with one decimal point). Returns (string, char_count).
fn scan_arabic_digits(chars: &[char]) -> Option<(String, usize)> {
    let mut s = String::new();
    let mut count = 0;
    let mut seen_dot = false;
    for &c in chars {
        if c.is_ascii_digit() {
            s.push(c);
            count += 1;
        } else if c == '.' && !seen_dot && count > 0 {
            // Allow one decimal point (e.g., 40.5摄氏度)
            seen_dot = true;
            s.push(c);
            count += 1;
        } else {
            break;
        }
    }
    if count == 0 {
        None
    } else {
        Some((s, count))
    }
}

/// Match the longest unit_symbol trigger at the given position. Returns (trigger_char_len, replacement).
fn match_symbol_trigger(chars: &[char], r: &CompiledRules) -> Option<(usize, String)> {
    let rest: String = chars.iter().collect();
    for (trigger, replacement) in &r.unit_symbol_rules {
        if rest.starts_with(trigger.as_str()) {
            return Some((trigger.chars().count(), replacement.clone()));
        }
    }
    None
}
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

    // ITN-COLLISION-TYPEA-002: 单位前缀碰撞保护（机器派生词表，优先级低于上方人工分组）
    if let Some(bucket) = r.unit_collision_map.get(&chars[start]) {
        let remaining = chars.len() - start;
        for w in bucket {
            let n = w.chars().count();
            if n > remaining {
                continue; // 桶内已按长度降序，可直接跳过更长的
            }
            if rest.starts_with(w.as_str()) {
                return Some(n);
            }
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
    /// 主控修正（TEST-EXEC 2026-07-30）：本助手原先只调 `normalize_with_rules`，
    /// **绕过了公共入口 `normalize_numbers` 的第二阶段** `normalize_unit_symbols`
    /// （ITN-CELSIUS-003 新增）。后果：96 条断言长期测的是一条**非公共路径**，
    /// 新通道对它们全部不可见 —— TEST-EXEC 的 6 个失败即由此而来（生产代码是对的）。
    ///
    /// 已改为直接调用公共入口。**测试必须走生产入口**，否则测试面与真实行为会静默分叉：
    /// 这与同日 `builtin_rules_parse_ok` 之前那个「单测全用内联 fixture、覆盖不到真实 toml」
    /// 的黑洞属同一类缺陷。
    fn normalize_test(text: &str) -> String {
        normalize_numbers(text)
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
    // ITN-GEOMETRIC-001: 几何图形 + 「角」族复合词白名单
    // ============================================================
    //
    // 根因：「角」(itn-rules.toml:23 units.currency) 的 is_unit 用
    // s.starts_with(u) → 「角形」被判为单位语境 → 单字数字被转。
    // 「边」不在任何单位词表，故四边形本来不受影响，但仍纳入白名单作为护栏。
    //
    // 全部断言使用 normalize_test（公共入口 normalize_numbers，
    // 包含 normalize_with_rules + normalize_unit_symbols 两阶段）。

    /// P0-1: 几何图形主修复目标
    #[test]
    fn geometric_triangle_protected() {
        assert_eq!(normalize_test("三角形"), "三角形");
    }
    #[test]
    fn geometric_quadrilateral_protected() {
        assert_eq!(normalize_test("四边形"), "四边形");
    }
    #[test]
    fn geometric_pentagon_protected() {
        assert_eq!(normalize_test("五边形"), "五边形");
    }
    #[test]
    fn geometric_hexagon_protected() {
        assert_eq!(normalize_test("六边形"), "六边形");
    }

    /// P0-2: 八角形（主控补漏项）— 注意 check_protection 是整词前缀匹配，
    /// 白名单「八角形」不影响裸「八角」的货币转换
    #[test]
    fn geometric_octagon_protected() {
        assert_eq!(normalize_test("八角形"), "八角形");
    }

    /// P0-3: 白名单不外溢 — 裸「八角」不在白名单内，走正常货币转换。
    /// 通过代码分析（src/itn.rs:1010 decide_conversion）：
    ///   parse_cn_number("八") → ("8", 1)
    ///   is_unit("角") → true（角 ∈ units.currency）
    ///   返回 true → 得到 "8角"
    /// 这是白名单不误伤合法货币转换的护栏。
    #[test]
    fn geometric_bajiao_not_protected_currency_conversion() {
        assert_eq!(normalize_test("八角"), "8角");
    }

    /// P0-4: 「角」族复合词白名单
    #[test]
    fn geometric_angle_compound_san_jiao_zhou() {
        assert_eq!(normalize_test("三角洲"), "三角洲");
    }
    #[test]
    fn geometric_angle_compound_san_jiao_zhai() {
        assert_eq!(normalize_test("三角债"), "三角债");
    }
    #[test]
    fn geometric_angle_compound_san_jiao_lian() {
        assert_eq!(normalize_test("三角恋"), "三角恋");
    }
    #[test]
    fn geometric_angle_compound_san_jiao_han_shu() {
        assert_eq!(normalize_test("三角函数"), "三角函数");
    }

    /// P0-5: 反向护栏 — 合法转换未被误伤
    /// 三十度 → 30度（decide_conversion: consumed=2 ≥ 2 → 转）
    /// 五公斤 → 5公斤（decide_conversion: is_unit("公斤") → 转）
    /// 三角 → 3角（单字+货币单位，参见 P0-3 同理）
    /// 三分之一 → 1/3（fraction 模式，先于 check_protection 运行）
    #[test]
    fn geometric_legitimate_conversion_not_harmed() {
        assert_eq!(normalize_test("三十度"), "30度");
        assert_eq!(normalize_test("五公斤"), "5公斤");
        assert_eq!(normalize_test("三角"), "3角");
        assert_eq!(normalize_test("三分之一"), "1/3");
    }

    /// P1-1: 非数字前缀 + 白名单（以「正」「等腰」等开头，
    /// check_protection 扫描到内部「三角形」时命中白名单）
    #[test]
    fn geometric_special_triangle_prefix() {
        assert_eq!(normalize_test("正三角形"), "正三角形");
        assert_eq!(normalize_test("等腰三角形"), "等腰三角形");
        assert_eq!(normalize_test("直角三角形"), "直角三角形");
    }

    /// P1-2: 多位数字 + 白名单
    #[test]
    fn geometric_multi_digit_polyhedron() {
        assert_eq!(normalize_test("十二面体"), "十二面体");
        assert_eq!(normalize_test("二十面体"), "二十面体");
    }

    /// P1-3: 匹配顺序隐患护栏（注释说明）
    ///
    /// 无法用测试表达：check_protection（src/itn.rs:1063）遍历 HashSet，
    /// 迭代顺序未定义（因 proper_noun_set 是 HashSet<String>）。
    /// 当前 itn-rules.toml 无互相为前缀的条目冲突：
    ///   - 「三角」族（三角洲/三角债/三角恋…）起点相同但「三角」不在白名单内
    ///   - 同一几何图形组内各条目互不为前缀
    ///
    /// 隐患：若将来有人同时加入「三角」和「三角洲」，此二条互为前缀关系，
    /// 而 HashSet 迭代顺序未定义 → 先遍历到「三角」则匹配 2 字，「三角洲」
    /// 的保护被截断为「三角」+「洲」单独处理。但「洲」无单位语境 → 不会被误转，
    /// 只是保护不完全。修复方法：entry 加入时确保无前缀重叠，或改用 BTreeSet
    /// 并实现最长匹配优先。当前无实例，无法写确定性测试。
    #[test]
    fn geometric_order_hazard_documented() {
        // 本条仅为确保注释可见（空断言），隐患已在文档中记录
        assert!(true);
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

    // ============================================================
    // ITN-REORDER-001: LLM 成品输入域 — 新位置后 ITN 的新输入特征
    // ============================================================

    /// 带标点输入：全角标点不破坏数字/单位边界判定，ITN-CELSIUS-003 后
    /// 阿拉伯数字后的摄氏度 trigger 也会被规整为 ℃。
    #[test]
    fn itn_new_domain_with_punctuation() {
        assert_eq!(normalize_test("温度是40摄氏度。"), "温度是40℃。");
        assert_eq!(
            normalize_test("明天最高40摄氏度，最低25摄氏度。"),
            "明天最高40℃，最低25℃。"
        );
    }

    /// 汉字+标点混合：中文数字转换为阿拉伯数字，全角标点保留在原位
    #[test]
    fn itn_new_domain_cn_with_punct() {
        assert_eq!(normalize_test("温度是四十摄氏度。"), "温度是40℃。");
    }

    /// 多行输入：逐行处理，换行符不丢失
    #[test]
    fn itn_new_domain_multiline() {
        // 主控修正（TEST-EXEC 2026-07-30）：原期待值 "第一行25\n第二行30" 漏算了序数规则 ——
        // `itn-rules.toml:85-86` 的 `[ordinal] prefix = "第"` 会把「第一/第二」转成「第1/第2」
        // （DEC-030-③ 明列序数为覆盖场景）。故本例实际考察**两条规则跨换行同时生效**：
        // 序数（第一→第1）+ 多位数字（二十五→25），且 `\n` 原样保留。
        assert_eq!(
            normalize_test("第一行二十五\n第二行三十"),
            "第1行25\n第2行30"
        );
        assert_eq!(normalize_test("你好\n世界"), "你好\n世界");
    }

    /// 中英混排/技术术语：不被误改（回归风险最高的一类）
    #[test]
    fn itn_new_domain_tech_terms() {
        assert_eq!(normalize_test("按F4键"), "按F4键");
        assert_eq!(normalize_test("Ctrl+2"), "Ctrl+2");
        assert_eq!(normalize_test("v0.7.2"), "v0.7.2");
        assert_eq!(normalize_test("esbuild 0.21.5"), "esbuild 0.21.5");
    }

    /// 纯英文输入（翻译路径输出）：no-op 逐字节不变
    #[test]
    fn itn_new_domain_pure_english() {
        assert_eq!(
            normalize_test("The temperature is 40 degrees."),
            "The temperature is 40 degrees."
        );
    }

    // ============================================================
    // ITN-CELSIUS-003: 单位符号独立通道（normalize_unit_symbols）
    // ============================================================

    /// P0-1: 最长匹配优先——"摄氏度"（3 字）优先于"摄氏"（2 字），
    /// 若排序被破坏会退化为"40℃度"。
    #[test]
    fn unit_symbol_longest_match_first() {
        assert_eq!(normalize_test("40摄氏度"), "40℃");
    }

    /// P0-2: 必须有阿拉伯数字前缀——纯单位词不触发替换
    #[test]
    fn unit_symbol_requires_digit_prefix() {
        assert_eq!(normalize_test("摄氏度是温度单位"), "摄氏度是温度单位");
        assert_eq!(normalize_test("摄氏"), "摄氏");
    }

    /// P0-3: "度"（无"摄氏"）绝不转——角度/温度同形，Gavin 2026-07-27 拍板
    #[test]
    fn unit_symbol_bare_degree_unchanged() {
        assert_eq!(normalize_test("44度"), "44度");
        assert_eq!(normalize_test("转90度"), "转90度");
        assert_eq!(normalize_test("温度是44度。"), "温度是44度。");
    }

    /// P0-4: 幂等——两遍归一化结果与一遍相同；℃ 本身不触发任何 trigger
    #[test]
    fn unit_symbol_idempotent() {
        let once = normalize_test("温度是四十摄氏度");
        let twice = normalize_test(&once);
        assert_eq!(once, twice, "f(f(x)) == f(x)");
        assert_eq!(normalize_test("40℃"), "40℃");
    }

    /// P0-5: °C (U+00B0 + C) → ℃ (U+2103)
    #[test]
    fn unit_symbol_degree_c_to_celsius() {
        assert_eq!(normalize_test("40°C"), "40℃");
    }

    /// P1-1: 负号/零下联动——"-"前缀保持原有负号，"零下"按 below_zero_style
    /// (default="text") 输出"零下X℃"
    #[test]
    fn unit_symbol_negative_and_below_zero() {
        assert_eq!(normalize_test("-10摄氏度"), "-10℃");
        assert_eq!(normalize_test("零下10摄氏度"), "零下10℃");
    }

    /// P1-3: 端到端形态——LLM 输出含"摄氏度"时，ITN 补 ℃ 符号
    /// （即本批次修复的目标形态，亦由 itn_new_domain_with_punctuation 覆盖）
    #[test]
    fn unit_symbol_e2e_llm_output() {
        assert_eq!(normalize_test("温度是40摄氏度。"), "温度是40℃。");
    }
}
