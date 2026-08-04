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
    #[serde(default)]
    unit_hierarchy: UnitHierarchy,
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
    time: UnitList,
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
    /// ITN-FIX-GRADECLASS-016: 紧跟 2 位逐位串时抑制合并的后缀（年级班级简写等语法族）。
    /// 当 serial_len == 2 且紧随其后的字符命中本表时，不做逐位串合并（如「一三班」→
    /// 「13班」是错的，应为「一三班」= 一年级三班）。DEC-038：保护词表不得承载
    /// 规则性语法族，本表只承载「抑制后缀」这一规则参数，不逐条枚举 N 年级 M 班。
    #[serde(default)]
    serial_suffixes: ProtectList,
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

/// ITN-V2-004 (P4) 单位层级表：丙型多级单位链合并用。
/// 每族按单位→相对倍率。`分` 同时出现在 currency 和 time，由识别器按前驱单位消歧。
#[derive(Debug, Clone, Deserialize, Default)]
struct UnitHierarchy {
    #[serde(default)]
    currency: std::collections::HashMap<String, f64>,
    #[serde(default)]
    length: std::collections::HashMap<String, f64>,
    #[serde(default)]
    weight: std::collections::HashMap<String, f64>,
    #[serde(default)]
    time: std::collections::HashMap<String, f64>,
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
    /// ITN-V2-004 (P4) 单位层级表：丙型合并用，unit→(族名, 倍率)
    /// 一个单位可能在多族（如「分」在 currency+time），由识别器按前驱消歧
    unit_hierarchy: HashMap<String, Vec<(&'static str, f64)>>,
    /// 可小数化单位集合（currency+length+weight+volume+temperature+pressure+
    /// electrical+frequency+acoustic+data+time，排除 other/geo_prefix）
    decimalizable_units: HashSet<String>,
    /// ITN-FIX-GRADECLASS-016: 紧跟 2 位逐位串时抑制合并的后缀集合
    /// （年级班级简写等语法族，如「班」）
    serial_suffix_set: HashSet<String>,
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
            &r.units.time,
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
            unit_hierarchy: {
                let mut map: HashMap<String, Vec<(&'static str, f64)>> = HashMap::new();
                for (unit, val) in &r.unit_hierarchy.currency {
                    map.entry(unit.clone())
                        .or_default()
                        .push(("currency", *val));
                }
                for (unit, val) in &r.unit_hierarchy.length {
                    map.entry(unit.clone()).or_default().push(("length", *val));
                }
                for (unit, val) in &r.unit_hierarchy.weight {
                    map.entry(unit.clone()).or_default().push(("weight", *val));
                }
                for (unit, val) in &r.unit_hierarchy.time {
                    map.entry(unit.clone()).or_default().push(("time", *val));
                }
                map
            },
            decimalizable_units: {
                let mut set = HashSet::new();
                for list in [
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
                    &r.units.time,
                ] {
                    for w in &list.words {
                        set.insert(w.clone());
                    }
                }
                set
            },
            serial_suffix_set: r.protect.serial_suffixes.words.iter().cloned().collect(),
        }
    }

    fn is_unit(&self, s: &str) -> bool {
        self.all_units.iter().any(|u| s.starts_with(u))
    }

    /// ITN-V2-003 (甲型守卫)：判定是否「真单位」（在 all_units 但不在 classifiers）。
    /// 通用量词（个/间/件…）不算单位——甲型文法不处理「一个半」「五间半」，
    /// 它们保持汉字（守卫统一路径，符合 DEC-038）。
    /// 「间」同时在 units.other 和 classifiers，此处排除。
    fn is_real_unit(&self, s: &str) -> bool {
        self.all_units.iter().any(|u| s.starts_with(u))
            && !self
                .classifier_set
                .iter()
                .any(|c| s.starts_with(c.as_str()))
    }

    /// ITN-V2-004 (P4)：可小数化单位（排除 other/geo_prefix）。
    fn is_decimalizable(&self, s: &str) -> bool {
        self.decimalizable_units
            .iter()
            .any(|u| s.starts_with(u.as_str()))
    }

    /// ITN-V2-004 (P4)：取单位在某族的倍率。
    fn hierarchy_value(&self, unit: &str, family: &str) -> Option<f64> {
        self.unit_hierarchy
            .get(unit)
            .and_then(|entries| entries.iter().find(|(f, _)| *f == family).map(|(_, v)| *v))
    }

    /// ITN-V2-004 (P4)：取单位的所有族属。
    fn unit_families(&self, unit: &str) -> Vec<&'static str> {
        self.unit_hierarchy
            .get(unit)
            .map(|entries| entries.iter().map(|(f, _)| *f).collect())
            .unwrap_or_default()
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

/// ITN-FIX-CURRENCY-017：判定当前位置的「两」是否应视为重量单位而非数字2。
/// 前置条件：前有数字/进位字符（idx > start，即两 不是数字首位）。
/// 后接单位词（斤/块/毛/克/两 等 all_units）→ 两 是数字2（两块/三块两毛五）；
/// 后接非单位（数字/名词/边界）→ 两 是重量单位（二两/六两五/二十五两/三两银子）。
/// 进位单位（百/千/万）由进位组合路径自行继续，此处不误判（一亿两 → 两 单位）。
fn two_is_unit(chars: &[char], idx: usize, start: usize, r: &CompiledRules) -> bool {
    if chars[idx] != '两' || idx <= start {
        return false;
    }
    if r.match_unit_word(chars, idx + 1).is_some() {
        return false;
    }
    true
}

/// ITN-V2-001 (R2 缺陷A ①右邻否决)：判定词是否全由中文数字字符组成（含进位单位）。
/// 用于识别「十一」「五一」「八一」这类纯数字保护词，以便撤销保护时区分。
/// 例：「十一」=true、「三亚」=false、「五角大楼」=false。
fn is_pure_cn_digit_word(word: &str) -> bool {
    !word.is_empty() && word.chars().all(is_cn_num_char)
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

    // 先检测逐位串模式：连续≥2个纯数字字符（零/〇/一/二/三/四/五/六/七/八/九/幺/两）
    // 且后面不跟进位单位（十百千万亿）
    let mut serial_len = 0usize;
    for k in start..chars.len() {
        if chinese_digit_char(chars[k]).is_some() {
            // ITN-FIX-CURRENCY-017：两 兼重量单位 → 以单位词开头时终止逐位串扫描。
            // 二两/二十五两 的「两」不再被吞进逐位串（否则 → 22/222）。`两斤` 首位不受影响
            // （两 是数字2，交给进位组合路径）。当前词表仅「两」命中单位前缀，故等价于判两。
            if let Some(r) = rules {
                if r.match_unit_word(chars, k).is_some() {
                    break;
                }
            }
            serial_len += 1;
        } else {
            break;
        }
    }
    // 检查逐位串后面是否跟进位单位
    let serial_end = start + serial_len;
    let next_is_unit = serial_end < chars.len() && is_cn_unit_char(chars[serial_end]);

    // ITN-FIX-GRADECLASS-016: 年级班级简写守卫 —— 当 serial_len == 2 且紧随其后的
    // 字符命中 serial_suffixes（如「班」）时，不做逐位串合并，直接返回 None。
    //
    // 为什么 return None 而非「跳过 :582 的 early return 落进位组合路径」：
    // 进位组合路径对「一三」这类连续纯数字会按「末位 digit 覆盖」解析（digit 被
    // 反复覆盖只留最后一个），产出 ("3", 2) → 「一三班」→「3班」撕裂。return None
    // 后主循环 :1600 的 `if let Some(...)` 短路，字符逐个走「普通字符」单字路径，
    // 「班」既非单位也非量词 → 保持汉字，输出「一三班」。
    //
    // 为什么限定 serial_len == 2：
    //   - 「一三/五一/二三/一四」全部是 2 位，语义是两个不同层级的数字（年级+班号）
    //     被误当成一个两位数
    //   - 进位组合路径不受影响 → 「十三班」（十非逐位串首字符）仍转
    //   - ≥3 位逐位串（三零二房间/二零二六/幺三八零零）完全不进入本守卫
    if serial_len == 2 && serial_end < chars.len() {
        if let Some(r) = rules {
            let after_serial: String = chars[serial_end..].iter().collect();
            if r
                .serial_suffix_set
                .iter()
                .any(|s| after_serial.starts_with(s.as_str()))
            {
                return None;
            }
        }
    }

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
            // ITN-FIX-CURRENCY-017：两 在「前有数字 + 后接非单位」时是重量单位 → 终止数字。
            // 二十五两 → 25、六两五 → 6（两 不吞）；两斤/两百/三块两毛五 → 首位或后接单位 → 按数字2。
            if let Some(r) = rules {
                if ch == '两' && two_is_unit(chars, idx, start, r) {
                    break;
                }
            }
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

/// ITN-V2-001 (R1 双通道补丁通道)：仅运行单位符号规整（第二阶段），
/// 不做中文数字→阿拉伯转换（第一阶段）。
///
/// 用途：ITN 主通道已跑过 `normalize_numbers`（LLM 之前），LLM 可能纠正
/// ASR 同音错字（如「摄息」→「摄氏」）后输出「40摄氏度」，补丁通道在 LLM
/// 之后、本地标点之前调用本函数，捞回这类 LLM 纠正后的单位符号。
///
/// 幂等：主通道产出 `40℃`，补丁通道再跑 → `℃` 不匹配任何 trigger → 不变。
pub fn normalize_unit_symbols_only(text: &str) -> String {
    let r = rules();
    normalize_unit_symbols(text, r)
}

// ============================================================
// ITN-V2-004 (P4) 丙型「显式多级单位链」+ 乙型「隐式小数位」
// 取代 ITN-V2-001 的 ③ 块级匹配（任务D：③被丙型取代删除）
// ============================================================

/// 丙型识别器输出：结构化的多级「数字-单位」链。
#[derive(Debug, Clone)]
struct UnitChain {
    parts: Vec<(String, String)>,
    consumed: usize,
    family: &'static str,
    /// ITN-FIX-CURRENCY-017：货币链后的单价限定词（如「一斤」），原样汉字保留。
    /// 仅 currency 族链在终止点后紧跟 weight 族单位时设置（`三块四毛八一斤`→`3.48元一斤`）。
    per_unit: Option<String>,
}

/// 丙型识别器：扫描连续「中文数字+可小数化单位」多级链。
/// 与③的关键差异：match 失败用 break（保留已识别段）而非 ?（整体返回None）；
/// 单位必须 is_decimalizable（排除编号单位）；`分`消歧靠前驱。
fn try_parse_unit_chain(chars: &[char], start: usize, r: &CompiledRules) -> Option<UnitChain> {
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut pos = start;
    let mut family: Option<&'static str> = None;

    loop {
        let (num_str, num_consumed) = match parse_cn_number(&chars[pos..], 0, Some(r)) {
            Some(v) => v,
            None => break,
        };
        if num_consumed == 0 {
            break;
        }
        let after_num = pos + num_consumed;
        let rest: String = chars[after_num..].iter().collect();
        // 单位匹配：先试 all_units，再试 date_suffix（点/分/秒 时间族）
        let (unit_len, unit_word) = if let Some(v) = r.match_unit_word(chars, after_num) {
            v
        } else if let Some(len) = r.match_date_suffix_len(&rest) {
            let word: String = chars[after_num..after_num + len].iter().collect();
            // 仅时间 date_suffix（点/分/秒），排除号/日/年/月等日期
            if word == "点" || word == "分" || word == "秒" {
                (len, word)
            } else {
                break;
            }
        } else {
            // 无单位：检查隐含末级单位尾数
            // ITN-FIX-CHAIN-TEAR-026：去掉 after_is_boundary 检查，数值合成与后继识别正交。
            if !parts.is_empty() && after_num <= chars.len() {
                // 隐式尾数单位取决于链内最后一个显式单位的层级：
                //   块/元(≥1.0) → 隐式尾数=毛(0.1) → 五块一=5.1元
                //   毛/角(=0.1) → 隐式尾数=分(0.01) → 三块四毛八=3.48元
                //   分(=0.01)   → 已到最小单位，不再吸收隐式尾数
                let (_, last_unit) = parts.last().unwrap();
                if let Some(last_val) = r.hierarchy_value(last_unit, "currency") {
                    let implicit_unit = if last_val >= 1.0 {
                        "毛"
                    } else if last_val == 0.1 {
                        "分"
                    } else {
                        ""
                    };
                    if !implicit_unit.is_empty() && num_consumed <= 2 {
                        parts.push((num_str, implicit_unit.to_string()));
                        pos = after_num;
                    }
                }
            }
            break;
        };
        // 单位可小数化检查（date_suffix 时间词已在上方过滤，此处查 decimalizable）
        if unit_len == 0 {
            break;
        }
        let is_decimalizable = r.is_decimalizable(&unit_word);
        let is_time_suffix = unit_word == "点" || unit_word == "分" || unit_word == "秒";
        if !is_decimalizable && !is_time_suffix {
            break;
        }
        // `分`消歧：首段就是`分`→不纳入多级链（裸N分不合并）
        if unit_word == "分" && parts.is_empty() {
            break;
        }
        // ITN-FIX-CURRENCY-017 条件3：族一致性校验。多族单位（如「分」∈currency+time）
        // 优先消歧为当前链族，避免 time 链（两点五分）被「分」解析成 currency 误判不一致。
        let unit_fam = resolve_family_consistent(&unit_word, family.unwrap_or(""), r);
        if let Some(cur) = family {
            if unit_fam != cur {
                // 族不一致 → 终止链。currency 链终止点后紧跟 weight 族单位 →
                // 捕获单价限定词（如「一斤」），原样汉字保留在 per_unit。
                if cur == "currency" && unit_fam == "weight" {
                    if let Some(chain) = capture_price_per_unit(
                        chars,
                        start,
                        after_num,
                        unit_len,
                        &num_str,
                        num_consumed,
                        &mut parts,
                        r,
                    ) {
                        return Some(chain);
                    }
                }
                break;
            }
        } else {
            family = Some(unit_fam);
        }
        parts.push((num_str, unit_word.clone()));
        pos = after_num + unit_len;
        if pos >= chars.len()
            || !is_cn_num_char(chars[pos]) && chars[pos] != '零' && chars[pos] != '〇'
        {
            break;
        }
    }

    // ITN-FIX-CHAIN-TEAR-026：允许单段 currency 链（如「五块」=1段），否则逐字路径
    // 会撕裂成「5块」+「8」（只转数字不转单位）。currency 族在 format_unit_chain
    // 中会归一到「元」，所以单段「五块」→「5元」是合法且预期的行为。
    // 约束：单段非主单位（角/毛/分）在 format_currency_chain 中保留原单位。
    if parts.len() >= 2 || (parts.len() == 1 && family == Some("currency")) {
        Some(UnitChain {
            consumed: pos - start,
            parts,
            family: family.unwrap_or("length"),
            per_unit: None,
        })
    } else {
        None
    }
}

fn resolve_family(unit: &str, r: &CompiledRules) -> &'static str {
    let families = r.unit_families(unit);
    if families.is_empty() {
        if r.is_date_suffix(unit) {
            return "time";
        }
        return "length";
    }
    families[0]
}

/// ITN-FIX-CURRENCY-017 条件3：多族单位（如「分」∈currency+time）优先消歧为当前链族，
/// 避免 time 链（两点五分）被「分」解析成 currency 而误判族不一致导致链被截断。
fn resolve_family_consistent(unit: &str, cur: &str, r: &CompiledRules) -> &'static str {
    let fams = r.unit_families(unit);
    if let Some(f) = fams.iter().copied().find(|f| *f == cur) {
        f
    } else if fams.is_empty() {
        resolve_family(unit, r)
    } else {
        fams[0]
    }
}

