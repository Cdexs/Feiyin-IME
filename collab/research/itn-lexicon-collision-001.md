# ITN 单位前缀碰撞词表调研报告

## 调研目标
为修复 `ITN-GEOMETRIC-001` / 单位前缀碰撞导致的误转换问题，评估**是否可以从公开中文词库中离线挖掘一个“单位前缀碰撞保护词表”**，而不是在运行时挂字典或维护完整白名单。

## 核心结论
**可行**。以“单位首字符 / 数字+单位首字符”为碰撞特征，从公开词库中离线筛选出 **31,881** 条候选词。这些词覆盖了几何术语（三角形、三角洲）、常见动名词（批发、元素、度假、节目、升级、克服）等已知漏转案例，同时**不会破坏** “三十度 / 三分钟 / 三分之一 / 三点五米” 等必须转换的表达。

## 词库来源
| 词库 | 许可证 | 原始条目数 | 候选命中数 |
|------|--------|-----------|-----------|
| jieba（fxsjy/jieba）| MIT | ~349k | 19,001 |
| CC-CEDICT | CC BY-SA 4.0 | ~124k | 5,772 |
| THUOCL 11 类 | MIT | ~586k | 12,554 |

## 碰撞类型定义
- **Type A**：词以中文数字开头，第二个字符为 ITN 单位首字（如 `三 + 角 → 三角形`、`三 + 角 → 三角洲`）。
- **Type B**：词以 ITN 单位首字开头（如 `批 + 发 → 批发`、`元 + 素 → 元素`、`度 + 假 → 度假`）。

筛选时**剔除**：
1. 本身就是 ITN 单位词的全部形式（如 `元`、`公斤`、`分`）；
2. 所有 `必须转换` 表达的后缀（避免保护 `三/十度` 中的 `度` 等片段）；
3. 全由数字和单位字符组成的词（如 `三公里`、`五公斤`）。

## 当前代码行为验证
通过阅读 `src/itn.rs` 的 `normalize_with_rules` 与 `decide_conversion` 可知：

- `三角形` 会被解析为 `三`（数字）+ `角形`（`角` 是单位），输出 `3角形`。
- `三角洲` 同理输出 `3角洲`。
- `五批发`、`一元素` 等会被解析为 `五/一` + `批发/元素`（`批/元` 都是单位），输出 `5批发 / 1元素`。
- `批发`、`元素` 等单独出现时不转换，但当它们紧跟在数字后就会误转。

因此把它们作为**整词**加入 `check_protection` 的 `proper_nouns`（或新增 collision 分组）即可避免误转。

## 候选集命中情况
| 目标词 | 是否在候选集 |
|--------|-------------|
| 三角形 | ✅ 是 |
| 三角洲 | ✅ 是 |
| 元素 | ✅ 是 |
| 度假 | ✅ 是 |
| 批发 | ✅ 是 |
| 克服 | ✅ 是 |
| 升级 | ✅ 是 |
| 节目 | ✅ 是 |

## 必须转换表达的后缀检查
对下列 must-convert 表达做反向验证，候选集中**没有出现任何会阻止它们整体转换的后缀**：

三十度, 三分钟, 五公斤, 三个人, 十块钱, 两毛五, 第三次, 三分之一, 百分之三十, 三点五米, 零下十摄氏度

> 例如 `度` 在候选集中只作为普通词头出现，但 `三十度` 中的 `度` 本身并不被整词保护；候选词 `三角形` 等也**不是**任何 must-convert 表达的后缀。

## 词表规模与分布
- 候选词总数：**31,881** 条。
- Type A（数字+单位首字）：2,107 条。
- Type B（单位首字开头）：29,774 条。
- TOML 格式体积：约 563 KB。
- 频次分布：

| 频次区间 | 词数 |
|----------|------|
| 1 <= freq < 10 | 19,109 |
| 10 <= freq < 100 | 6,696 |
| 100 <= freq < 1,000 | 3,250 |
| 1,000 <= freq < 10,000 | 1,236 |
| 10,000 <= freq < 100,000 | 251 |
| 100,000 <= freq < ∞ | 34 |

## 落地形式
- **轻量接入**：从候选集中按频次/长度取 cutoff（如 top N 或长度 ≤4），加入 `itn-rules.toml` 的 `[protect.proper_nouns]` 或新增 `[protect.unit_collisions]`。
- **完整接入**：将候选集作为独立 TOML 文件，由 `CompiledRules` 加载为 `collision_set`，在 `check_protection` 中前缀匹配。

## 风险与建议
1. **过度保护风险**：31k 条候选经 must-convert 反向校验，未发现会把真正需要转换的表达误保护。
2. **性能**：`check_protection` 使用 HashSet 迭代前缀匹配，31k 整词单次扫描不会明显拖慢；若担心，可按频次做 cutoff 或 Trie。
3. **版权**：CC-CEDICT 为 CC BY-SA 4.0，直接纳入发行需注明；MIT 词库无此顾虑。建议仅将词库作为**离线派生依据**，最终只输出“词列表”，并在文档致谢来源。

