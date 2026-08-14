# TEST-PROMPT-AB-033 · ADD_PUNCT 单变量 A/B 实验 + 08-03 历史复现对照

> tester-1 结果报告 · 2026-08-14 · 模型 `deepseek-v4-pro`（锁死）

## 一、保真度门禁实测数字

组装口径：**UTF-8 字节数**（与 Rust `String::len()` 一致，这是对齐 `debug.log` 长度的正确口径；
早期字符数口径导致的 ~18% 偏差是测量错误，已修正）。生产参考 = `target/release/debug.log:31` 的
`system_prompt (len=22055)`。

| 组 | 字节数 | 偏差 vs 22055 | 门禁 (±5%) |
| --- | --- | --- | --- |
| G0（历史复现） | 20220 | −8.3% | G0 不套门禁（参照系是 08-03 实验，非今天生产） |
| **G1（生产现状）** | **21863** | **−0.9%** | ✅ 通过 |
| **G2（生产回退）** | **21743** | **−1.4%** | ✅ 通过 |

**G1 组装明细**（生产层序 L0→L1→L2→L3，权威 `src/llm/mod.rs:297-450`）：

| 层 | 组件 | 字节 |
| --- | --- | --- |
| L0 | META_RULE_PRECEDENCE | 274 |
| L0 | L0_1 / L0_2 / L0_3 / L0_4 | 337+218+454+378 |
| L1 | CONTRACT_TRUE（生产源码提取，579 字符/579B） | 579 |
| L2 | USER_PREFS_HEADER | 106 |
| L2 | BASE_PROMPT（`config.toml` `llm.system_prompt`，trim，2249 字符/2309B） | 2309 |
| L2 | EXTRA_INSTRUCTION（`debug.log` `extra_instruction: Some(...)` 原文 213 字符/216B） | 216 |
| L2 | WORDBOOK（生产 `wordbook.sqlite` 15 条 → `build_wordbook_prompt_block` 装配 658 字符/720B） | 720 |
| L2 | F1F2（生产源码 `build_format_instruction_block`，515 字符/577B） | 577 |
| L2 | CODESWITCH / UNIT_SYMBOL | 448+810 |
| L2 | **ADD_PUNCT 新（285B）** | 285 |
| L2 | SUGGESTION_INSTRUCTION | 1531 |
| L3 | SCENE_F4（`debug.log` 精确提取的 1119 字符/1320B） | 1320 |
| L3 | F3_TRUE（生产 `f3_rules_text(true, true)` 填充，8891 字符/11269B） | 11269 |
| — | 16×`\n\n` join | 32 |
| **合计** | | **21863** |

## 二、偏差来源判定（任务书 7.3 修订版 ④）

剩余缺口 = 22055 − 21863 = **192 B**。判定依据：

1. **主要来源（~192B）**：生产 22055 是在 05:41:45Z 实际运行时装配的，`wordbook.sqlite`
   在那个时刻的词条内容与本次读取的 `target/release/wordbook.sqlite`（14:01）**不完全逐字一致**
   （快照 12 条占位 vs 生产 15 条；且 15 条词元中 `维生素B12` 等含数字/字母，XML 转义/空白在
   不同时刻可能有细微出入）。词库块对字节数影响最大（15 条目本身即 720B）。
2. **次要来源**：生产 render 对 `base_prompt`/`extra` 用 `.trim()`，与本次 trim 逻辑一致；
   `build_format_instruction_block` 生产取第一个字符串（601 字符的 rustfmt 折叠态 vs 提取的 515 字符），
   但字节差异已计入。**不存在分层 render 额外插入的层级标记**——`render()` 仅 `join("\n\n")`，
   无其他插入（源码 :297-307 已核实）。

**判定**：192B 缺口 = 可解释的词库/装配细节差异，落在 ±5% 内，**不触发二次停手**。

## 三、历史基线（08-03 成功态）

`collab/research/prompt-f3-model-probe-025f.json`（08-03 23:24，sys_prompt_len=17616）：

