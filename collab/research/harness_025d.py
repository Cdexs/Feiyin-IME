#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Phase 4: Item-content bisection (fixed prompt, varying input items)
B9 / B10 / B8, conditional B11 if B9 succeeds.
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

# Build baseline system prompt
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
# B9: IN-A items + IN-B frame + IN-A markers
B9 = (
    "今天的讲话呢是要指出我们现在学校里面出现的一些现象。"
    "啊，比如嗯英语学习要多读多背，"
    "再比如啊多听一些视频的节目，"
    "还有比如就是要呃多出去和别人交流。"
    "啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正。"
)

# B10: IN-B items rewritten without internal commas/duns, IN-B frame/markers kept
B10 = (
    "今天的讲话呢是要指出我们现在学校里面出现的一些现象。"
    "啊，比如说有些同学下课之后在校园里徘徊不走。"
    "还有啊，比如有的同学烫发染发不符合校规。"
    "呃，比如还有啊，有的同学不遵守课堂纪律顶撞老师。"
    "啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正。"
)

# B8: IN-B items shortened + no frame (B3 + B6 combo)
B8 = (
    "啊，比如说有些同学下课徘徊。"
    "还有啊，比如有的同学烫发。"
    "呃，比如还有啊，有的同学不遵守纪律。"
)

# B11 (conditional): IN-B items + IN-A frame
B11 = (
    "建议啊从以下方面入手啊，"
    "啊，比如说有些同学啊下课之后徘徊在校园里不走，三五成群。"
    "还有啊，比如有的同学烫发、染黄头发、染头发，这个不符合校规。"
    "呃，比如还有啊，有的同学不遵守课堂纪律，啊，顶撞老师。"
)

INPUTS = {
    "B9": B9,
    "B10": B10,
    "B8": B8,
}

for k, v in INPUTS.items():
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
    print(f"TEXT: {text}")
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
    ordered_runs = sum(1 for r in outputs if r["class"] == "ordered")
    para_runs = sum(1 for r in outputs if r["class"] == "para")
    print(f"  → {bullet_runs}b/{ordered_runs}o/{para_runs}p")
    return bullet_runs >= 2 or ordered_runs >= 2

# Run B9, B10, B8
b9_success = run_group("B9", INPUTS["B9"], runs=3)
run_group("B10", INPUTS["B10"], runs=3)
run_group("B8", INPUTS["B8"], runs=3)

# Conditional B11
if b9_success:
    print(f"\n[B9 succeeded → running B11 conditional]")
    run_group("B11", B11, runs=3)

print(f"\n\nTotal API calls: {call_count}")

# Save
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-item-bisect-025d.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump({
        "sys_prompt_len": len(SYS_PROMPT),
        "call_count": call_count,
        "inputs": {**INPUTS, **({"B11": B11} if "B11" in RESULTS else {})},
        "results": RESULTS,
    }, f, ensure_ascii=False, indent=2)
print(f"Saved to: {out_path}")

# Summary
print("\n" + "="*60)
print("SUMMARY")
print("="*60)
for key in ["B9", "B10", "B8", "B11"]:
    if key not in RESULTS:
        continue
    bullet = sum(1 for r in RESULTS[key] if r["class"] == "bullet")
    ordered = sum(1 for r in RESULTS[key] if r["class"] == "ordered")
    para = sum(1 for r in RESULTS[key] if r["class"] == "para")
    print(f"  {key}: {bullet}b/{ordered}o/{para}p", end="")
    if bullet >= 2:
        print("  [LIST]")
    elif ordered >= 2:
        print("  [ORDERED]")
    else:
        print("  [PARA]")