## 交付物
- `collab/research/data/candidate_protection_list_v4.toml`：可直接嵌入 ITN 规则的 TOML 数组。
- `collab/research/data/candidate_protection_words_v4.txt`：纯词列表（按长度降序）。
- `collab/research/itn-lexicon-collision-001.md`：完整调研报告。
- `collab/outbox/coder-2/result.md`：面向调度员的摘要。

## 下一步
1. 产品/主控确认落地策略：完整 31k 词表还是高频 cutoff。
2. 如采纳，将 `[protect.unit_collisions]` 接入 `itn-rules.toml` 与 `src/itn.rs` 的 `check_protection`。
3. 增加单元测试覆盖新增的 bug case（如 `三角形`、`批发`、`元素` 等）。


# ITN-COLLISION-TYPEA-001 阶段一报告

## 任务目标
- 剔除 Type A 候选集中的金额/数量串（避免破坏 DEC-030 金额转换）。
- 补齐三份词库的许可证证据（URL + 原文 + 商业/署名结论）。
- 阶段二待主控放行。

## 一、Type A 过滤方案对比

Type A 候选集总数：**2107** 条。

| 方案 | 剔除数 | 保留数 | 核心判据 |
|------|--------|--------|----------|
| **F1 语义规则** | 366 | 1741 | 含 千/百/万/亿/余 **且** 去掉常见货币/单位后缀后全为数字/量词/单位字符 |
| **F2 长度阈值 <=5** | 384 | 1723 | 仅保留长度 <=5 的词 |

### F1 剔除样例（金额/数量串）
```
一千多年	176
二千平方米	128
一千多	101
两千多年	80
两千多	59
一千五百	47
五千余	41
一千余	38
三千多	38
三千余	35
二千余	33
一千万	29
两千余	28
五千万	28
一个亿	24
```

### F2 剔除样例（长度 >5）
```
四个坚定不移	2262
三元运算符 	2007
二分查找法 	1490
二分图匹配 	1453
二分法查找 	1329
一年之计在于春 	1237
一年之计在于春	1231
三角形面积 	1160
二分查找算法 	901
四种启动模式 	887
两点间距离 	702
一个巴掌拍不响 	688
一个萝卜一个坑 	686
三层交换机 	643
二分插入排序 	593
```

### F1 保留、但 F2 误删的该保护词（共 287 条，样例 20）
```
一个三十出头
一个三十多岁
一个中国政策
一个二十七八岁
一个二十三岁
一个二十五六岁
一个二十五岁
一个二十多岁
一个五十多岁
一个五十左右
一个八九十岁
一个六十七岁
一个六十多岁
一个十一二岁
一个十七八岁
一个十三四岁
一个十二三岁
一个十五六岁
一个十八九岁
一个十六七岁
```

### F2 保留、但 F1 正确剔除的金额/数量词（共 269 条，样例 20）
```
一个万里
一个三四百
一个两万多
一个五百
一个亿
一个亿万
一个几万
一个几亿
一个几百万
一个十余丈
一个十余岁
一个多亿
一个百里
一千一万
一千一万个
一千一百
一千一百万
一千一百里
一千七百
一千万
```

### 关键发现
- F2 把 `一个三十多岁`、`一个二十七八岁`、`一个五十多岁` 等任务明确要保护的年龄表达误删。
- F2 保留了 `一千一百`、`一千万`、`一个五百`、`一个亿` 等应剔除的数量/金额短串。
- F1 保留了所有任务要求的示例：`一个七十岁`、`一个九十度`、`一个五十两`、`一个六十多`、`一个三十岁` 均**未被剔除**。
- Type A 目标 bug 词 `三角形`、`三角洲` 在 F1/F2 中均被保留。

### 推荐
**采用 F1 语义规则**。它更精确：只剔除明显是数量/金额表达的长串，保留几何/专有/年龄等真正需要保护的词。F2 长度阈值过于粗暴。

## 二、许可证取证

### 1. jieba
- **仓库**：https://github.com/fxsjy/jieba
- **LICENSE（官方）**：https://raw.githubusercontent.com/fxsjy/jieba/master/LICENSE
- **原文摘录**：
  > MIT License
  > Copyright (c) 2013 Sun Junyi
  > Permission is hereby granted, free of charge, to any person obtaining a copy...
  > ... including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software...
- **商业闭源分发**：允许
- **署名要求**：需在副本中保留版权声明和许可声明。建议放在 `itn-rules.toml` 头部注释或 `docs/third-party-licenses.md`。