/// ITN-FIX-CURRENCY-017 条件3：捕获 currency 链终止点后的单价限定词（weight 族单位，如「一斤」），
/// 返回带 per_unit 的完整链，供 try_parse_unit_chain 直接 return。
///
/// 拆分规则：紧邻 weight 单位的最后一个汉字数字归 per_unit 数量（原样汉字保留），
/// 其前数字作为货币链隐式尾位（`一块两毛二一斤` → [1块,2毛] + 隐式 二=2分 + per_unit 一斤）。
fn capture_price_per_unit(
    chars: &[char],
    start: usize,
    after_num: usize,
    unit_len: usize,
    num_str: &str,
    num_consumed: usize,
    parts: &mut Vec<(String, String)>,
    r: &CompiledRules,
) -> Option<UnitChain> {
    if num_consumed == 0 || parts.is_empty() || !num_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // 隐式尾位单位：链内最后一个显式货币单位的下一位（块/元 → 毛，毛/角 → 分）
    let tail_unit = {
        let (_, last_unit) = parts.last()?;
        let last_val = r.hierarchy_value(last_unit, "currency")?;
        if last_val >= 1.0 {
            "毛"
        } else if last_val == 0.1 {
            "分"
        } else {
            return None;
        }
    };
    // 隐式尾位数字 = parse 数字串去掉最后一位（逐位串下 num_str 即源数字序列）
    if num_consumed >= 2 {
        let tail_digits = &num_str[..num_str.len() - 1];
        parts.push((tail_digits.to_string(), tail_unit.to_string()));
    }
    // per_unit = 紧邻单位的最后一个源字符 + 单位词，原样汉字保留
    let per_unit_str: String = chars[after_num - 1..after_num + unit_len].iter().collect();
    let consumed = after_num + unit_len - start;
    Some(UnitChain {
        consumed,
        parts: std::mem::take(parts),
        family: "currency",
        per_unit: Some(per_unit_str),
    })
}

/// 丙型 formatter：按族分派（DEC-037）。
fn format_unit_chain(chain: &UnitChain, r: &CompiledRules) -> String {
    match chain.family {
        "currency" => format_currency_chain(chain, r),
        "time" => format_time_chain(chain, r),
        // ITN-FIX-CURRENCY-017 条件2：weight 族不走通用小数合成，走零乘法拼接
        "weight" => format_weight_chain(chain),
        _ => format_generic_chain(chain, r),
    }
}

fn format_currency_chain(chain: &UnitChain, r: &CompiledRules) -> String {
    // ITN-FIX-CHAIN-TEAR-026-B (Gavin 方案 C)：单段 currency 链一律保留原单位
    //（五块→5块 / 八角→8角 / 五块一斤→5块一斤），不归一到元；多段链才归一到元
    //（五块一→5.1元 / 一块八毛五→1.85元）。
    //
    // 判定依据是段数（parts.len()），不是有没有 per_unit。单段说明用户明确用了某个
    // 货币单位（块/角/毛），系统不替他改写表达；多段才有合成数值的必要，此时归一到
    // 元是计算结果而非改写。
    if chain.parts.len() == 1 {
        let (num_str, unit) = &chain.parts[0];
        let body = format!("{}{}", num_str, unit);
        // 单段链同样可带 per_unit（如「五块一斤」→ 单价限定词仍原样保留）
        match &chain.per_unit {
            Some(per) => format!("{}{}", body, per),
            None => body,
        }
    } else {

    let mut total: f64 = 0.0;
    for (num_str, unit) in &chain.parts {
        let n: f64 = num_str.parse().unwrap_or(0.0);
        let mult = r.hierarchy_value(unit, "currency").unwrap_or(1.0);
        total += n * mult;
    }
    let body = if total.fract() == 0.0 {
        format!("{}元", total as u64)
    } else {
        // ITN-FIX-CHAIN-TEAR-026 条件B：去尾随零，与乙型 format_implicit_decimal 行为一致。
        // 5.80 → 5.8，1.00 → 1，5.01 → 5.01。
        let s = format!("{:.2}", total);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!("{}元", s)
    };
    // ITN-FIX-CURRENCY-017 条件3：追加单价限定词（原样汉字，如 1.22元一斤）
    match &chain.per_unit {
        Some(per) => format!("{}{}", body, per),
        None => body,
    }
    }
}

/// ITN-FIX-CURRENCY-017 条件2 (候选甲，主控 2026-08-03 拍板)：weight 族链逐 parts 零乘法拼接。
/// 一斤二两 → 1斤2两（不合成 1.2斤）。末位裸数字（三斤六两五 的 5）不进入本链，由逐字路径自然补出。
/// ⚠️ unit_hierarchy.weight 中 斤/两 的 value 不参与本函数计算（死数据锁死）—— 若未来接回通用
/// 小数 formatter，必须先同步更新本函数，否则 一斤二两 会静默变 1.2斤（见 itn-rules.toml 注释）。
fn format_weight_chain(chain: &UnitChain) -> String {
    let mut out = String::new();
    for (num_str, unit) in &chain.parts {
        out.push_str(num_str);
        out.push_str(unit);
    }
    out
}

fn format_time_chain(chain: &UnitChain, r: &CompiledRules) -> String {
    let mut hours: u32 = 0;
    let mut minutes: u32 = 0;
    for (num_str, unit) in &chain.parts {
        let n: u32 = num_str.parse().unwrap_or(0);
        // 时间族：小时/点→小时；分/分钟→分钟；秒→秒（本批不处理秒级合并）
        if unit == "小时" || unit == "点" || unit == "时" {
            hours += n;
        } else if unit == "分" || unit == "分钟" {
            minutes += n;
        }
        let _ = r; // hierarchy_value 暂不用于时间（直接按单位类型分派）
    }
    if minutes == 0 {
        format!("{}:00", hours)
    } else {
        format!("{}:{}", hours, minutes)
    }
}

fn format_generic_chain(chain: &UnitChain, r: &CompiledRules) -> String {
    let main_unit = &chain.parts[0].1;
    let main_mult = r.hierarchy_value(main_unit, chain.family).unwrap_or(1.0);
    let mut total: f64 = 0.0;
    for (num_str, unit) in &chain.parts {
        let n: f64 = num_str.parse().unwrap_or(0.0);
        let mult = r.hierarchy_value(unit, chain.family).unwrap_or(main_mult);
        total += n * mult / main_mult;
    }
    if total.fract() == 0.0 {
        format!("{}{}", total as u64, main_unit)
    } else {
        let s = format!("{:.4}", total);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!("{}{}", s, main_unit)
    }
}

// ============================================================
// ITN-V2-004 (P4) 乙型「隐式小数位」
// ============================================================

#[derive(Debug, Clone)]
struct ImplicitDecimal {
    main_num: String,
    unit_word: String,
    tail: String,
    consumed: usize,
}

