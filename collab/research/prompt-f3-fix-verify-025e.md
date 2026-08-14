# PROMPT-ARCH-025 阶段五 · 修法验证实验报告

> 生成时间：2026-08-03
> 实验者：tester-1
> 性质：零生产改动实验，验证 V10（规则层）/ V11（示例层）/ V12（组合）

---

## 一、机制假设（阶段四结论）

**无序标记本身不足以触发列表；模型还需要一个「预告列表」的语义线索。有序标记自带这个线索，无序标记没有。**

- IN-A 成功（"建议从以下方面入手" = 枚举预设）→ 无序标记被识别
- IN-B 失败（"指出一些现象" = 叙事预设）→ 无序标记被判定为叙事修辞
- B4 成功（有序标记）→ 有序标记自带序列语义，穿透叙事预设

---

## 二、本轮修法设计

### V10 · 规则层

在 F3 DECISION RULE 后追加三条语义规则（英文）：
1. 枚举的成立**不要求**存在预告短语
2. **叙述性或描述性的引入句不降低枚举的成立性**
3. 判据仍然只有一条：是否存在 2+ 个并列跨度

### V11 · 示例层

在 F3c few-shot 中追加一条：叙事引入句 + 无序标记 → 仍然出列表（用 IN-B 真实形态构造）。

### V12 · V10 + V11

---

## 三、完整矩阵（每组 3 次）

| 输入 | V10 | V11 | V12 | 期望 |
|------|-----|-----|-----|------|
| **IN-A** | **3/3 bullet** | **3/3 bullet** | **3/3 bullet** | 列表 |
| **IN-B** | **0/3 para** | **0/3 para** | **0/3 para** | **列表** |
| **IN-C** | **0/3 para** | **0/3 para** | **1/3 bullet**, 2/3 para | **列表** |
| C1 有序 | 3/3 ordered | 3/3 ordered | 3/3 ordered | 有序 |
| C2 单例 | 3/3 para | 3/3 para | 3/3 para | 段落 |
| C3 短项 | 3/3 inline | 3/3 inline | 3/3 inline | 内联 |
| C4 叙事无并列 | 3/3 para | 3/3 para | 3/3 para | 段落 |

### 判死条件检查

| 方案 | C2 单例 | C3 短项 | C4 叙事无并列 | 判死？ |
|------|---------|---------|-------------|--------|
| V10 | ✅ 3/3 para | ✅ 3/3 inline | ✅ 3/3 para | **未判死** |
| V11 | ✅ 3/3 para | ✅ 3/3 inline | ✅ 3/3 para | **未判死** |
| V12 | ✅ 3/3 para | ✅ 3/3 inline | ✅ 3/3 para | **未判死** |

---

## 四、结论

### 核心结果

**V10、V11、V12 全部未能让 IN-B 稳定出列表（0/3 para），IN-C 也全部失败（V12 仅 1/3 bullet，噪声级）。**

### 意味着什么

1. **V10 的规则语言太抽象**：LLM 无法从"叙述性引入句不降低枚举成立性"这条规则中学会具体应用。抽象规则对 LLM 的校准效果有限。

2. **V11 的单个 few-shot 不够**：一个叙事引入+无序枚举的示例不足以覆盖所有变体。LLM 可能把这个示例当成特例，而不是普遍规则。

3. **机制假设可能需要深化**：当前机制假设（"预告列表语义线索"）能解释已有数据，但按此机制设计的修法未能修复问题。可能需要一个更强的信号——例如**在无序标记本身增加语义权重**，或者**增加多个不同形态的叙事+无序 few-shot**。

### 下一步建议（按优先级）

1. **V13 · 多示例覆盖**：在 F3c 中追加 3-5 个不同形态的「叙事引入 + 无序枚举」few-shot（学校场景、公司场景、日常场景），让 LLM 看到这是普遍模式而非特例。

2. **V14 · 标记词增强**：在 F3 的无序标记词表中，增加一条语义注解——"比如/再比如/还有在描述性语境中仍应被视为枚举标记，而非单纯修辞"。

3. **V15 · 引入句去权重**：实验去掉/弱化引入句在提示词中的权重，测试是否能让 LLM 更关注项目内容本身而非引入句预设。（但此方案风险高，可能破坏其他场景。）

---

## 五、原始数据

- `prompt-f3-fix-verify-025e.json` —— 63 次 API 响应
- `harness_025e.log` —— 终端输出
- `harness_025e.py` —— 实验脚本

---

## 六、提示词文本（实际使用的 V10/V11/V12）

### V10 追加文本

```
Additional Semantic Rule (DEC-039, symmetric): 
1) An enumeration does NOT require a预告短语 (e.g., "from the following aspects", "there are several points"). 
2) A narrative or descriptive introductory sentence does NOT reduce the validity of a subsequent enumeration 
— the intro merely sets the topic; the parallel items that follow are still an enumeration if they meet the DECISION RULE. 
3) The sole decisive test remains: whether TWO OR MORE spans stand in a PARALLEL relation — same syntactic slot, same semantic function, 
each contributing one coordinate member to a set. Preserve this test regardless of the surrounding discourse type.
```

### V11 追加文本

```
- Chinese unordered with narrative intro (markers DIFFER): 
"今天的讲话指出了学校的一些现象。比如说有些同学下课之后徘徊在校园里不走，三五成群；
再比如有的同学烫发、染黄头发、染头发，这个不符合校规；
还有的同学不遵守课堂纪律，顶撞老师。" → 
"- 有些同学下课之后徘徊在校园里不走，三五成群\n
- 有的同学烫发、染黄头发、染头发，这个不符合校规\n
- 有的同学不遵守课堂纪律，顶撞老师".
```

---

## 七、API 使用

本轮：63 次调用。
阶段二~五累计：165 次。