### 2. THUOCL
- **仓库**：https://github.com/thunlp/THUOCL
- **LICENSE（官方）**：https://raw.githubusercontent.com/thunlp/THUOCL/master/LICENSE
- **README 开源协议段落（官方）**：https://github.com/thunlp/THUOCL/blob/master/README.md
- **原文摘录**：
  > MIT License
  > Copyright (c) 2018 THUNLP
  > Permission is hereby granted, free of charge, to any person obtaining a copy...
  > ... including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software...
- **README 附加引用要求**：
  > README.md 开源协议段落要求：
  > "THUOCL面向国内外大学、研究所、企业、机构以及个人免费开放，可用于研究与商业。"
  > "如果您在THUOCL基础上发表论文或取得科研成果，请您在发表论文和申报成果时声明使用了清华大学开放中文词库，并按如下格式引用："
  > 中文： 韩世依, 张钰晖, 马云山, 涂存超, 郭志芃, 刘知远, 孙茂松. THUOCL：清华大学开放中文词库. 2016.
  > 英文： Shiyi Han, Yuhui Zhang, Yunshan Ma, Cunchao Tu, Zhipeng Guo, Zhiyuan Liu, Maosong Sun. THUOCL: Tsinghua Open Chinese Lexicon. 2016.
- **商业闭源分发**：允许（MIT 许可 + README 明确可用于研究与商业）
- **署名要求**：保留 MIT 声明；如发表科研成果需引用论文。作为闭源产品随包分发，建议同时保留 MIT 声明和 THUOCL 中文/英文引用格式。

### 3. CC-CEDICT
- **官方下载/许可页**：https://www.mdbg.net/chinese/dictionary?page=cc-cedict
- **官方下载入口**：https://cc-cedict.org/editor/editor.php?handler=Download
- **法律文本**：https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt
- **原文摘录**：
  > This work is licensed under a Creative Commons Attribution-ShareAlike 4.0 International License.
  > 
  > It more or less means that you are allowed to use this data for both non-commercial and commercial purposes provided that you:
  > - mention where you got the data from (attribution)
  > - and that in case you improve / add to the data you will share these changes under the same license (share alike).
- **商业使用**：允许（BY-SA 允许商业使用）
- **闭源分发风险**：⚠️ **share-alike 条款可能传染派生数据**。CC BY-SA 4.0 第 3(b) 条规定：若分享“Adapted Material”，必须使用相同许可。第 4 条进一步说明：若分享数据库的“substantial portion”，也须遵守署名和相同方式共享。
- **结论**：
  - 如果将 CC-CEDICT 的 5,772 条候选词原样/近似原样放入闭源产品，存在许可证传染风险。
  - 本项目诉求是离线派生一个“是否为单位前缀碰撞”的二元判断，最终只输出少量汉字序列。该派生数据是否构成“Adapted Material”或“substantial portion”存在解释空间，**法律上不确定**。
  - 为保守起见，**建议阶段二落地时弃用 CC-CEDICT 来源**。

### 4. 弃用 CC-CEDICT 后 Type A 目标覆盖情况
Type A 目标 bug 词只有 `三角形` 和 `三角洲`。
- `三角形`：jieba + THUOCL
- `三角洲`：jieba + THUOCL

**弃用 CC-CEDICT 不影响 Type A 目标覆盖。**

## 三、署名/致谢文本建议
在 `itn-rules.toml` 新增分组头部注释中写入：

```toml
# [protect.unit_collisions]
# Derived from jieba (MIT, https://github.com/fxsjy/jieba) and
# THUOCL (MIT, https://github.com/thunlp/THUOCL).
# Citation: Han et al., THUOCL: Tsinghua Open Chinese Lexicon, 2016.
# CC-CEDICT source excluded from this release to avoid CC BY-SA share-alike ambiguity.
# Generation date: 2026-07-30
# Rules: Type A only (digit + unit-prefix start); money/quantity strings removed.
# Type B deferred to ITN-COLLISION-TYPEB-001.
```

或单独维护 `docs/third-party-licenses.md` 并在 toml 注释中引用。

## 四、阶段二建议（待主控确认）
1. 以 **F1 语义规则** 净化 Type A，得到 **1741** 条候选。
2. **剔除 CC-CEDICT 来源**后剩余条目数待算；预计保留 jieba/THUOCL 交集/并集。
3. 与既有 `protect.idioms`/`proper_nouns`/`historical`（共 245 条）去重。
4. 新增 `[protect.unit_collisions]` 分组并入 `itn-rules.toml`，或按主控意见并入 `proper_nouns`。
   - `CompiledRules` 侧：若新增分组，只需在 `Rules`/`CompiledRules` 加字段并在 `check_protection` 加一段 HashSet 前缀匹配。**这属于数据文件联动的小代码改动**；任务书阶段二允许只改根目录 `itn-rules.toml`，但新增分组必然需要代码侧同步。因此**先报主控确认**。
5. 运行 `cargo check --tests`。

## 五、macOS 影响评估
`itn.rs` 与 `itn-rules.toml` 均平台中立，纯数据方案直接让 macOS 侧 ITN 同时受益，无需额外改动。