/// 乙型识别器：`N<可小数化单位>M`，M 纯数字尾数后紧邻边界。
fn try_parse_implicit_decimal(
    chars: &[char],
    start: usize,
    r: &CompiledRules,
) -> Option<ImplicitDecimal> {
    let (main_num, num_consumed) = parse_cn_number(chars, start, Some(r))?;
    if num_consumed == 0 {
        return None;
    }
    let after_num = start + num_consumed;
    let rest: String = chars[after_num..].iter().collect();
    if !r.is_decimalizable(&rest) {
        return None;
    }
    // date_suffix（点/分/秒）不走乙型，但「度」是温度单位兼date_suffix，允许
    if r.date_suffixes.iter().any(|d| rest.starts_with(d.as_str())) && !rest.starts_with("度") {
        return None;
    }
    let (unit_len, unit_word) = r.match_unit_word(chars, after_num)?;
    let after_unit = after_num + unit_len;
    let mut tail_chars: Vec<char> = Vec::new();
    let mut pos = after_unit;
    while pos < chars.len() {
        if let Some(d) = chinese_digit_char(chars[pos]) {
            // ITN-FIX-CURRENCY-017：尾数遇到单位词开头 → 终止。`一斤二两` 的「两」是重量
            // 单位而非小数位（两=2 兼数字，会被 chinese_digit_char 吞进尾数 → 1.22斤）。
            // `三块两毛` 的「两」同理：乙型让位，由丙型按显式链 [3块,2毛] 处理。
            if r.match_unit_word(chars, pos).is_some() {
                break;
            }
            tail_chars.push(d);
            pos += 1;
        } else {
            break;
        }
    }
    if tail_chars.is_empty() {
        return None;
    }
    // 边界护栏：尾数后必须紧邻边界
    if pos < chars.len() && !is_boundary_char(chars[pos]) {
        return None;
    }
    let tail: String = tail_chars.iter().collect();
    Some(ImplicitDecimal {
        main_num,
        unit_word,
        tail,
        consumed: pos - start,
    })
}

fn is_boundary_char(ch: char) -> bool {
    ch.is_ascii_punctuation() || is_cjk_punctuation(ch) || ch.is_whitespace()
}

fn is_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '。' | '，'
            | '、'
            | '；'
            | '：'
            | '？'
            | '！'
            | '"'
            | '"'
            | '\''
            | '\''
            | '《'
            | '》'
            | '（'
            | '）'
    )
}

/// 乙型 formatter：`N.M单位`（尾数逐位作小数位）。
/// 货币族归一到「元」（DEC-037 Gavin 拍板）；其他族保留原单位。
fn format_implicit_decimal(id: &ImplicitDecimal, r: &CompiledRules) -> String {
    let families = r.unit_families(&id.unit_word);
    if families.contains(&"currency") {
        // 货币归一：N.M单位 → N.M元（块/角/毛/分 → 元）
        // 块=元，所以 5.8块=5.8元；角/毛/分 需按倍率换算
        let mult = r.hierarchy_value(&id.unit_word, "currency").unwrap_or(1.0);
        let main: f64 = id.main_num.parse().unwrap_or(0.0);
        let tail_val: f64 = format!("0.{}", id.tail).parse().unwrap_or(0.0);
        let total = (main + tail_val) * mult;
        if total.fract() == 0.0 {
            format!("{}元", total as u64)
        } else {
            // 去尾零：5.80→5.8，5.83→5.83
            let s = format!("{:.2}", total);
            let s = s.trim_end_matches('0').trim_end_matches('.');
            format!("{}元", s)
        }
    } else {
        format!("{}.{}{}", id.main_num, id.tail, id.unit_word)
    }
}

// ============================================================
// ITN-V2-003 (P3 甲型文法)：余数后缀（半/刻）
// ============================================================

/// 甲型识别器输出（识别器与 formatter 分离，沿用 001 架构）。
#[derive(Debug, Clone)]
struct RemainderSuffix {
    /// 主数（阿拉伯数字串，如 "4"/"1"/"39"）
    main_num: String,
    /// 单位词原文（如 "点"/"吨"/"寸"）；量词穿透时为通用量词（"个"），真实单位在 `real_unit`
    unit_word: String,
    /// 量词穿透：若 `unit_word` 是通用量词且后跟真实单位，此处存真实单位；否则 None
    real_unit: Option<String>,
    /// 余数值（分钟数用于时间族，或 0.5 用于度量衡）
    remainder: RemainderKind,
    /// 识别器消耗的源字符数
    consumed: usize,
}

#[derive(Debug, Clone, Copy)]
enum RemainderKind {
    /// 半 = 0.5（度量衡）或 30分（时间）
    Half,
    /// 刻 = N×15分（仅时间，N∈{1,2,3}）
    Quarter(u32),
}

/// 甲型识别器：尝试在 `start` 处匹配余数后缀文法。
///
/// 两种子模式：
/// 1. **半模式** `N<单位>半`：主数 + 单位 + `半`（时间/度量衡/量词穿透）
/// 2. **刻模式** `N点M刻`：主数 + `点` + M(1/2/3) + `刻`（仅时间）
///
/// 反例护栏：
/// - `一刻钟`：`刻` 后跟 `钟` → 否决
/// - `半小时`/`半个小时`：前置 `半` 无主数 → 不匹配
/// - `三点五`：`五` 是数字非 `半/刻` → 不匹配
fn try_parse_remainder_suffix(
    chars: &[char],
    start: usize,
    r: &CompiledRules,
) -> Option<RemainderSuffix> {
    // 解析主数
    let (main_num, num_consumed) = parse_cn_number(chars, start, Some(r))?;
    if num_consumed == 0 {
        return None;
    }
    let after_num = start + num_consumed;
    let rest: String = chars[after_num..].iter().collect();

    // 尝试半模式：N<单位>半
    if let Some(rs) =
        try_parse_half_mode(chars, start, &main_num, num_consumed, after_num, &rest, r)
    {
        return Some(rs);
    }
    // 尝试刻模式：N点M刻（仅时间）
    if let Some(rs) =
        try_parse_quarter_mode(chars, start, &main_num, num_consumed, after_num, &rest, r)
    {
        return Some(rs);
    }
    None
}

/// 半模式：N<单位>半
fn try_parse_half_mode(
    _chars: &[char],
    _start: usize,
    main_num: &str,
    num_consumed_inner: usize,
    after_num: usize,
    rest: &str,
    r: &CompiledRules,
) -> Option<RemainderSuffix> {
    // 时间族：date_suffix（点/分/秒）+ 半
    if let Some(len) = r.match_date_suffix_len(rest) {
        let after_unit = after_num + len;
        if _chars.get(after_unit) == Some(&'半') {
            let word: String = _chars[after_num..after_num + len].iter().collect();
            return Some(RemainderSuffix {
                main_num: main_num.to_string(),
                unit_word: word,
                real_unit: None,
                remainder: RemainderKind::Half,
                consumed: num_consumed_inner + len + 1,
            });
        }
    }
    // 度量衡族：真单位（排除 classifiers）+ 半
    if r.is_real_unit(rest) {
        let (len, word) = r.match_unit_word(_chars, after_num)?;
        let after_unit = after_num + len;
        if _chars.get(after_unit) == Some(&'半') {
            return Some(RemainderSuffix {
                main_num: main_num.to_string(),
                unit_word: word,
                real_unit: None,
                remainder: RemainderKind::Half,
                consumed: num_consumed_inner + len + 1,
            });
        }
    }
    // 量词穿透：通用量词 + 半 + 真单位（一个半小时=1.5小时）
    // 结构：N<量词>半<真单位>，与普通 N<单位>半 顺序不同
    let cls = r
        .classifier_set
        .iter()
        .find(|c| rest.starts_with(c.as_str()))?;
    let cls_len = cls.chars().count();
    let after_cls = after_num + cls_len;
    // 量词后必须是「半」
    if _chars.get(after_cls) != Some(&'半') {
        return None;
    }
    let after_half = after_cls + 1;
    if after_half >= _chars.len() {
        return None;
    }
    // 半后必须是真单位
    let rest2: String = _chars[after_half..].iter().collect();
    if !r.is_real_unit(&rest2) {
        return None;
    }
    let (real_len, real_word) = r.match_unit_word(_chars, after_half)?;
    let cls_word: String = _chars[after_num..after_num + cls_len].iter().collect();
    Some(RemainderSuffix {
        main_num: main_num.to_string(),
        unit_word: cls_word,
        real_unit: Some(real_word),
        remainder: RemainderKind::Half,
        consumed: num_consumed_inner + cls_len + 1 + real_len,
    })
}

/// 刻模式：N点M刻（仅时间族）
fn try_parse_quarter_mode(
    chars: &[char],
    _start: usize,
    main_num: &str,
    num_consumed_inner: usize,
    after_num: usize,
    rest: &str,
    r: &CompiledRules,
) -> Option<RemainderSuffix> {
    // 必须以「点」开头
    if !rest.starts_with('点') {
        return None;
    }
    let after_point = after_num + 1;
    // 解析 M（1/2/3，单字）
    let (_m_str, m_consumed) = parse_cn_number(chars, after_point, Some(r))?;
    if m_consumed == 0 {
        return None;
    }
    let after_m = after_point + m_consumed;
    // 必须是「刻」
    if chars.get(after_m) != Some(&'刻') {
        return None;
    }
    // 刻后跟「钟」→否决（一刻钟=时长）
    if after_m + 1 < chars.len() && chars[after_m + 1] == '钟' {
        return None;
    }
    // M 必须是 1/2/3
    let m: u32 = _m_str.parse().ok()?;
    if m == 0 || m > 3 {
        return None;
    }
    Some(RemainderSuffix {
        main_num: main_num.to_string(),
        unit_word: "点".to_string(),
        real_unit: None,
        remainder: RemainderKind::Quarter(m),
        consumed: num_consumed_inner + 1 + m_consumed + 1,
    })
}

/// 甲型 formatter：按单位族分派渲染（DEC-037）。
fn format_remainder_suffix(rs: &RemainderSuffix) -> String {
    let unit = rs.real_unit.as_ref().unwrap_or(&rs.unit_word);
    let is_time = rs.real_unit.is_none() && rs.unit_word == "点";

    if is_time {
        // 时间族 → H:MM
        let h: u32 = rs.main_num.parse().unwrap_or(0);
        let mm = match rs.remainder {
            RemainderKind::Half => 30,
            RemainderKind::Quarter(n) => (n * 15) % 60,
        };
        if mm == 0 {
            format!("{}:00", h)
        } else {
            format!("{}:{}", h, mm)
        }
    } else {
        // 度量衡 / 量词穿透 → N.5单位
        format!("{}.5{}", rs.main_num, unit)
    }
}

/// ITN-V2-004 (P4 任务C) 扫描「数字+跟随字符」链的终点。
/// 链 = 连续的「中文数字 + 单位/date_suffix/classifier」序列。
/// 终止于：非数字非单位非量词字符，或标点/空白（链终止符）。
fn scan_chain_end(chars: &[char], start: usize, r: &CompiledRules) -> usize {
    let mut pos = start;
    loop {
        // 数字段
        if pos >= chars.len()
            || !(is_cn_num_char(chars[pos]) || chars[pos] == '零' || chars[pos] == '〇')
        {
            break;
        }
        let (_, num_consumed) = match parse_cn_number(&chars[pos..], 0, Some(r)) {
            Some(v) => v,
            None => break,
        };
        if num_consumed == 0 {
            break;
        }
        pos += num_consumed;
        if pos >= chars.len() {
            break;
        }
        // 跟随字符：单位/date_suffix/classifier？
        let rest: String = chars[pos..].iter().collect();
        if r.is_unit(&rest)
            || r.is_date_suffix(&rest)
            || r.classifier_set
                .iter()
                .any(|c| rest.starts_with(c.as_str()))
        {
            let unit_len = r
                .match_unit_word(chars, pos)
                .map(|(l, _)| l)
                .or_else(|| r.match_date_suffix_len(&rest))
                .or_else(|| {
                    r.classifier_set
                        .iter()
                        .find(|c| rest.starts_with(c.as_str()))
                        .map(|c| c.chars().count())
                })
                .unwrap_or(0);
            if unit_len == 0 {
                break;
            }
            pos += unit_len;
        } else {
            break;
        }
        // 标点/空白 → 链终止
        if pos < chars.len() && is_boundary_char(chars[pos]) {
            break;
        }
    }
    pos
}