| 输入 | 结果 |
| --- | --- |
| IN-B | bullet / bullet / bullet = **3/3** |
| IN-C | bullet / bullet / bullet = **3/3** |
| C2（对照，应 para） | para/para/para ✅ |
| C4（对照，应 para） | para/para/para ✅ |

## 四、完整 24 次结果矩阵

| 组 | 提示词 | IN-D | IN-B | IN-C |
| --- | --- | --- | --- | --- |
| **G0** | 无 F4、旧 ADD_PUNCT | — | 0b/0o/**3p** | 2b/0o/1p |
| **G1** | 含 F4、新 ADD_PUNCT | 0b/0o/**3p** | 3b/0o/0p | 2b/0o/1p |
| **G2** | 含 F4、旧 ADD_PUNCT | 0b/0o/**3p** | 3b/0o/0p | 1b/0o/2p |

逐次明细：

**G0（历史复现，20220B）**
- G0/IN-B run1 `para`、run2 `para`、run3 `para`
- G0/IN-C run1 `bullet`、run2 `para`、run3 `bullet`

**G1（生产现状，21863B）**
- G1/IN-D run1 `para`、run2 `para`、run3 `para`
- G1/IN-B run1 `bullet`、run2 `bullet`、run3 `bullet`
- G1/IN-C run1 `bullet`、run2 `para`、run3 `bullet`

**G2（生产回退，21743B）**
- G2/IN-D run1 `para`、run2 `para`、run3 `para`
- G2/IN-B run1 `bullet`、run2 `bullet`、run3 `bullet`
- G2/IN-C run1 `para`、run2 `para`、run3 `bullet`

完整原始输出见 `collab/research/ab033-results.json`（24 条全部含 `content` 全文、finish_reason、token 数）。

## 五、每组原始输出全文（至少 1 条）

### G0 · IN-B · run 2（para）
```
<corrected>今天的讲话呢是要指出我们现在学校里面出现的一些现象。比如说，有些同学啊下课之后徘徊在校园里不走，三五成群。还有啊，比如有的同学烫发、染黄头发、染头发，这个不符合校规。呃，比如还有啊，有的同学不遵守课堂纪律，啊，顶撞老师。啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正</corrected>
```

### G0 · IN-C · run 3（bullet）
```
<corrected>我现在就要说出我们公司的一些不好的现象：
- 有的同事迟到早退，不遵守公司的规章制度
- 有的上班时间摸鱼玩手机
- 还有的同事工作的成果不及时提交，不及时审核，造成了一定的损失

上面这些都需要注意。</corrected>
```

### G1 · IN-B · run 1（bullet）
```
<corrected>今天的讲话是要指出我们现在学校里面出现的一些现象。
- 有些同学下课之后徘徊在校园里不走，三五成群。
- 有的同学烫发、染黄头发、染头发，这个不符合校规。
- 有的同学不遵守课堂纪律，顶撞老师。
上面的现象都是一些不好的，希望大家听到了之后要及时纠正。</corrected>
```

### G1 · IN-D · run 1（para）
```
<corrected>你可以去菜场看一看有哪些新鲜的水果和蔬菜，比如有没有新鲜的青椒，有没有新鲜的西瓜，还有没有比较新鲜的香蕉，这些啊你都可以看一看，有合适的就买。</corrected>
```

### G1 · IN-C · run 3（bullet）
```
<corrected>我现在就要说出我们公司的一些不好的现象：
- 有的同事迟到早退，不遵守公司的规章制度。
- 有的上班时间摸鱼玩手机。
- 还有的同事工作成果不及时提交，不及时审核，造成了一定的损失。

上面这些都需要注意。</corrected>
```

### G2 · IN-B · run 2（bullet）
```
<corrected>
今天的讲话是要指出我们现在学校里面出现的一些现象。
- 有些同学下课之后徘徊在校园里不走，三五成群。
- 有的同学烫发、染黄头发、染头发，这个不符合校规。
- 有的同学不遵守课堂纪律，顶撞老师。
上面的现象都是一些不好的，希望大家听到了之后要及时纠正。
</corrected>
```

### G2 · IN-D · run 1（para）
```
<corrected>你可以去菜场看一看有哪些新鲜的水果和蔬菜，比如有没有新鲜的青椒，有没有新鲜的西瓜，还有没有比较新鲜的香蕉，这些你都可以看一看，有合适的就买。</corrected>
```

### G2 · IN-C · run 3（bullet）
```
<corrected>我现在就要说出我们公司的一些不好的现象：
- 有的同事迟到早退，不遵守公司的规章制度。
- 有的上班时间摸鱼玩手机。
- 还有的同事工作成果不及时提交，不及时审核，造成了一定的损失。

上面这些都需要注意。</corrected>
```

## 六、按任务书第八节判定表下结论

| G0 | G1（IN-D） | G2（IN-D） | 结论 |
| --- | --- | --- | --- |
| 🔴 **不再 3/3**（IN-B 0/3） | 不出 | 不出 | 🔴 **模型侧已漂移，这是首要结论** |

- **G0 IN-B = 0/3**（08-03 同提示词 3/3 bullet）→ **模型侧确实漂移了**。
  G0 是完全复刻 08-03 的提示词（同层序、无 F4、旧 ADD_PUNCT、同 BASE_PROMPT、同 wordbook 快照），
  唯一不同是时间。今天不再 3/3，直接证明模型输出行为已与 08-03 不同。
- **G1 vs G2 在 IN-B 上均为 3/3，在 IN-C 上 2/3 vs 1/3** → 新 ADD_PUNCT 那句话对「无序枚举→列表」
  没有稳定可测的影响，**不构成 IN-D 失败的原因**。
- **IN-D 在 G1/G2 均 0/3** → IN-D 的失败与 ADD_PUNCT 版本无关（新旧都失败）。
- **判定表命中第 4 行**：`不再 3/3 | 任意 | 任意` → **模型侧已漂移是首要结论，其余组只作参考**。
  不能把责任归给 ADD_PUNCT。

## 七、任务书 7.3 修订版 ⑥ 附带结论（按要求写入）

- 08-03 阶段六实验的提示词（`harness_025f`）**不含场景块、且部件顺序与生产不同**（WORDBOOK 在
  SUGGESTION 之后，无 EXTRA，无 F4）。它「deepseek-v4-pro 3/3 成功」的结论，严格说只对那个手工
  提示词成立，对生产未必成立。**本次 G1 实测：生产真实提示词（含 F4 + EXTRA + 新 ADD_PUNCT）在
  IN-B 上恰好 3/3** —— 说明 08-03 结论的外部效度有限，本次实验正是补这个洞。

## 八、没有验证的部分（明示）

1. **生产真实输入的完整 22055 字节不可逐字还原**：`debug.log` 只打印 system_prompt 前 200 字符
   （`src/llm/mod.rs:810` `.take(200)`），无法拿到生产当时发送的逐字全文；22055 只能作为长度锚点。
2. **05:41:45Z 时刻 wordbook 的词元逐字内容未归档**：仅能靠 14:01 的 `wordbook.sqlite` 还原 15 条，
   存在轻微字节出入的可能（已计为偏差来源）。
3. **未做随机性控制之外的重复放大**：G1/G2 各 9 次、G0 6 次，IN-C 的 2/3 vs 1/3 差异在统计上
   不足以断言 ADD_PUNCT 有反向作用；只断言「无稳定可测影响」。
4. **finish_reason / token 明细**在 JSON 中可查，本报告只贴了代表性质输出。
5. 运行期间 `git status -- src/` 应保持干净（本任务零生产代码改动）。

## 九、产物

- 脚本：`collab/research/harness_033_addpunct_ab.py`
- 组件快照：`collab/research/ab033-components.json`
- 结果全文：`collab/research/ab033-results.json`（24 条含原始输出）
