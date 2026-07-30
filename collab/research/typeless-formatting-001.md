# 竞品研究：Typeless「格式化输出」功能分析

> 编号：RESEARCH-TYPELESS-FORMAT-001
> 日期：2026-07-10
> 研究人：Orchestrator（咖啡）
> 背景：Gavin 指令——研究知名语音输入法 Typeless 的格式化输出具备哪些特点和功能
> 关联决策：DEC-029（词库单词化）/ DEC-030（智能 ITN）/ FILLER-STRIP-001（待拍板）

---

## 一句话定位

Typeless 的格式化输出不是规则引擎，而是**「转录 + LLM 语义重写」两段式管线**——ASR 出原始文本后，云端 LLM 按「你想表达什么」而非「你说了什么」重写全文。产品口号即此理念："AI voice dictation that's actually intelligent"。

---

## 格式化输出的六大能力

### 1. 结构自动化（Auto-formatting）——最核心卖点

- 口述的清单、步骤、要点自动重组为结构化文本
- 官网例子：说 "My shopping list, bananas, oat milk, dark chocolate" → 自动输出带标题的分行列表
- 中文实测例子：说"这个星期我主要做了三件事：第一，测试了新功能，发现有个bug；第二，和团队讨论了下周计划；第三，完成了用户反馈分析" → 输出"本周主要工作：1) 完成新功能测试并发现bug；2) 与团队讨论下周计划；3) 完成用户反馈分析"
- **注意：连内容都做了压缩提炼，不只是加序号**

### 2. 语气词/口头禅去除（Filler removal）

- 中文"嗯、啊、额、那个"、英文"um、uh、you know、like"自动删除
- 覆盖范围比纯规则方案更宽（含口头禅类，非仅拟声语气词）

### 3. 改口修正（Self-correction）+ 去重

- 识别中途改口，只保留最终意图（说"周三开会……不对，周四"→ 只出"周四开会"）
- 口吃/卡顿导致的邻近重复词自动清理

### 4. 场景感知语气适配（Context-aware tone）——差异化最强

- 检测当前焦点应用，同一句话在不同 App 输出不同文风
- 实例：短信里保留 "kinda wanna" 口语体；切到邮件应用自动改写为 "I kind of want to" 正式体；"tho" 补全为 "though"
- 本质：把「注入目标窗口」作为 prompt 上下文变量

### 5. 个人化风格 + 自定义词典

- 学习用户措辞习惯，输出"像你写的"
- 个人词典保证专有名词（Kubernetes、品牌名）拼写恒定
- **词典条目作为 LLM 上下文参与润色，不是字符串替换**（与我们 DEC-029 词库单词化选的同一路线）

### 6. 选中即语音编辑（Voice commands on selection）

- 选中任意文本后口述指令：改短/改长/换语气/翻译/生成回复
- 格式化能力从"输入时"延伸到"编辑时"

---

## 实现机制与代价（架构视角）

| 维度 | 事实 | 含义 |
| --- | --- | --- |
| 处理位置 | 全云端，无本地离线模式 | 隐私是公开软肋（评测明确提醒只用于不敏感内容） |
| 延迟 | 松开快捷键后"思考几秒"才上屏 | LLM 全文重写的固有代价，换全文级重组能力 |
| 商业模式 | 免费版每周 4000 词；Pro $12/月（年付）/ $30/月（月付） | 云端 LLM 成本决定其必须订阅制 |
| 中文表现 | 中英混说、英文专有名词（ChatGPT/Gemini）识别准确，繁体场景突出 | 多语言混合是其强项 |

---

## 对 voice-ime 的对照与启示

| Typeless 能力 | voice-ime 现状 | 差距/机会 |
| --- | --- | --- |
| 语气词去除 | FILLER-STRIP-001 等 Gavin 拍板（A 规则后处理 + C LLM prompt 双路线） | Typeless 验证 LLM 路线（C 方案）是业界标杆做法；它连"那个/就是"口头禅也删（我们 v1 建议不碰的部分） |
| 数字/标点规整 | ITN-SMART-001 已交付（本地规则引擎，零延迟零隐私代价） | 确定性场景我们的规则路线反而更稳；Typeless 无独立数字规整，靠 LLM 顺带 |
| 词典参与纠偏 | DEC-029 词汇表进 LLM prompt + accuracy hotwords | 路线一致，已对齐业界做法 |
| 列表/结构重组 | ❌ 无 | Typeless 格式化的**皇冠功能**；我们 LLM 开启时可通过 prompt 扩展实现 |
| 改口修正 | ❌ 无 | LLM prompt 可覆盖，与 FILLER-STRIP C 方案是同一个 prompt 的不同指令段 |
| 场景语气适配 | ❌ 无 | 需焦点窗口检测（Win32 基础已有）+ prompt 变量；差异化大功能但建议排后 |
| 选中语音编辑 | ❌ 无 | 交互形态大改，远期评估 |

### 关键结论

1. Typeless 的"格式化输出" = **一个精心设计的 LLM 后处理 prompt 体系**（语气词/改口/结构/语气四类指令）+ 焦点应用上下文，无独立规则引擎
2. voice-ime 的架构差异在**分层**：确定性规整走本地规则（ITN——零延迟、离线可用、隐私无代价），语义级格式化走 LLM（可选开启）——这一分层对隐私敏感用户是相对 Typeless 的结构性优势
3. **建议（供拍板参考）**：若 FILLER-STRIP-001 立项选 C 方案，可顺势把 LLM prompt 升级为「格式化指令集」（语气词去除 + 改口修正 + 列表结构重组一并做），一次 LLM 调用覆盖 Typeless 三项核心能力，比逐个立项更划算；规则路线（A 方案）仍可作为 LLM 未开启时的保底层

---

## 信息来源

- [Typeless 官网功能页](https://www.typeless.com/)
- [少数派：Typeless 语音输入法新物种，开启 AI 信息记录新时代](https://sspai.com/post/105358)
- [效率火箭：可能是目前最理想的智能语音输入了](https://xlrocket.blog/2026/02/01/%E5%8F%AF%E8%83%BD%E6%98%AF%E7%9B%AE%E5%89%8D%E6%9C%80%E7%90%86%E6%83%B3%E7%9A%84%E6%99%BA%E8%83%BD%E8%AF%AD%E9%9F%B3%E8%BE%93%E5%85%A5%E4%BA%86%EF%BC%8C%E4%BB%8E%E6%8E%92%E6%96%A5%E8%AF%AD%E9%9F%B3/)
- [兔哥博客：Typeless 中文语音输入设置指南及心得](https://uuzi.net/typeless-ai-voice-input-review/)
- [OpenTypeless 功能页（开源对标品）](https://www.opentypeless.com/en/features)
- [ChatGate：Typeless auto-edits speech](https://chatgate.ai/post/typeless)
- [AIHub：Typeless AI 智能语音输入工具](https://www.aihub.cn/tools/typeless/)
