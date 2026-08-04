#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
ITN-V2-006-B-R2 语法族全量扫描工具（反向分组算法）

算法（主控定）：
1. 汇总五个保护分组全部词条（unit_collisions + proper_nouns + idioms
   + historical + function_words），不预设尾串是什么
2. 对每个「以中文数字开头」的词：剥掉首字数字 → 得到「尾串」
3. 按尾串分组 → 家族
4. 对每个家族，统计 11 个数字（一二三四五六七八九十两）中哪些在表、哪些缺
5. 在表 ≥2 且未覆盖全部 11 个 → 随机子集候选

输出：stdout（重定向到报告文件）。本脚本只产出数据，分类在报告中手工标注。
"""

import re
import codecs
import sys
import io
import os
from collections import defaultdict

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

TOML_PATH = os.path.join(os.path.dirname(__file__), '..', '..', 'itn-rules.toml')

with codecs.open(TOML_PATH, 'r', 'utf-8') as f:
    content = f.read()


def extract_block_words(block_path):
    """提取 [protect.<block>] 下的 words 列表"""
    pat = r'\[protect\.' + re.escape(block_path) + r'\]([\s\S]*?)(?=\n\[|\Z)'
    m = re.search(pat, content)
    if not m:
        return set()
    words = set()
    for wm in re.finditer(r'words\s*=\s*\[(.*?)\]', m.group(1), re.S):
        for w in re.findall(r'"([^"]+)"', wm.group(1)):
            words.add(w)
    return words


# 数据加载
uc = extract_block_words('unit_collisions')
pn = extract_block_words('proper_nouns')
idioms = extract_block_words('idioms')
historical = extract_block_words('historical')
function_words = extract_block_words('function_words')
all_protect = uc | pn | idioms | historical | function_words

DIGITS = ['一', '二', '三', '四', '五', '六', '七', '八', '九', '十', '两']
DIGIT_SET = set(DIGITS)

# 记录每个词属于哪个保护分组
word_to_groups = defaultdict(set)
for w in uc: word_to_groups[w].add('uc')
for w in pn: word_to_groups[w].add('pn')
for w in idioms: word_to_groups[w].add('idioms')
for w in historical: word_to_groups[w].add('hist')
for w in function_words: word_to_groups[w].add('fw')

# 反向分组
families = defaultdict(lambda: defaultdict(list))
for word in all_protect:
    if not word or word[0] not in DIGIT_SET:
        continue
    tail = word[1:]
    families[tail][word[0]].append(word)

# 随机子集候选
random_candidates = []
for tail, dm in families.items():
    in_table = set(dm.keys())
    cnt = len(in_table)
    if cnt >= 2 and cnt < 11:
        not_in_table = [d for d in DIGITS if d not in in_table]
        random_candidates.append((tail, dm, in_table, not_in_table))

random_candidates.sort(key=lambda x: (-len(x[2]), x[0]))

# 输出纯数据表
print(f"# 数据统计：uc={len(uc)} pn={len(pn)} idioms={len(idioms)} hist={len(historical)} fw={len(function_words)} 五组合计={len(all_protect)}")
print(f"# 家族总数={len(families)} 随机子集候选={len(random_candidates)}")
print()
print("| # | 尾串 | 在表(数) | 在表数字 | 缺失数字 | 示例词(分组) |")
print("| --- | --- | --- | --- | --- | --- |")

for idx, (tail, dm, in_table, not_in_table) in enumerate(random_candidates, 1):
    in_str = ','.join(sorted(in_table, key=lambda d: DIGITS.index(d)))
    not_str = ','.join(not_in_table)
    sample_digit = sorted(in_table, key=lambda d: DIGITS.index(d))[0]
    sample_word = dm[sample_digit][0]
    groups = ','.join(sorted(word_to_groups.get(sample_word, set())))
    print(f"| {idx} | {tail} | {len(in_table)} | {in_str} | {not_str} | {sample_word}({groups}) |")