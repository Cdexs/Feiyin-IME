#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Phase 5: Verify fix candidates V10/V11/V12
63 API calls: 3 variants x 7 inputs x 3 runs each.
"""
import json
import tomllib
import urllib.request
import time
import sys
import re

CONFIG_PATH = r"D:\Workspace\CodeLab\voice-ime\target\release\config.toml"
PARTS_PATH = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-parts-025-full.json"

with open(CONFIG_PATH, "rb") as f:
    cfg = tomllib.load(f)
llm = cfg["llm"]
API_KEY = llm["api_key"]
API_URL = llm["api_url"].rstrip("/") + "/chat/completions"
MODEL = llm["model"]

with open(PARTS_PATH, "r", encoding="utf-8") as f:
    PARTS = json.load(f)
PARTS["BASE_PROMPT"] = cfg["llm"]["system_prompt"].strip()

# ── Build baseline system prompt ────────────────────────────────
def build_sys_prompt():
    sections = [
        PARTS["META"],
        PARTS["L0_1"], PARTS["L0_2"], PARTS["L0_3"], PARTS["L0_4"],
        PARTS["CONTRACT_TRUE"],
        PARTS["USER_PREFS"],
        PARTS["BASE_PROMPT"],
        PARTS["F1F2"],
        PARTS["CODESWITCH"],
        PARTS["UNIT_SYMBOL"],
        PARTS["ADD_PUNCT"],
        PARTS["SUGGESTION"],
        PARTS["WORDBOOK"],
        PARTS["F3_TRUE"],
    ]
    return "\n\n".join(sections)

BASE_SYS = build_sys_prompt()
print(f"BASE SYS prompt_len={len(BASE_SYS)}")

# ── V10: Append semantic rule after DECISION RULE ──────────────
V10_APPEND = (
    "\nAdditional Semantic Rule (DEC-039, symmetric): "
    "1) An enumeration does NOT require a预告短语 (e.g., \"from the following aspects\", \"there are several points\"). "
    "2) A narrative or descriptive introductory sentence does NOT reduce the validity of a subsequent enumeration "
    "— the intro merely sets the topic; the parallel items that follow are still an enumeration if they meet the DECISION RULE. "
    "3) The sole decisive test remains: whether TWO OR MORE spans stand in a PARALLEL relation — same syntactic slot, same semantic function, "
    "each contributing one coordinate member to a set. Preserve this test regardless of the surrounding discourse type."
)

# Insert after "However, failing to list a genuine parallel enumeration..."
V10_SYS = BASE_SYS.replace(
    "However, failing to list a genuine parallel enumeration (2+ distinct items) is ALSO a \\n        regression — both directions are equally wrong.\\",
    "However, failing to list a genuine parallel enumeration (2+ distinct items) is ALSO a \\n        regression — both directions are equally wrong.\\" + V10_APPEND
)

# ── V11: Append narrative+few-shot to F3c ─────────────────────
V11_APPEND = (
    "\n- Chinese unordered with narrative intro (markers DIFFER): "
    "\"今天的讲话指出了学校的一些现象。比如说有些同学下课之后徘徊在校园里不走，三五成群；"
    "再比如有的同学烫发、染黄头发、染头发，这个不符合校规；"
    "还有的同学不遵守课堂纪律，顶撞老师。\" → "
    "\"- 有些同学下课之后徘徊在校园里不走，三五成群\\n"
    "- 有的同学烫发、染黄头发、染头发，这个不符合校规\\n"
    "- 有的同学不遵守课堂纪律，顶撞老师\"."
)

# Insert after the last Chinese unordered example
V11_SYS = BASE_SYS.replace(
    '- Chinese SHORT items inline (enumeration confirmed but NO list): "今天出去买菜了，买了3斤土豆，一个西瓜，20斤大米，还有3斤香蕉" → "今天出去买菜了，买了3斤土豆、一个西瓜、20斤大米、还有3斤香蕉".\\',
    '- Chinese SHORT items inline (enumeration confirmed but NO list): "今天出去买菜了，买了3斤土豆，一个西瓜，20斤大米，还有3斤香蕉" → "今天出去买菜了，买了3斤土豆、一个西瓜、20斤大米、还有3斤香蕉".\\' + V11_APPEND
)

if V11_SYS == BASE_SYS:
    print("WARNING: V11 insertion failed", file=sys.stderr)

# ── V12: V10 + V11 ────────────────────────────────────────────
V12_SYS = V10_SYS.replace(
    '- Chinese SHORT items inline (enumeration confirmed but NO list): "今天出去买菜了，买了3斤土豆，一个西瓜，20斤大米，还有3斤香蕉" → "今天出去买菜了，买了3斤土豆、一个西瓜、20斤大米、还有3斤香蕉".\\',
    '- Chinese SHORT items inline (enumeration confirmed but NO list): "今天出去买菜了，买了3斤土豆，一个西瓜，20斤大米，还有3斤香蕉" → "今天出去买菜了，买了3斤土豆、一个西瓜、20斤大米、还有3斤香蕉".\\' + V11_APPEND
)

if V12_SYS == V10_SYS:
    print("WARNING: V12 insertion failed", file=sys.stderr)

VARIANTS = {
    "V10": V10_SYS,
    "V11": V11_SYS,
    "V12": V12_SYS,
}

for name, sys_p in VARIANTS.items():
    print(f"  {name} prompt_len={len(sys_p)}")

# ── Inputs ──────────────────────────────────────────────────────
# C4 is new for this phase: narrative intro + single item, no enumeration
C4 = "今天的讲话呢是要指出我们现在学校里面出现的一些现象，就是有些同学不遵守课堂纪律，这个问题很严重，希望大家注意。"

INPUTS = {
    "IN-A": "建议啊从以下方面入手啊，比如嗯英语学习要多读多背，再比如啊多听一些视频的节目，还有比如就是要呃多出去和别人交流，增长自己的口语能力。",
    "IN-B": "今天的讲话呢是要指出我们现在学校里面出现的一些现象。啊，比如说有些同学啊下课之后徘徊在校园里不走，三五成群。还有啊，比如有的同学烫发、染黄头发、染头发，这个不符合校规。呃，比如还有啊，有的同学不遵守课堂纪律，啊，顶撞老师。啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正。",
    "IN-C": "我现在就要说出我们公司的一些不好的现象，比如有的同事迟到早退，啊，不遵守公司的规章制度，啊，比如还有啊，有的上班时间摸鱼玩手机，还有的同事呢就是，嗯，工作的这个成果呀不及时提交，不及时审核，造成了一定的损失，上面这些啊都需要注意。",
    "C1": "现在有三点现象要注意，第一点就是程序会崩溃崩溃退出，第二点就是会输出的字符都是乱字符，第三点就是呃运行一段时间之后就会异常报错。",
    "C2": "今天雨下得很大，比如早上那阵就特别急",
    "C3": "今天到菜场去买菜，买了好多菜，然后三斤土豆，嗯，两斤八两的番茄，还有一斤半的西西瓜，还有八斤的土豆。",
    "C4": C4,
}

# ── API caller ─────────────────────────────────────────────────
headers = {
    "Content-Type": "application/json",
    "Authorization": f"Bearer {API_KEY}",
}

call_count = 0

def call_deepseek(system_prompt, user_text):
    global call_count
    call_count += 1
    body = json.dumps({
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": f"<speech>{user_text}</speech>"},
        ],
        "temperature": 0.3,
        "max_tokens": 512,
        "stream": False,
        "thinking": {"type": "disabled"},
    }, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(API_URL, data=body, headers=headers, method="POST")
    with urllib.request.urlopen(req, timeout=60) as resp:
        data = json.loads(resp.read().decode("utf-8"))
        choice = data.get("choices", [{}])[0]
        msg = choice.get("message", {})
        return {
            "content": msg.get("content", ""),
            "finish_reason": choice.get("finish_reason", ""),
            "prompt_tokens": data.get("usage", {}).get("prompt_tokens", 0),
            "completion_tokens": data.get("usage", {}).get("completion_tokens", 0),
        }

def classify(text, inp_key):
    t = text.strip()
    lines = [l for l in t.splitlines() if l.strip()]
    
    if inp_key == "C2":
        return "para" if len(lines) <= 1 else "list"
    if inp_key == "C3":
        has_bullet = any(l.strip().startswith("- ") for l in lines)
        return "bullet" if has_bullet else "inline"
    if inp_key == "C4":
        return "para" if len(lines) <= 1 else "list"
    
    bullet_count = sum(1 for l in lines if l.strip().startswith("- "))
    ordered_count = sum(1 for l in lines if re.match(r'^\d+\.\s', l.strip()))
    
    if bullet_count >= 2:
        return "bullet"
    if ordered_count >= 2:
        return "ordered"
    return "para"

# ── Runner ─────────────────────────────────────────────────────
RESULTS = {}

def run_variant(var_name, sys_prompt, inputs, runs=3):
    print(f"\n{'='*60}")
    print(f"VARIANT {var_name} -- prompt_len={len(sys_prompt)}")
    print(f"{'='*60}")
    var_results = {}
    for inp_key, inp_text in inputs.items():
        run_outputs = []
        for run in range(runs):
            print(f"  [{var_name}][{inp_key}] run {run+1}/{runs}...", end=" ", flush=True)
            result = call_deepseek(sys_prompt, inp_text)
            if "error" in result:
                print(f"ERROR: {result['error'][:80]}")
                run_outputs.append(result)
                break
            content = result["content"].replace("\r", "")
            fr = result["finish_reason"]
            pt = result["prompt_tokens"]
            ct = result["completion_tokens"]
            cls = classify(content, inp_key)
            print(f"OK fr={fr} pt={pt} ct={ct} cls={cls}")
            raw_preview = content.replace("\n", "\\n")[:120]
            print(f"    RAW: {raw_preview}")
            run_outputs.append({**result, "class": cls})
            time.sleep(1.0)
        var_results[inp_key] = run_outputs
    RESULTS[var_name] = var_results
    return var_results

# Run all variants
for var_name, sys_prompt in VARIANTS.items():
    run_variant(var_name, sys_prompt, INPUTS, runs=3)

print(f"\n\nTotal API calls: {call_count}")

# Save
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-fix-verify-025e.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump({
        "call_count": call_count,
        "prompts_len": {k: len(v) for k, v in VARIANTS.items()},
        "v10_append": V10_APPEND,
        "v11_append": V11_APPEND,
        "results": RESULTS,
    }, f, ensure_ascii=False, indent=2)
print(f"Saved to: {out_path}")

# Summary
print("\n" + "="*60)
print("SUMMARY")
print("="*60)
for var_name in ["V10", "V11", "V12"]:
    print(f"\n{var_name}:")
    data = RESULTS[var_name]
    for inp_key in ["IN-A", "IN-B", "IN-C", "C1", "C2", "C3", "C4"]:
        if inp_key not in data:
            continue
        bullet = sum(1 for r in data[inp_key] if r["class"] == "bullet")
        ordered = sum(1 for r in data[inp_key] if r["class"] == "ordered")
        para = sum(1 for r in data[inp_key] if r["class"] == "para")
        inline = sum(1 for r in data[inp_key] if r["class"] == "inline")
        
        if inp_key in ["IN-A", "IN-B", "IN-C"]:
            status = "LIST" if bullet >= 2 else ("ORDERED" if ordered >= 2 else "PARA")
            expected = "LIST"
        elif inp_key == "C1":
            status = "ORDERED" if ordered >= 2 else "FAIL"
            expected = "ORDERED"
        elif inp_key == "C2":
            status = "PARA" if para >= 2 else "FAIL"
            expected = "PARA"
        elif inp_key == "C3":
            status = "INLINE" if inline >= 2 else "FAIL"
            expected = "INLINE"
        elif inp_key == "C4":
            status = "PARA" if para >= 2 else "FAIL"
            expected = "PARA"
        
        ok = "OK" if status == expected else "DEAD"
        print(f"  {inp_key}: {bullet}b/{ordered}o/{para}p/{inline}i = {status} [{ok}]")