/// ITN-V2-004 (P4 任务C) 全或无检查：链中所有数字段是否一致（全转或全不转）。
/// 返回 true = 一致（可继续逐字路径）；false = 混合（整段原样输出）。
fn check_chain_consistency(chars: &[char], start: usize, r: &CompiledRules) -> bool {
    let mut pos = start;
    let mut any_convert = false;
    let mut any_not_convert = false;

    loop {
        if pos >= chars.len()
            || !(is_cn_num_char(chars[pos]) || chars[pos] == '零' || chars[pos] == '〇')
        {
            break;
        }
        let (num_str, num_consumed) = match parse_cn_number(&chars[pos..], 0, Some(r)) {
            Some(v) => v,
            None => break,
        };
        if num_consumed == 0 {
            break;
        }
        let after_num = pos + num_consumed;
        let after_str: String = if after_num < chars.len() {
            chars[after_num..].iter().collect()
        } else {
            String::new()
        };
        let should =
            decide_conversion(&num_str, num_consumed, chars, pos, after_num, &after_str, r);
        if should {
            any_convert = true;
        } else {
            any_not_convert = true;
        }
        pos = after_num;
        if pos >= chars.len() {
            break;
        }
        // 跟随字符
        let rest: String = chars[pos..].iter().collect();
        if r.is_unit(&rest)
            || r.is_date_suffix(&rest)
            || r.classifier_set
                .iter()
                .any(|c| rest.starts_with(c.as_str()))
        {
            let unit_len = r
                .match_unit_word(chars, pos)
                .map(|(l, _)| l)
                .or_else(|| r.match_date_suffix_len(&rest))
                .or_else(|| {
                    r.classifier_set
                        .iter()
                        .find(|c| rest.starts_with(c.as_str()))
                        .map(|c| c.chars().count())
                })
                .unwrap_or(0);
            if unit_len == 0 {
                break;
            }
            pos += unit_len;
        } else {
            break;
        }
        if pos < chars.len() && is_boundary_char(chars[pos]) {
            break;
        }
    }

    !(any_convert && any_not_convert)
}

