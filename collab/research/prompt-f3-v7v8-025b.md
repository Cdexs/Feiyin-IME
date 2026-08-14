# PROMPT-ARCH-025 阶段二 · V7/V8 实验记录

## 实验目标
测试两个候选修法（V7：保留标记词；V8：显式授权混合结构）对 IN-B/IN-C 的效果，并验证反向对照。

## 实验条件
- 输入：verbatim-inputs-025b.json（逐字原文，含全部口水词和结尾句）
- prompt：当前 HEAD（与阶段一 V0 相同结构）
- API：deepseek-v4-flash，temperature=0.3，max_tokens=512
- 包装：<speech>{verbatim}</speech>

## 完整矩阵

### Task B：IN-B/IN-C 候选修法

| 变体 | IN-B（142字） | IN-C（115字） | 结论 |
|------|--------------|--------------|------|
| baseline | 0/3 para | 0/3 para | — |
| V7（保留标记词） | 0/3 para | **1/3 bullet**, 2/3 para | 偶发不稳定 |
| V8（混合结构授权） | 0/3 para | 0/3 para | 无效 |
| V7+V8 | 0/3 para | **1/3 bullet**, 2/3 para | 与 V7 单独同 |

### Task C：反向对照

| 变体 | C1（有序） | C2（单例） | C3（短项） |
|------|-----------|-----------|-----------|
| baseline | 3/3 ordered | 3/3 para | 3/3 inline |
| V7 | 3/3 ordered | 3/3 para | 3/3 inline |
| V8 | 3/3 ordered | 3/3 para | 3/3 inline |
| V7+V8 | 3/3 ordered | 3/3 para | 3/3 inline |

## 结论

1. **V7 偶发有效**：IN-C 在 V7 下 1/3 出 bullet（baseline 0/3），但远未达到 2/3 稳定标准。
2. **V8 无效**：混合结构授权未改变任何结果。
3. **无死方案**：所有变体均通过 C1/C2/C3 反向对照。
4. **真正区分特征**：IN-B/C 的独立总结句（"上面的现象都是不好的..."/"上面这些都需要注意"）导致 LLM 判定为非纯枚举，退回到 paragraph。

## API 原始数据

`prompt-f3-v7v8-025b.json`（同目录）
