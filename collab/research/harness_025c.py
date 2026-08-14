#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Phase 3: Input-side bisection (fixed prompt, varying input)
30 API calls budget. IN-B B0-B6 (7 groups x3), IN-C C0-C1' (2 groups x3) = 27 calls.
"""
import json
import tomllib
import urllib.request
import time
import sys
import re

CONFIG_PATH = r"D:\Workspace\CodeLab\voice-ime\target\release\config.toml"
PARTS_PATH = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-parts-025-full.json"
VERBATIM_PATH = r"D:\Workspace\CodeLab\voice-ime\collab\research\verbatim-inputs-025b.json"

with open(CONFIG_PATH, "rb") as f:
    cfg = tomllib.load(f)
llm = cfg["llm"]
API_KEY = llm["api_key"]
API_URL = llm["api_url"].rstrip("/") + "/chat/completions"
MODEL = llm["model"]

with open(PARTS_PATH, "r", encoding="utf-8") as f:
    PARTS = json.load(f)
with open(VERBATIM_PATH, "r", encoding="utf-8") as f:
    VERBATIM = json.load(f)

PARTS["BASE_PROMPT"] = cfg["llm"]["system_prompt"].strip()

# Build baseline system prompt (same as phase 2 baseline)
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

SYS_PROMPT = build_sys_prompt()
print(f"SYS prompt_len={len(SYS_PROMPT)}")

# ── Input variants ─────────────────────────────────────────────
IN_B = VERBATIM["IN-B"]["verbatim"]
IN_C = VERBATIM["IN-C"]["verbatim"]

# B0: original
B0 = IN_B

# B1: delete closing summary sentence only
# "啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正。"
B1 = IN_B.replace(
    "啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正。", ""
)

# B2: delete opening intro sentence only
# "今天的讲话呢是要指出我们现在学校里面出现的一些现象。"
B2 = IN_B.replace(
    "今天的讲话呢是要指出我们现在学校里面出现的一些现象。", ""
)

# B3: delete both intro and closing, keep three items only
B3_raw = IN_B.replace(
    "今天的讲话呢是要指出我们现在学校里面出现的一些现象。", ""
).replace(
    "啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正。", ""
)
B3 = B3_raw.strip()

# B4: change three unordered markers to ordered markers, keep everything else
# 比如说→第一， 还有啊，比如→第二， 呃，比如还有啊→第三
B4 = IN_B.replace(
    "啊，比如说有些同学啊下课之后徘徊在校园里不走，三五成群。",
    "啊，第一有些同学啊下课之后徘徊在校园里不走，三五成群。"
).replace(
    "还有啊，比如有的同学烫发、染黄头发、染头发，这个不符合校规。",
    "第二有的同学烫发、染黄头发、染头发，这个不符合校规。"
).replace(
    "呃，比如还有啊，有的同学不遵守课堂纪律，啊，顶撞老师。",
    "第三有的同学不遵守课堂纪律，啊，顶撞老师。"
)

# B5: remove filler words (啊/呃/呢/嗯) only, keep structure
# Need careful replacement to preserve sentence structure
B5 = (IN_B
    .replace("呢", "")
    .replace("啊，", "，")
    .replace("啊", "")
    .replace("呃，", "，")
    .replace("呃", "")
    .replace("，，", "，")
    .replace("，，", "，")
)

# B6: shorten each of the three items by ~50%, keep intro+closing
# Item 1: "有些同学啊下课之后徘徊在校园里不走，三五成群" → "有些同学下课徘徊"
# Item 2: "有的同学烫发、染黄头发、染头发，这个不符合校规" → "有的同学烫发"
# Item 3: "有的同学不遵守课堂纪律，啊，顶撞老师" → "有的同学不遵守纪律"
B6 = (IN_B
    .replace(
        "啊，比如说有些同学啊下课之后徘徊在校园里不走，三五成群。",
        "啊，比如说有些同学下课徘徊。"
    )
    .replace(
        "还有啊，比如有的同学烫发、染黄头发、染头发，这个不符合校规。",
        "还有啊，比如有的同学烫发。"
    )
    .replace(
        "呃，比如还有啊，有的同学不遵守课堂纪律，啊，顶撞老师。",
        "呃，比如还有啊，有的同学不遵守纪律。"
    )
)

# IN-C variants
C0 = IN_C

# C1': delete closing summary only
# "上面这些啊都需要注意。"
C1_prime = IN_C.replace(
    "上面这些啊都需要注意。", ""
)

INPUT_VARIANTS = {
    # IN-B variants
    "B0": B0,
    "B1": B1,
    "B2": B2,
    "B3": B3,
    "B4": B4,
    "B5": B5,
    "B6": B6,
    # IN-C variants
    "C0": C0,
    "C1_prime": C1_prime,
}

# Show lengths
for k, v in INPUT_VARIANTS.items():
    print(f"  {k}: {len(v)} chars")

# ── API caller ─────────────────────────────────────────────────
headers = {
    "Content-Type": "application/json",
    "Authorization": f"Bearer {API_KEY}",
}

call_count = 0

def call_deepseek(user_text):
    global call_count
    call_count += 1
    body = json.dumps({
        "model": MODEL,
        "messages": [
            {"role": "system", "content": SYS_PROMPT},
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

def classify(text):
    t = text.strip()
    lines = [l for l in t.splitlines() if l.strip()]
    bullet = sum(1 for l in lines if l.strip().startswith("- "))
    if bullet >= 2:
        return "bullet"
    ordered = sum(1 for l in lines if re.match(r'^\d+\.\s', l.strip()))
    if ordered >= 2:
        return "ordered"
    return "para"

# ── Runner ─────────────────────────────────────────────────────
RESULTS = {}

def run_group(group_key, text, runs=3):
    print(f"\n{'='*60}")
    print(f"GROUP {group_key} ({len(text)} chars)")
    print(f"TEXT: {text[:80]}...")
    print(f"{'='*60}")
    outputs = []
    for i in range(runs):
        print(f"  run {i+1}/{runs}...", end=" ", flush=True)
        r = call_deepseek(text)
        cls = classify(r["content"])
        print(f"pt={r['prompt_tokens']} ct={r['completion_tokens']} cls={cls}")
        raw_preview = r["content"].replace("\n", "\\n")[:120]
        print(f"    RAW: {raw_preview}")
        outputs.append({**r, "class": cls})
        time.sleep(1.0)
    RESULTS[group_key] = outputs
    bullet_runs = sum(1 for r in outputs if r["class"] == "bullet")
    print(f"  → {bullet_runs}/3 bullet")

# Run IN-B variants B0-B6
for key in ["B0", "B1", "B2", "B3", "B4", "B5", "B6"]:
    run_group(key, INPUT_VARIANTS[key], runs=3)

# Run IN-C variants C0, C1_prime
for key in ["C0", "C1_prime"]:
    run_group(key, INPUT_VARIANTS[key], runs=3)

print(f"\n\nTotal API calls: {call_count}")

# Save
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-input-bisect-025c.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump({
        "sys_prompt_len": len(SYS_PROMPT),
        "call_count": call_count,
        "inputs": {k: v for k, v in INPUT_VARIANTS.items()},
        "results": RESULTS,
    }, f, ensure_ascii=False, indent=2)
print(f"Saved to: {out_path}")

# Summary
print("\n" + "="*60)
print("SUMMARY")
print("="*60)
for key in ["B0", "B1", "B2", "B3", "B4", "B5", "B6", "C0", "C1_prime"]:
    if key not in RESULTS:
        continue
    bullet = sum(1 for r in RESULTS[key] if r["class"] == "bullet")
    ordered = sum(1 for r in RESULTS[key] if r["class"] == "ordered")
    para = sum(1 for r in RESULTS[key] if r["class"] == "para")
    print(f"  {key}: {bullet}b/{ordered}o/{para}p", end="")
    if bullet >= 2:
        print("  ✅ LIST")
    elif ordered >= 2:
        print("  ✅ ORDERED")
    else:
        print("  ❌ PARA")