fn normalize_with_rules(text: &str, r: &CompiledRules) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        // 检查保护白名单（最长匹配优先）——在甲型文法之前。
        // 理由：保护词表（如「五一」）应优先于甲型文法，避免「五一点半」被甲型
        // 误转为「51:30」。移除的甲型词条（八点半等）不再命中保护，自然落入甲型。
        if let Some(skip) = check_protection(&chars, i, r) {
            // 输出受保护的原文
            for ch in &chars[i..i + skip] {
                result.push(*ch);
            }
            i += skip;
            continue;
        }
        // ITN-V2-003 (P3 甲型文法)：余数后缀（半/刻）匹配。
        if is_cn_num_char(chars[i]) || chars[i] == '零' || chars[i] == '〇' {
            if let Some(rs) = try_parse_remainder_suffix(&chars, i, r) {
                result.push_str(&format_remainder_suffix(&rs));
                i += rs.consumed;
                continue;
            }
        }
        // ITN-V2-004 (P4 乙型)：隐式小数位 N<可小数化单位>M（M后紧邻边界）。
        if is_cn_num_char(chars[i]) || chars[i] == '零' || chars[i] == '〇' {
            if let Some(id) = try_parse_implicit_decimal(&chars, i, r) {
                result.push_str(&format_implicit_decimal(&id, r));
                i += id.consumed;
                continue;
            }
        }
        // ITN-V2-004 (P4 丙型)：显式多级单位链（取代③，用break不return None）。
        // 货币归一(11.92元)/时间H:MM(3:20)/度量衡小数合并。
        if is_cn_num_char(chars[i]) || chars[i] == '零' || chars[i] == '〇' {
            if let Some(chain) = try_parse_unit_chain(&chars, i, r) {
                result.push_str(&format_unit_chain(&chain, r));
                i += chain.consumed;
                continue;
            }
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
            // ITN-V2-FIX-TIMEPREFIX-001：文法优先让位 —— 若数字位置能被甲/乙/丙型识别，
            // 只输出前缀并把游标交还主循环，让下一轮迭代正常走文法分支（如「下午四点三刻」→
            // 甲型刻模式产出 4:45）。否则时段词分支会抢先消费数字导致甲型被跳过。
            // 主控修正：必须用 get() 越界安全取字符。match_date_prefix 只做 starts_with，
            // 对前缀之后是否还有字符无任何要求 —— 文本恰好以时段词结尾（「改到明天下午」
            // 「那就晚上」）时 after == chars.len()，直接索引 chars[after] 会 panic。
            if chars
                .get(after)
                .is_some_and(|c| is_cn_num_char(*c) || *c == '零' || *c == '〇')
            {
                if try_parse_remainder_suffix(&chars, after, r).is_some()
                    || try_parse_implicit_decimal(&chars, after, r).is_some()
                    || try_parse_unit_chain(&chars, after, r).is_some()
                {
                    for ch in &chars[i..after] {
                        result.push(*ch);
                    }
                    i = after;
                    continue;
                }
            }
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
                        // ITN-V2-004 (P4 任务C) 全或无撕裂防护：
                        // Gavin 2026-07-31 指令「三年二班这种就直接不转」。
                        // 在逐字路径转换前，扫描整个「数字+跟随字符」链，若部分转部分不转→整段不转。
                        // 这是逐字路径的守门员（甲/乙/丙型已在前面处理，此处只管逐字路径）。
                        // 链定义：连续「中文数字+单位/date_suffix/classifier」序列，
                        // 终止于非数字非单位非量词字符（如的/有/班非单位）。
                        if !check_chain_consistency(&chars, i, r) {
                            // 链中混合 → 整段原样输出，游标跳过整个链
                            let chain_end = scan_chain_end(&chars, i, r);
                            for ch in &chars[i..chain_end] {
                                result.push(*ch);
                            }
                            i = chain_end;
                            continue;
                        }
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
    // ITN-FIX-CURRENCY-017 条件1：虚指数量短语（一两个/三两天/三两句话/三两个人）整体保汉字。
    // 置于最前：即使「两」已注册为重量单位（下方 is_real_unit 会命中），虚指语境也不转。
    if is_virtual_two_phrase(chars, start) {
        return false;
    }
    // 如果后面有真单位（排除 classifiers）→ 转
    // ITN-V2-006 (红2修复)：从 is_unit 改为 is_real_unit，使双隶属词（间/条/次/名/台/辆/句/篇）
    // 在逐字路径与甲型路径行为一致。`五间半`→`间`在 all_units+classifier_set →
    // is_real_unit 返回 false → 不转 → 保持汉字。符合 DEC-030「单字数字+通用量词保留汉字」。
    if r.is_real_unit(after_str) {
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
        // ITN-FIX-CURRENCY-017 条件1 后续数字护栏：`两三天` 的「三」在虚指短语内 → 不转。
        // 无此护栏时，unit_preceded 会因「两」已注册为重量单位而把「三」误转成 3（两3天）。
        // 仅命中 两+[一二三]+量词 结构，`六两五` 的「五」（∉一二三）不受影响，仍由逐字转 5。
        if start > 0 && chars[start - 1] == '两' && matches!(chars[start], '一' | '二' | '三') {
            let after_d: String = chars[start + 1..].iter().collect();
            const QUANT2: &[&str] = &["个", "只", "天", "句", "次", "回"];
            if QUANT2.iter().any(|q| after_d.starts_with(q)) {
                return false;
            }
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

/// ITN-FIX-CURRENCY-017 条件1 虚指护栏：近似数量短语整体保汉字。
/// 覆盖主控指定族：一两个人、两三个人、三两个人、一两个、两三天、三两天、三两句话。
/// 两种模式：
/// - A: `[一二三] 两 <量词>`（三两个人/三两天/三两句话/一两个）
/// - B: `两 [一二三] <量词>`（两三个人/两三天）
/// 量词集合与条件一致（个/只/天/句/次/回 等）。注意：`三两酒`（三+两+酒）不在集合内 →
/// 「酒」非量词 → 仍按重量转 3两酒，虚指护栏不误伤真实度量。
fn is_virtual_two_phrase(chars: &[char], start: usize) -> bool {
    if start + 2 > chars.len() {
        return false;
    }
    const QUANT: &[&str] = &["个", "只", "天", "句", "次", "回"];
    // 模式A: [一二三] 两 <量词>
    if matches!(chars[start], '一' | '二' | '三') && chars.get(start + 1) == Some(&'两') {
        let after_two: String = chars[start + 2..].iter().collect();
        if QUANT.iter().any(|q| after_two.starts_with(q)) {
            return true;
        }
    }
    // 模式B: 两 [一二三] <量词>（量词紧跟数字后，如 两三个人/两三天）
    if chars[start] == '两' && matches!(chars.get(start + 1), Some('一' | '二' | '三')) {
        let after_q: String = chars[start + 2..].iter().collect();
        if QUANT.iter().any(|q| after_q.starts_with(q)) {
            return true;
        }
    }
    false
}

/// 检查保护白名单，返回匹配长度（0=未匹配）
fn check_protection(chars: &[char], start: usize, r: &CompiledRules) -> Option<usize> {
    let rest: String = chars[start..].iter().collect();

    // 成语——ITN-V2-003：改确定性最长匹配（一致性收口，五个 set 统一语义）。
    // 盘点 45 条 0 前缀重叠，max() 与 find_map 今天结果相同，零风险；
    // 改动消除「靠注释传递纪律」的脆弱性（1877 行注释「无前缀冲突」已被 002 证伪）。
    let matched_idiom = r
        .idiom_set
        .iter()
        .filter(|idiom| rest.starts_with(idiom.as_str()))
        .map(|idiom| idiom.chars().count())
        .max();
    if let Some(skip) = matched_idiom {
        return Some(skip);
    }

    // 专有名词——ITN-V2-002：改确定性最长匹配（HashSet 迭代顺序不确定，find_map
    // 会导致 `十一月` 在 `十一` 与 `十一月` 之间随机命中，输出不确定）。
    // 盘点出 4 组前缀重叠：五一⊂五一广场、十一⊂{十一国庆,十一月,十一边形}。
    // max() 与遍历顺序无关 → 确定性；3 字条目 `十一月` 胜出 → 非纯数字词 →
    // 不触发①右邻否决 → 保护生效 → 稳定输出 `十一月`（白名单作者原意）。
    let matched_noun = r
        .proper_noun_set
        .iter()
        .filter(|noun| rest.starts_with(noun.as_str()))
        .map(|noun| noun.chars().count())
        .max();

    // ITN-V2-001 (R2 缺陷A ①右邻否决)：仅对含进位单位的纯数字词撤销保护。
    // 保护词若全由中文数字字符组成（如「十一」「五一」「八一」）且紧邻右侧是
    // 单位/date_suffix，则撤销保护——避免「十一块」前半被保护、后半照转的撕裂。
    // ③ 块级匹配已在主循环先行拦截「数字+单位+数字+单位」复合块；此处兜底
    // 处理未形成复合块的单段「保护数字+单位」（如「十一块」后无更多数字）。
    //
    // ⚠️ 逐位串兜底（主控约束）：`五一`/`七一`/`八一` 等纯逐位串（无进位单位十/百/
    // 千/万/亿）不撤销——它们在节日语境是名称而非数字（`五一点半`≠`51点半`）。
    // 仅当词含进位单位（`十一`/`十五`/`二十五`）才撤销，此时 `parse_cn_number`
    // 按进位组合解析（`十一`→`11`），非逐位串，语义安全。
    // 兜底：撤销后主循环走 parse_cn_number，若 decide_conversion 返回 false
    // 则字符原样输出，保护语义自然回退。
    if let Some(skip) = matched_noun {
        let word: String = chars[start..start + skip].iter().collect();
        let after = start + skip;
        if is_pure_cn_digit_word(&word) && word.chars().any(is_cn_unit_char) && after < chars.len()
        {
            let right: String = chars[after..].iter().collect();
            if r.is_unit(&right) || r.is_date_suffix(&right) {
                return None; // 撤销保护，交给主循环 parse_cn_number
            }
        }
        return Some(skip);
    }

    // ITN-SMART-002: 历史/文化/民俗词汇（"五代十国"等）
    // ITN-V2-002：改确定性最长匹配。盘点出 4 组前缀重叠：
    // 五代⊂五代十国、三十六计⊂三十六计走为上计、四海⊂四海八荒、三省⊂三省吾身。
    let matched_hist = r
        .historical_set
        .iter()
        .filter(|hist| rest.starts_with(hist.as_str()))
        .map(|hist| hist.chars().count())
        .max();
    if let Some(skip) = matched_hist {
        return Some(skip);
    }

    // 虚词"一"的搭配
    // ITN-V2-002：改确定性最长匹配。盘点出 1 组前缀重叠：一下⊂一下子。
    let matched_fw = r
        .function_word_set
        .iter()
        .filter(|fw| rest.starts_with(fw.as_str()))
        .map(|fw| fw.chars().count())
        .max();
    if let Some(skip) = matched_fw {
        return Some(skip);
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
        // TEST-SYNC-ITN-V2-007：断言更新。ITN-V2-FIX-TIMEPREFIX-001（时段词前缀
        // 不再抢先消费数字）落地后，「下午三点五十分」在时段词分支让位给丙型
        // try_parse_unit_chain 接管 → 产出 `下午3:50`（DEC-037 时间族归一为 H:MM
        // 通用书写形式）。旧值 `下午3点50分` 是时段词分支抢先消费产物，已过时。
        assert_eq!(normalize_test("下午三点五十分"), "下午3:50");
    }

    #[test]
    fn time_half() {
        // TEST-SYNC-ITN-V2-001 (A类)：旧断言 "8点半" 过时 —— ITN-V2-003 (P3 甲型) 上线后
        // 「八点半」走甲型半模式（时间族 H:MM），半=30分。CHANGELOG ENGINE-003/004 实证。
        assert_eq!(normalize_test("八点半"), "8:30");
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
        // TEST-SYNC-ITN-V2-001 (A类)：旧断言 "5块8" 过时 —— ITN-V2-004 (P4 乙型/丙型) +
        // DEC-037 货币归一上线后，「五块八」走乙型隐式小数，块→元归一。ENGINE-004 实证。
        assert_eq!(normalize_test("五块八"), "5.8元");
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

    /// P1-3: 匹配顺序隐患护栏 —— ITN-V2-ENGINE-002 根治后由「注释说明」升级为真断言。
    ///
    /// 旧注释声称「无互相为前缀的条目冲突」，已被 coder-1 盘点证伪（4 组前缀重叠：
    /// 五一⊂五一广场、十一⊂{十一国庆,十一月,十一边形}；historical 另有 4 组、
    /// function_words 有一下⊂一下子）。ENGINE-002 将三个有重叠的 set 从 find_map
    /// （HashSet RandomState 迭代顺序不定 → 输出不确定）改为 filter+max()
    /// （确定性最长匹配），idioms/classifiers 无重叠保持原样。
    ///
    /// 本测试锁定确定性语义：最长匹配必先命中（十一月=3 字而非 十一=2 字），
    /// 且多次独立调用结果恒定 —— test 通过即证明 filter+max() 与遍历顺序无关。
    #[test]
    fn geometric_order_hazard_documented() {
        let r = rules();
        // 十一月：3 字条目（十一月）胜出，不得退化为 2 字（十一）
        let chars: Vec<char> = "十一月".chars().collect();
        for _ in 0..5 {
            assert_eq!(
                check_protection(&chars, 0, r),
                Some(3),
                "十一月 必须命中最长匹配 3 字（ENGINE-002 filter+max() 确定性）"
            );
        }
        // 十一：孤立出现命中 2 字
        let chars: Vec<char> = "十一".chars().collect();
        assert_eq!(check_protection(&chars, 0, r), Some(2));
        // ①右邻否决：十一（纯数字进位词）+ 块（单位）→ 撤销保护交给主循环
        let chars: Vec<char> = "十一块".chars().collect();
        assert_eq!(check_protection(&chars, 0, r), None);
        // 五一广场：4 字条目胜出；五一（逐位串无进位）孤立命中 2 字且不否决
        let chars: Vec<char> = "五一广场".chars().collect();
        assert_eq!(check_protection(&chars, 0, r), Some(4));
        let chars: Vec<char> = "五一".chars().collect();
        assert_eq!(check_protection(&chars, 0, r), Some(2));
        // 端到端确定性（P2）：十一月 多次独立归一结果恒定
        let first = normalize_test("十一月");
        assert_eq!(first, "十一月");
        for _ in 0..5 {
            assert_eq!(normalize_test("十一月"), first);
        }
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

    // ============================================================
    // TEST-SYNC-ITN-V2-001 (D类) ITN-V2 双通道 + 甲/乙/丙型 + 全或无 + 地名
    // ============================================================

    // ------------------------------------------------------------
    // P1 · DEC-036 双通道：主通道（normalize_numbers，LLM 前）+
    // 补丁通道（normalize_unit_symbols_only，LLM 后）
    // 三条路径（LLM 成功/运行时失败兜底/LLM 关闭）在 main.rs run_pipeline
    // 内联实现、无单测接缝（生产代码零改动约束下无法拆分），
    // 此处锁定可单测的 ITN 侧契约（见 result.md 覆盖缺口清单）。
    // ------------------------------------------------------------

    #[test]
    fn itn_v2_dual_channel_main_normalizes_all() {
        // 主通道（LLM 之前）：中文数字→阿拉伯 + 单位符号规整
        assert_eq!(normalize_numbers("四十摄氏度"), "40℃");
        assert_eq!(normalize_numbers("八点半"), "8:30");
    }

    #[test]
    fn itn_v2_dual_channel_patch_catches_unit_symbols_only() {
        // 补丁通道（LLM 之后）：只做单位符号规整，捞回 LLM 纠正同音错字后的「40摄氏度」
        assert_eq!(normalize_unit_symbols_only("40摄氏度"), "40℃");
        // 不重跑中文数字→阿拉伯转换（第一阶段）
        assert_eq!(normalize_unit_symbols_only("八点半"), "八点半");
        // 翻译路径英文输出：no-op 逐字节不变
        assert_eq!(
            normalize_unit_symbols_only("The temperature is 40 degrees."),
            "The temperature is 40 degrees."
        );
    }

    #[test]
    fn itn_v2_dual_channel_patch_idempotent_after_main() {
        // 幂等性（DEC-036）：主通道产出「40℃」后，补丁通道重跑逐字节不变
        let main_out = normalize_numbers("四十摄氏度");
        assert_eq!(main_out, "40℃");
        let patch_out = normalize_unit_symbols_only(&main_out);
        assert_eq!(
            patch_out, main_out,
            "补丁通道对主通道产出的 ℃ 必须逐字节不变"
        );
    }

    // ------------------------------------------------------------
    // P2 · 撕裂修复 + 确定性
    // ------------------------------------------------------------

    #[test]
    fn itn_v2_p2_shiyikuai_jiumaoer_normalized() {
        // 十一块九毛二 → 11.92元（P4 丙型货币归一后的 P2 验收形态）
        assert_eq!(normalize_test("十一块九毛二"), "11.92元");
    }

    #[test]
    fn itn_v2_p2_shiyiyue_deterministic() {
        // 十一月 多次独立运行输出恒定（ENGINE-002 filter+max() 确定性）
        let first = normalize_test("十一月");
        assert_eq!(first, "十一月");
        for _ in 0..5 {
            assert_eq!(normalize_test("十一月"), first);
        }
    }

    #[test]
    fn itn_v2_p2_wuyi_yiban_preserved() {
        // 五一点半 保持（①右邻否决的逐位串兜底：五一 无进位单位不撤销）
        assert_eq!(normalize_test("五一点半"), "五一点半");
    }

    // ------------------------------------------------------------
    // P3 · 甲型文法（半/刻）
    // ------------------------------------------------------------

    #[test]
    fn itn_v2_p3_jia_time_half() {
        // 时间族：N点半 → H:MM（半=30 分）
        assert_eq!(normalize_test("四点半"), "4:30");
        assert_eq!(normalize_test("八点半"), "8:30");
        assert_eq!(normalize_test("一点半"), "1:30");
        assert_eq!(normalize_test("两点半"), "2:30");
        assert_eq!(normalize_test("十二点半"), "12:30");
    }

    #[test]
    fn itn_v2_p3_jia_quarter_mode() {
        // 刻模式：N点M刻（仅时间，M∈{1,2,3}）
        assert_eq!(normalize_test("五点三刻"), "5:45");
    }

    #[test]
    fn itn_v2_p3_jia_measure_half() {
        // 度量衡：N<真单位>半 → N.5单位
        assert_eq!(normalize_test("一吨半"), "1.5吨");
        assert_eq!(normalize_test("三寸半"), "3.5寸");
    }

    #[test]
    fn itn_v2_p3_jia_classifier_passthrough() {
        // 量词穿透：N<量词>半<真单位>
        assert_eq!(normalize_test("一个半小时"), "1.5小时");
    }

    #[test]
    fn itn_v2_p3_jia_counterexamples() {
        // 反例护栏：一刻钟（刻后跟钟否决）保持；三点五走既有小数路径；半小时/半个小时非数字开头
        assert_eq!(normalize_test("一刻钟"), "一刻钟");
        assert_eq!(normalize_test("三点五"), "3.5");
        assert_eq!(normalize_test("半小时"), "半小时");
        assert_eq!(normalize_test("半个小时"), "半个小时");
    }

    #[test]
    fn itn_v2_p3_jia_classifier_guard() {
        // 守卫（DEC-038）：通用量词不算单位 → 一个半 / 五间半 保持汉字
        assert_eq!(normalize_test("一个半"), "一个半");
        assert_eq!(normalize_test("五间半"), "五间半");
    }

    #[test]
    fn itn_v2_p3_units_time_scope_expansion() {
        // ⚠️ 未经请求的范围扩张专项锁定：新增 [units.time]（小时/分钟）导致
        // 「三小时」→「3小时」、「五分钟」→「5分钟」。输出正确但非 Gavin 需求，
        // 必须独立测试锁定，不得混在甲型测试里带过。
        assert_eq!(normalize_test("三小时"), "3小时");
        assert_eq!(normalize_test("五分钟"), "5分钟");
    }

    // ------------------------------------------------------------
    // P4 · 乙/丙型 + 层级表 + 全或无
    // ------------------------------------------------------------

    #[test]
    fn itn_v2_p4_yi_implicit_decimal() {
        // 乙型：N<可小数化单位>M（M 后紧邻边界）
        assert_eq!(normalize_test("一米二"), "1.2米");
        assert_eq!(normalize_test("一米八五"), "1.85米");
        assert_eq!(normalize_test("三十九度八"), "39.8度");
        assert_eq!(normalize_test("一百零八度五"), "108.5度");
    }

    #[test]
    fn itn_v2_p4_yi_boundary_guard() {
        // 乙型边界护栏：尾数后不紧邻边界 → 不走乙型
        assert_eq!(normalize_test("三年二班"), "三年二班");
        assert_eq!(normalize_test("三楼二号"), "3楼2号"); // 楼/号不可小数化 → 逐字路径全转
        assert_eq!(normalize_test("五排八座"), "五排八座");
    }

    #[test]
    fn itn_v2_p4_bing_unit_chain() {
        // 丙型：显式多级单位链 + 货币归一（DEC-037）
        assert_eq!(normalize_test("十一块九毛二"), "11.92元");
        assert_eq!(normalize_test("五块八"), "5.8元");
        assert_eq!(normalize_test("三小时二十分"), "3:20");
    }

    #[test]
    fn itn_v2_p4_bare_unit_not_normalized() {
        // 裸单位不归一：单级链（parts=1）不进丙型 → 逐字路径只转数字
        assert_eq!(normalize_test("五块钱"), "5块钱");
        assert_eq!(normalize_test("十一块"), "11块");
    }

    #[test]
    fn itn_v2_p4_fen_disambiguation() {
        // 🔴 分 族属消歧：前驱块→货币分(0.01元)；前驱点→时间分(分钟)；裸 N分 不合并
        assert_eq!(normalize_test("五块八毛三分"), "5.83元");
        assert_eq!(normalize_test("三点二十分"), "3:20");
        assert_eq!(normalize_test("三分"), "3分");
    }

    #[test]
    fn itn_v2_p4_all_or_nothing_continuity() {
        // 全或无连续性边界（DEC-037 附则）：三年二班 整段保持，后文 五个人 不被连坐
        assert_eq!(
            normalize_test("三年二班的学生有五个人"),
            "三年二班的学生有五个人"
        );
    }

    // ------------------------------------------------------------
    // P5 · 地名白名单（≥3 字）
    // ------------------------------------------------------------

    #[test]
    fn itn_v2_p5_place_names_protected() {
        // ≥3 字含数字地名命中保护（ENGINE-005，proper_nouns 69→129）
        assert_eq!(normalize_test("十三陵"), "十三陵");
        assert_eq!(normalize_test("九寨沟"), "九寨沟");
        assert_eq!(normalize_test("五道口"), "五道口");
        assert_eq!(normalize_test("去三门峡旅游"), "去三门峡旅游");
    }

    #[test]
    fn itn_v2_p5_place_reverse_guard() {
        // 反向护栏：<数字前缀>+单位 正常表达不得失效（十三陵 与 十三块钱 同前缀共存）
        assert_eq!(normalize_test("十三块钱"), "13块钱");
        assert_eq!(normalize_test("去十三陵玩花十三块钱"), "去十三陵玩花13块钱");
    }

    // ============================================================
    // TEST-SYNC-ITN-V2-006 · ENGINE-006 + LEXICON-006-C 测试同步
    // ============================================================
    // 覆盖对象：
    //  - ENGINE-006 (b462f83)：decide_conversion 判据 is_unit → is_real_unit，
    //    双隶属量词（间/条/次/名/台/辆/句/篇）后单字数字保持汉字（DEC-030）。
    //  - LEXICON-006-C (05de1bc)：移除 5 条 2 字遮蔽词条（三元/九度/二分/五类/四大）
    //    + N分钟 家族 7 条，闭合红1（二分钟→2分钟）。
    // 期望值全部来自 coder-1 真实 cargo test 实测，非推断。

    // T1 · 双隶属量词保持汉字（ENGINE-006 正向）
    // 判定路径：decide_conversion → is_real_unit(after_str)=false（词在 all_units 且
    // 在 classifier_set）→ is_date_suffix=false → consumed<2 → classifier 分支 return false
    #[test]
    fn itn_v2_006_t1_dual_classifier_single_digit_preserved() {
        assert_eq!(normalize_test("三条"), "三条");
        assert_eq!(normalize_test("五台"), "五台");
        assert_eq!(normalize_test("两辆"), "两辆");
        assert_eq!(normalize_test("三次"), "三次");
        assert_eq!(normalize_test("五名"), "五名");
        assert_eq!(normalize_test("两句"), "两句");
        assert_eq!(normalize_test("三篇"), "三篇");
        // 甲型路径守卫（P3 既有覆盖，此处作为 T1 完整组保留）
        assert_eq!(normalize_test("五间半"), "五间半");
        assert_eq!(normalize_test("一个半"), "一个半");
    }

    // T2 · 真单位仍转换（ENGINE-006 反向护栏）
    // 判定路径：is_real_unit=true（词在 all_units 且不在 classifier_set）→ 转
    #[test]
    fn itn_v2_006_t2_real_unit_still_converts() {
        assert_eq!(normalize_test("五块"), "5块");
        assert_eq!(normalize_test("三度"), "3度");
        assert_eq!(normalize_test("五米"), "5米");
        assert_eq!(normalize_test("三小时"), "3小时");
        assert_eq!(normalize_test("十三块钱"), "13块钱");
    }

    // T3 · ⭐ 多位数不受判据收紧影响（最重要的护栏，consumed>=2 分支零覆盖→补齐）
    // 判定路径：is_real_unit=false（双隶属量词）→ is_date_suffix=false →
    // consumed>=2 return true（src/itn.rs:1810，在 is_real_unit 之后）→ 转
    // 这证明 is_real_unit 收紧只影响单字数字，不误伤多位数表达。
    #[test]
    fn itn_v2_006_t3_multi_digit_unaffected_by_guard() {
        assert_eq!(normalize_test("三十五台"), "35台");
        assert_eq!(normalize_test("二十三条"), "23条");
        assert_eq!(normalize_test("一百二十次"), "120次");
        // 补充组合：多位数 + 双隶属量词「间」（units.other + classifiers 双隶属）
        assert_eq!(normalize_test("二十五间"), "25间");
    }

    // T4 · N分钟 家族一致性（ENGINE-006 + LEXICON-006-C 联合）
    // 红1 闭合判定项：二分钟→2分钟。一/七/两/五/八/六/四分钟 7 条从
    // unit_collisions 移除（b462f83），二/三/九/十分钟本就不在表内。
    // 移除后 N分钟 走逐字路径：分钟 ∈ units.time 且 ∉ classifier_set →
    // is_real_unit=true → 转。五分钟 已由 itn_v2_p3_units_time_scope_expansion 覆盖，不重复。
    #[test]
    fn itn_v2_006_t4_minute_family_consistent() {
        // 红1 闭合判定项（单独成条）
        assert_eq!(normalize_test("二分钟"), "2分钟");
        // 其余 9 条家族一致性
        assert_eq!(normalize_test("一分钟"), "1分钟");
        assert_eq!(normalize_test("三分钟"), "3分钟");
        assert_eq!(normalize_test("四分钟"), "4分钟");
        assert_eq!(normalize_test("六分钟"), "6分钟");
        assert_eq!(normalize_test("七分钟"), "7分钟");
        assert_eq!(normalize_test("八分钟"), "8分钟");
        assert_eq!(normalize_test("九分钟"), "9分钟");
        assert_eq!(normalize_test("十分钟"), "10分钟");
        assert_eq!(normalize_test("两分钟"), "2分钟");
    }

    // T5 · 5 条 2 字词删除后的正向恢复（LEXICON-006-C）
    // 三元/九度/二分/五类/四大 移除后各自遮蔽的能产族恢复转换；
    // 五类人/四大件 因「类」「大」非单位仍保持汉字。
    #[test]
    fn itn_v2_006_t5_two_char_word_removal_recovery() {
        // 红1（同 T4，此处重复锁定，LEXICON-006-C 移除点）
        assert_eq!(normalize_test("二分钟"), "2分钟");
        assert_eq!(normalize_test("三元钱"), "3元钱");
        assert_eq!(normalize_test("九度电"), "9度电");
        // 反向：非单位后缀保持汉字
        assert_eq!(normalize_test("五类人"), "五类人");
        assert_eq!(normalize_test("四大件"), "四大件");
    }

    // T6 · ⭐ 更长同前缀词条仍受保护（LEXICON-006-C 反向护栏，13 条）
    // 锁「确定性最长匹配」（ENGINE-002 filter+max）架构假设：
    // 移除 2 字遮蔽词条后，同前缀更长条目（二分* 26/三元* 17/四大* 17 等）
    // 原样保留、仍受保护。将来若有人改动 check_protection，这组是第一道警报。
    #[test]
    fn itn_v2_006_t6_longer_same_prefix_still_protected() {
        assert_eq!(normalize_test("二分查找"), "二分查找");
        assert_eq!(normalize_test("二分图"), "二分图");
        assert_eq!(normalize_test("二分法"), "二分法");
        assert_eq!(normalize_test("二分之一"), "二分之一");
        assert_eq!(normalize_test("二分音符"), "二分音符");
        assert_eq!(normalize_test("三元催化"), "三元催化");
        assert_eq!(normalize_test("三元及第"), "三元及第");
        assert_eq!(normalize_test("三元桥"), "三元桥");
        assert_eq!(normalize_test("四大发明"), "四大发明");
        assert_eq!(normalize_test("四大皆空"), "四大皆空");
        assert_eq!(normalize_test("四大名捕"), "四大名捕");
        assert_eq!(normalize_test("九度OJ"), "九度OJ");
        assert_eq!(normalize_test("五类分子"), "五类分子");
    }

    // ============================================================
    // TEST-SYNC-ITN-V2-007 · 时段词前缀修复（ITN-V2-FIX-TIMEPREFIX-001）
    // ============================================================
    // 根因：主循环时段词分支（:1509）在甲型（:1425）之后，但甲型在时段词首字
    // 位置匹配不上（不是数字），游标落到时段词分支后被整体消费「前缀+数字」并
    // 跳到时间后缀 →「四点三刻」再无机会进入甲型 → 输出 `4点3刻` 而非 `4:45`。
    // 修复：时段词分支匹配前缀后、消费数字前，先在数字位置试甲/乙/丙型，命中
    // 则只输出前缀、游标交还主循环，复用无前缀时的同一条处理路径。
    //
    // ⚠️ 本轮规格要求（TEST-SYNC-ITN-V2-007 §二）：用例必须带真实语流上下文
    // （时段词前缀 / 前后文 / 句中位置），裸串断言只能作为补充。这是本项目第二例
    // 「测试全绿但生产不生效」—— 旧用例 `itn_v2_p3_jia_quarter_mode` 断言裸串
    // `五点三刻`→`5:45`，真实语流 `下午四点三刻在...` 却被时段词分支抢先消费。
    // 故本组 T7/T8/T9 全部带前缀/上下文，仅 T10 保留裸串作反向护栏补充。
    //
    // 期望值来源：T7/T8/T10 锚点来自 coder-1 真机 `cargo test` 实测；T9 边界用例为
    // 不变式保持（前缀后无数字，任何路径都不可能触发转换），由代码走查验证，非推断。

    // T7 · 时段词 × 刻模式（修复的核心命中场景）
    #[test]
    fn itn_v2_007_t7_period_quarter_mode() {
        assert_eq!(normalize_test("下午四点三刻见面"), "下午4:45见面");
        // ⭐ Gavin 原句：`八里庄` 必须仍受保护（itn-rules.toml:485）
        assert_eq!(
            normalize_test("每天下午四点三刻在八里庄见面"),
            "每天下午4:45在八里庄见面"
        );
        assert_eq!(normalize_test("晚上七点三刻"), "晚上7:45");
    }

    // T8 · ⭐ 时段词 × 半模式（7 个时段词全覆盖，爆炸半径的另一半）
    // 上午/下午/凌晨/晚上/中午/傍晚/清晨 一个都不能少 —— 少测一个就等于给那个
    // 词留一条无人看守的路径。
    #[test]
    fn itn_v2_007_t8_period_half_mode_all_7() {
        assert_eq!(normalize_test("上午八点半"), "上午8:30");
        assert_eq!(normalize_test("下午四点半"), "下午4:30");
        assert_eq!(normalize_test("凌晨两点半"), "凌晨2:30");
        assert_eq!(normalize_test("晚上七点半"), "晚上7:30");
        assert_eq!(normalize_test("中午十二点半"), "中午12:30");
        assert_eq!(normalize_test("傍晚六点半"), "傍晚6:30");
        assert_eq!(normalize_test("清晨五点半"), "清晨5:30");
    }

    // T9 · ⭐⭐ 边界护栏：文本以时段词结尾（主控在验收中拦下的 panic）
    // match_date_prefix 只做 starts_with，对前缀之后是否还有字符无任何要求。
    // 文本恰好以时段词结尾时 after == chars.len()，守卫若用 chars[after] 直接
    // 索引会 panic（index out of bounds）。修复已改用 chars.get(after).is_some_and。
    // 成因与本批修的 bug 同源 —— 既有 124 条用例没有任何一条以时段词结尾，
    // 本组写为显式边界护栏锁死，防止将来有人重构回 chars[after] 直索引。
    #[test]
    fn itn_v2_007_t9_period_word_at_end_boundary() {
        assert_eq!(normalize_test("改到明天下午"), "改到明天下午");
        assert_eq!(normalize_test("那就晚上"), "那就晚上");
        // 最极端情形：整串就是一个时段词
        assert_eq!(normalize_test("凌晨"), "凌晨");
        assert_eq!(normalize_test("上午"), "上午");
        assert_eq!(normalize_test("中午"), "中午");
        assert_eq!(normalize_test("傍晚"), "傍晚");
        assert_eq!(normalize_test("清晨"), "清晨");
    }

    // T10 · 反向护栏（既有行为不得回归）
    // 正向/负向路径 + 裸串补充（裸串已有 P3 覆盖，此处仅作本批回归锚点）。
    #[test]
    fn itn_v2_007_t10_period_regression_guards() {
        // 时段词正向路径不变：前缀后数字+时间后缀仍转
        assert_eq!(normalize_test("下午四点"), "下午4点");
        // 负向路径不变：前缀后非时间后缀 → 不转
        assert_eq!(normalize_test("下午三个人"), "下午三个人");
        // 裸串仍过（补充形态，P3 已有裸串覆盖）
        assert_eq!(normalize_test("五点三刻"), "5:45");
        assert_eq!(normalize_test("四点半"), "4:30");
        // `刻`+`钟` 否决仍生效（一刻钟=时长，不转）—— 命中保护表 itn-rules.toml:420
        assert_eq!(normalize_test("一刻钟"), "一刻钟");
    }

    // ============================================================
    // TEST-SYNC-016 · ITN-FIX-GRADECLASS-016 年级班级简写守卫
    // ============================================================
    // 覆盖对象（2d7703d）：新增 [protect.serial_suffixes] = ["班"]。
    // 机制：serial_len == 2（「一三/五一/二三/一四」两位逐位串）且紧随其后命中
    // serial_suffixes 时，parse_cn_number 直接 return None → 主循环走单字路径，
    // 「班」既非单位也非量词 → 整串保持汉字。限定 serial_len == 2 使进位组合
    // 路径（十三班）、≥3 位逐位串（三零二房间/二零二六/幺三八零零）不受影响。
    //
    // 期望值来源：code 走查 + 既有断言锚点（九八年/三零二房间/二零二六/幺三八零零
    // 沿用既有断言；十三班为 code 走查判断——十非逐位串首字符，走进位组合路径，
    // 不受本守卫影响，行为与修前一致，非凭猜）。

    // T1 · 正向：目标 4 条全汉字（Gavin 端测原话用例）
    #[test]
    fn itn_v2_016_t1_grade_class_serial_preserved() {
        assert_eq!(normalize_test("一三班"), "一三班");
        assert_eq!(normalize_test("五一班"), "五一班");
        assert_eq!(normalize_test("初二三班"), "初二三班");
        assert_eq!(normalize_test("高一四班"), "高一四班");
    }

    // T2 · 正向句子形态：整句无阿拉伯数字（Gavin 原句）
    #[test]
    fn itn_v2_016_t2_sentence_form_no_digits() {
        assert_eq!(normalize_test("我是一三班的学生"), "我是一三班的学生");
        assert_eq!(normalize_test("他在五一班上课"), "他在五一班上课");
    }

    // T3 · 反向护栏：既有行为必须保持不变
    #[test]
    fn itn_v2_016_t3_reverse_guards() {
        // 九八年 → 98年（serial_len=2 但「年」不在 serial_suffixes，仍逐位合并）
        assert_eq!(normalize_test("九八年"), "98年");
        // ≥3 位逐位串不进守卫
        assert_eq!(normalize_test("三零二房间"), "302房间");
        assert_eq!(normalize_test("二零二六"), "2026");
        assert_eq!(normalize_test("幺三八零零"), "13800");
        // 全或无路径（三年二班）与本守卫互不干扰
        assert_eq!(normalize_test("三年二班"), "三年二班");
        // 十三班：十非逐位串首字符（走进位组合路径），守卫不触发 → 仍转（修前行为）
        assert_eq!(normalize_test("十三班"), "13班");
        // 单字+班：serial_len=1，不进守卫，单字数字无单位语境不转
        assert_eq!(normalize_test("一班"), "一班");
        assert_eq!(normalize_test("三班"), "三班");
    }

    // T4 · proper_nouns 保护不受影响（五一/五一广场 走保护路径，与守卫无关）
    #[test]
    fn itn_v2_016_t4_proper_noun_protection_intact() {
        assert_eq!(normalize_test("五一广场"), "五一广场");
        assert_eq!(normalize_test("五一"), "五一");
        assert_eq!(normalize_test("五一放假"), "五一放假");
    }

    // T5 · 边界：班出现在非数字后不受影响
    #[test]
    fn itn_v2_016_t5_ban_not_after_digit_untouched() {
        assert_eq!(normalize_test("上班"), "上班");
        assert_eq!(normalize_test("班车"), "班车");
        assert_eq!(normalize_test("今天上班坐班车"), "今天上班坐班车");
    }

    // T6 · ⭐ 降级测试：serial_suffixes 缺失（serde default → 空集）行为回到修前。
    // 锁死 [TOML-STALE-001] 的失效形态——外置旧 toml（无 [protect.serial_suffixes]
    // 段）会让本修复静默失效（「一三班」→「13班」），有断言才能在回归时看见。
    #[test]
    fn itn_v2_016_t6_downgrade_without_serial_suffixes() {
        // 构造一个「旧 toml」：仅含 proper_nouns 等既有分组，缺 serial_suffixes 段
        let old_rules = r#"
[switches]
[units.other]
words = ["岁", "楼", "层", "号", "房间", "倍", "页", "章", "节", "条", "款", "名", "次", "台", "辆", "间", "句", "篇"]
[units.time]
words = ["小时", "分钟"]
[units.currency]
words = ["元", "块", "角", "毛", "分"]
[protect.proper_nouns]
words = ["五一", "五一广场"]
[protect.classifiers]
words = ["个", "件", "位", "名", "次", "只", "条", "张", "份", "台", "辆", "间", "句", "篇", "本", "部", "场", "组", "批", "种", "类", "段", "堆", "瓶", "盒", "包", "箱"]
"#;
        // 缺 serial_suffixes → 空集 → 「一三班」退回逐位合并（修前行为 = 错误行为）
        assert_eq!(normalize_with(old_rules, "一三班"), "13班");
        // 反向对照：proper_nouns 保护不受缺段影响（五一/五一广场 仍保汉字）
        assert_eq!(normalize_with(old_rules, "五一广场"), "五一广场");
        // 反向对照：≥3 位逐位串在缺段时仍正常合并
        assert_eq!(normalize_with(old_rules, "三零二房间"), "302房间");
    }

    // ============================================================
    // ITN-FIX-CURRENCY-017 (P0) 货币/度量链数值算错（Gavin 6 条端测）
    // 根因：
    //   RC-A `一斤二两`→1.22斤 / `三斤六两五`→3.625斤 —— 乙型尾数扫描把「两」（兼数字2）
    //   吞进小数尾；丙型 weight 链又走通用小数合成。
    //   RC-B `一块两毛二一斤`→22.20元 —— 丙型不查族一致性，把 weight 单位拼进 currency 链。
    // 方案（主控 2026-08-03 拍板候选甲 + 三条件）：两 消歧为重量单位、丙型族一致性终止 +
    // per_unit 捕获、weight 链零乘法拼接、虚指短语护栏。
    // ============================================================

    // T1 · 六条端测逐条修复（Gavin 原句）
    #[test]
    fn itn_v2_017_t1_six_bugs_fixed() {
        assert_eq!(normalize_test("一斤二两"), "1斤2两");
        assert_eq!(normalize_test("这个西瓜是一块两毛二一斤"), "这个西瓜是1.22元一斤");
        assert_eq!(normalize_test("一块两毛二一斤"), "1.22元一斤");
        assert_eq!(normalize_test("这个水果是三块四毛八一斤"), "这个水果是3.48元一斤");
        // ITN-FIX-CHAIN-TEAR-026 条件B：尾零去除（一块八毛一斤 → 1.8元一斤，不是 1.80）
        assert_eq!(normalize_test("这个西瓜是一块八毛一斤"), "这个西瓜是1.8元一斤");
        assert_eq!(normalize_test("这个西瓜是一块八一斤"), "这个西瓜是1.8元一斤");
        assert_eq!(normalize_test("这个重量是三斤六两五"), "这个重量是3斤6两5");
    }

    // T2 · 条件2 锁死断言：斤/两 的 hierarchy value 不参与计算（死数据）。
    // 一斤二两 必须等于 1斤2两，且显式断言 != 1.2斤（通用小数 formatter 误接会静默变 1.2斤）。
    #[test]
    fn itn_v2_017_t2_weight_lock_dead_data() {
        assert_eq!(normalize_test("一斤二两"), "1斤2两");
        assert_ne!(normalize_test("一斤二两"), "1.2斤");
        assert_ne!(normalize_test("一斤二两"), "1.22斤");
        assert_eq!(normalize_test("三斤六两五"), "3斤6两5");
        assert_ne!(normalize_test("三斤六两五"), "3.625斤");
    }

    // T3 · 两 消歧为重量单位：裸 N两 / 重量语境正常转
    #[test]
    fn itn_v2_017_t3_liang_as_weight_unit() {
        assert_eq!(normalize_test("二两"), "2两");
        assert_eq!(normalize_test("三两"), "3两");
        assert_eq!(normalize_test("五两"), "5两");
        assert_eq!(normalize_test("二十五两"), "25两");
        assert_eq!(normalize_test("三两银子"), "3两银子");
        assert_eq!(normalize_test("一斤二两半"), "1斤2两半");
    }

    // T4 · 条件1 虚指护栏：近似数量短语整体保汉字（一两个人/两三个人/三两天/三两句话…）
    #[test]
    fn itn_v2_017_t4_virtual_two_phrase_guards() {
        assert_eq!(normalize_test("一两个人"), "一两个人");
        assert_eq!(normalize_test("两三个人"), "两三个人");
        assert_eq!(normalize_test("三两个人"), "三两个人");
        assert_eq!(normalize_test("一两个"), "一两个");
        assert_eq!(normalize_test("两三天"), "两三天");
        assert_eq!(normalize_test("三两天"), "三两天");
        assert_eq!(normalize_test("三两句话"), "三两句话");
        // 反向护栏：量词集合外（酒）不误伤 → 仍按重量转
        assert_eq!(normalize_test("三两酒"), "3两酒");
    }

    // T5 · 既有行为必须保持不变（反向护栏）
    #[test]
    fn itn_v2_017_t5_reverse_guards() {
        assert_eq!(normalize_test("两个人"), "两个人");
        assert_eq!(normalize_test("两本书"), "两本书");
        assert_eq!(normalize_test("两块二"), "2.2元");
        assert_eq!(normalize_test("五块八"), "5.8元");
        assert_eq!(normalize_test("一块八毛五"), "1.85元");
        assert_eq!(normalize_test("十一块九毛二"), "11.92元");
        assert_eq!(normalize_test("一米二"), "1.2米");
        assert_eq!(normalize_test("半斤八两"), "半斤八两");
        assert_eq!(normalize_test("四点半"), "4:30");
        assert_eq!(normalize_test("一个半小时"), "1.5小时");
    }

    // ============================================================
    // ITN-FIX-CHAIN-TEAR-026 新增测试（2026-08-03）
    // ============================================================

    // T6 · 条件A 撕裂修复（未知后继 + 货币链独立收口）
    #[test]
    fn itn_v2_026_t6_chain_tear_fixed() {
        // 未知后继不应撕裂：数值合成与后继识别正交
        assert_eq!(normalize_test("五块一以斤"), "5.1元以斤");
        assert_eq!(normalize_test("五块一已经"), "5.1元已经");
        // 已知后继（单价限定词）保持捕获
        assert_eq!(normalize_test("五块一一斤"), "5.1元一斤");
        // ITN-FIX-CHAIN-TEAR-026-B (Gavin 方案 C)：单段链一律保留原单位，与 per_unit 无关。
        // 五块一斤 = 1 段（"五块"）+ per_unit "一斤" → 单段保留原单位 → 5块一斤
        assert_eq!(normalize_test("五块一斤"), "5块一斤");
        // 完整链 + 单价限定词
        assert_eq!(normalize_test("五块一毛二一斤"), "5.12元一斤");
        assert_eq!(normalize_test("一块两毛二"), "1.22元");
        assert_eq!(normalize_test("一块两毛二一斤"), "1.22元一斤");
    }

    // T7 · 条件B 尾零去除（format_currency_chain 与 format_implicit_decimal 行为一致）
    #[test]
    fn itn_v2_026_t7_trailing_zero_removed() {
        assert_eq!(normalize_test("五块一"), "5.1元");     // 原 5.10元 → 修复
        // 单段链保持原单位（不归一到元）
        assert_eq!(normalize_test("五块"), "5块");        // 保持不变
        assert_eq!(normalize_test("八角"), "8角");          // 保持原单位
        assert_eq!(normalize_test("二十五块"), "25块");    // 保持原单位
        assert_eq!(normalize_test("一块两毛二"), "1.22元"); // 保持不变（两位有效）
        assert_eq!(normalize_test("五块一毛二"), "5.12元"); // 保持不变
        assert_eq!(normalize_test("一块八"), "1.8元");     // 原 1.80元 → 修复
        assert_eq!(normalize_test("一块八一斤"), "1.8元一斤");
    }

    // T8 · 交叉（条件A+B 叠加 + 017 单价限定词）
    #[test]
    fn itn_v2_026_t8_combined() {
        assert_eq!(normalize_test("三块四毛八一斤"), "3.48元一斤");
    }

    // T9 · ITN-FIX-CHAIN-TEAR-026-B (Gavin 方案 C) 核心差异锁死：
    // 段数决定是否归一，与 per_unit 无关。
    //   五块一斤 = 1 段（"五块"）+ per_unit "一斤" → 单段保留原单位 → 5块一斤
    //   五块一   = 2 段（"五块" + 隐式"一毛"）    → 多段归一到元 → 5.1元
    //   五块     = 1 段（"五块"）无 per_unit       → 单段保留原单位 → 5块
    #[test]
    fn itn_v2_026_t9_scheme_c_segment_count_rule() {
        assert_eq!(normalize_test("五块一斤"), "5块一斤");
        assert_eq!(normalize_test("五块一"), "5.1元");
        assert_eq!(normalize_test("五块"), "5块");
    }

    // T10 · DEC-038 保护词护栏（026 放开单段 currency 链后的主要风险面）
    // 本断言锁定的是 DEC-038 随机覆盖下的现状行为，不代表期望行为，P6 批次会整体重构。
    #[test]
    fn itn_v2_026_t10_dec038_protected_word_guards() {
        // 以下词条行为由 itn-rules.toml 保护词表随机覆盖决定，026-B 不应改变它们
        // 在保护词表中的条目（保持汉字）：
        assert_eq!(normalize_test("五毛钱"), "五毛钱");
        assert_eq!(normalize_test("一块钱"), "一块钱");
        assert_eq!(normalize_test("二块钱"), "二块钱");
        assert_eq!(normalize_test("六块钱"), "六块钱");
        assert_eq!(normalize_test("八块钱"), "八块钱");
        assert_eq!(normalize_test("一毛钱"), "一毛钱");
        assert_eq!(normalize_test("一角钱"), "一角钱");
        assert_eq!(normalize_test("五角钱"), "五角钱");
        // 不在保护词表中的条目（026 单段 currency 链转数字，现状锁定）：
        assert_eq!(normalize_test("三毛钱"), "3毛钱");
        assert_eq!(normalize_test("三块钱"), "3块钱");
        assert_eq!(normalize_test("三元钱"), "3元钱");
        assert_eq!(normalize_test("五块钱"), "5块钱");
    }

    // ============================================================
    // ITN-FIX-CHAIN-TEAR-026 反向护栏（B1-B5 五组歧义）
    // 026 改动 B 允许单段 currency 链后，以下用例可能被误转。
    // 本组断言锁定现状行为，不代表期望行为。
    // 若走查发现某条现状是误转，在断言前标注 TODO-026-REGRESSION。
    // ============================================================

    // B1 · 「块」量词义/虚指（非货币语境）
    #[test]
    fn itn_v2_026_b1_kuai_ambiguity() {
        // 保护词表命中 → 保持汉字
        assert_eq!(normalize_test("一块儿去"), "一块儿去");
        assert_eq!(normalize_test("一块钱"), "一块钱");
        // 非保护词表 → 单段 currency 链转数字（现状锁定，非货币语境可能误转）
        assert_eq!(normalize_test("一块石头"), "1块石头");
        assert_eq!(normalize_test("掰成两块"), "掰成2块");
        assert_eq!(normalize_test("三块木板"), "3块木板");
    }

    // B2 · 「角」歧义（几何/方位/货币）
    #[test]
    fn itn_v2_026_b2_jiao_ambiguity() {
        // 保护词表命中 → 保持汉字
        assert_eq!(normalize_test("八角茴香"), "八角茴香");
        assert_eq!(normalize_test("三角形"), "三角形");
        assert_eq!(normalize_test("一角钱"), "一角钱");
        // 非保护词表 → 单段 currency 链转数字（现状锁定）
        assert_eq!(normalize_test("四角"), "4角");
        assert_eq!(normalize_test("墙角"), "墙角");  // 墙非数字，不触发
    }

    // B3 · 「分」歧义（分数/时间/评分/货币）
    #[test]
    fn itn_v2_026_b3_fen_ambiguity() {
        // 保护词表命中 → 保持汉字
        assert_eq!(normalize_test("一分为二"), "一分为二");
        // 分数路径 → 1/3
        assert_eq!(normalize_test("三分之一"), "1/3");
        // 非保护词表 → 单段 currency/time 链转数字（现状锁定）
        assert_eq!(normalize_test("打了三分"), "打了3分");
        assert_eq!(normalize_test("五分熟"), "5分熟");
        // 时间语境 → 正确转数字
        assert_eq!(normalize_test("三分钟"), "3分钟");
    }

    // B4 · 「元」歧义（数学/纪年/货币）
    #[test]
    fn itn_v2_026_b4_yuan_ambiguity() {
        // 保护词表命中 → 保持汉字
        // 实测：三元一次方程 不在保护词表（unit_collisions 中无此条目），
        // 单段 currency 链捕获「三元」→「3元一次方程」。现状锁定。
        assert_eq!(normalize_test("三元一次方程"), "3元一次方程");
        // 实测：一元二次 同样被单段 currency 链捕获 →「1元二次」。现状锁定。
        assert_eq!(normalize_test("一元二次"), "1元二次");
        assert_eq!(normalize_test("三元钱"), "3元钱");
        // 非数字开头 → 不触发
        assert_eq!(normalize_test("公元"), "公元");
    }

    // B5 · 「毛」歧义（成语/专名/货币）
    #[test]
    fn itn_v2_026_b5_mao_ambiguity() {
        // 保护词表命中 → 保持汉字
        assert_eq!(normalize_test("一毛不拔"), "一毛不拔");
        // 非保护词表 → 单段 currency 链转数字（现状锁定）
        // TODO-026-REGRESSION: "三毛"（人名）被转 "3毛"，非货币语境误转
        assert_eq!(normalize_test("三毛"), "3毛");
        // "九牛一毛" 实测保持汉字（"九牛"非数字开头，不触发丙型链）
        assert_eq!(normalize_test("九牛一毛"), "九牛一毛");
    }

    // ============================================================
    // ITN-FIX-CHAIN-TEAR-026 交叉回归护栏（C1-C4）
    // ============================================================

    // C1 · 017 六条端测用例全部复核（尾零去除已同步）
    #[test]
    fn itn_v2_026_c1_017_six_bugs_still_fixed() {
        assert_eq!(normalize_test("一斤二两"), "1斤2两");
        assert_eq!(normalize_test("一块两毛二一斤"), "1.22元一斤");
        assert_eq!(normalize_test("三块四毛八一斤"), "3.48元一斤");
        assert_eq!(normalize_test("一块八毛一斤"), "1.8元一斤");
        assert_eq!(normalize_test("一块八一斤"), "1.8元一斤");
        assert_eq!(normalize_test("三斤六两五"), "3斤6两5");
    }

    // C2 · 尾零边界：total.fract()==0.0 走整数分支不去尾零
    #[test]
    fn itn_v2_026_c2_trailing_zero_boundary() {
        // fract()==0.0 → 整数分支，不去尾零
        assert_eq!(normalize_test("五块"), "5块");          // 单段，5.0 → 5块
        // 实测：五块零 → 丙型链捕获「五块」+ 隐式尾数「零」=0毛 → total=5.0 → 5元
        assert_eq!(normalize_test("五块零"), "5元");
        // 多段链 fract()==0.0 → 整数元
        assert_eq!(normalize_test("五块零毛"), "5元");
        // 多段链 fract()!=0.0 → 去尾零
        assert_eq!(normalize_test("五块零一"), "5.01元");    // 5.01 保留
        assert_eq!(normalize_test("五块一"), "5.1元");       // 5.10 → 5.1
        assert_eq!(normalize_test("五块零毛一"), "5.01元");  // 5.01 保留
        // 边界：0.05 元
        assert_eq!(normalize_test("五分"), "5分");           // 单段保留原单位
    }

    // C3 · weight 族不受 026 影响（只动 currency）
    #[test]
    fn itn_v2_026_c3_weight_unchanged() {
        assert_eq!(normalize_test("三斤六两五"), "3斤6两5");
        assert_eq!(normalize_test("一斤二两"), "1斤2两");
        assert_eq!(normalize_test("二两"), "2两");
        assert_eq!(normalize_test("五斤"), "5斤");           // 单段 weight 链不受影响
    }

    // C4 · 016 班级简写零回归（026 只动 currency 族）
    #[test]
    fn itn_v2_026_c4_grade_class_unchanged() {
        assert_eq!(normalize_test("一三班"), "一三班");
        assert_eq!(normalize_test("五一班"), "五一班");
        assert_eq!(normalize_test("初二三班"), "初二三班");
        assert_eq!(normalize_test("高一四班"), "高一四班");
    }
}
