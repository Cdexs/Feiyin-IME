#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Phase 2 Task B+C: V7/V8 candidate fixes on verbatim inputs.
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

# ── Prompt builders ─────────────────────────────────────────────
def build_baseline():
    """Current HEAD prompt (V9-full minus scene_f4, as Task B uses stage-1 harness style)"""
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

def build_v7():
    """V7: Keep marker words in Chinese unordered few-shot expected output"""
    f3 = PARTS["F3_TRUE"]
    old = '- Chinese unordered (markers DIFFER): "比如有些学生头发过长，再比如还有些学生奇装异服，还有些学生说脏话" → "- 有些学生头发过长\\n- 还有些学生奇装异服\\n- 还有些学生说脏话".'
    new = '- Chinese unordered (markers DIFFER): "比如有些学生头发过长，再比如还有些学生奇装异服，还有些学生说脏话" → "- 比如有些学生头发过长\\n- 再比如还有些学生奇装异服\\n- 还有些学生说脏话".'
    f3_v7 = f3.replace(old, new)
    if f3_v7 == f3:
        print("WARNING: V7 replacement failed - old text not found", file=sys.stderr)
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
        f3_v7,
    ]
    return "\n\n".join(sections)

def build_v8():
    """V8: Explicitly authorize mixed intro+list+closing structure"""
    f3 = PARTS["F3_TRUE"]
    # Insert after F3-item form section, before F3a
    insert_after = (
        'Mixed short/long: follow the MAJORITY form; if still uncertain, use a list (lists are '\
        'safer for long items). Ordered enumeration with short items: join INLINE but KEEP the '\
        'sequence markers (e.g., "第一个土豆，第二个西瓜" stays inline with 第一个/第二个).\\'
    )
    insert_text = (
        "\nMixed Structure: When the speech contains a narrative frame (introduction, summary, "
        "or conclusion sentence) surrounding a parallel enumeration, you MUST preserve the frame "
        "sentences as normal prose paragraphs and format ONLY the parallel enumeration items as "
        "a list. The correct output form is: prose paragraph → list lines → prose paragraph. "
        "DO NOT delete the introduction or conclusion merely because a list appears in the middle."
    )
    f3_v8 = f3.replace(insert_after, insert_after + insert_text)
    if f3_v8 == f3:
        print("WARNING: V8 insertion failed - anchor not found", file=sys.stderr)
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
        f3_v8,
    ]
    return "\n\n".join(sections)

def build_v7v8():
    """V7+V8 combined"""
    f3 = PARTS["F3_TRUE"]
    # V7 replacement
    old = '- Chinese unordered (markers DIFFER): "比如有些学生头发过长，再比如还有些学生奇装异服，还有些学生说脏话" → "- 有些学生头发过长\\n- 还有些学生奇装异服\\n- 还有些学生说脏话".'
    new = '- Chinese unordered (markers DIFFER): "比如有些学生头发过长，再比如还有些学生奇装异服，还有些学生说脏话" → "- 比如有些学生头发过长\\n- 再比如还有些学生奇装异服\\n- 还有些学生说脏话".'
    f3 = f3.replace(old, new)
    # V8 insertion
    insert_after = (
        'Mixed short/long: follow the MAJORITY form; if still uncertain, use a list (lists are '\
        'safer for long items). Ordered enumeration with short items: join INLINE but KEEP the '\
        'sequence markers (e.g., "第一个土豆，第二个西瓜" stays inline with 第一个/第二个).\\'
    )
    insert_text = (
        "\nMixed Structure: When the speech contains a narrative frame (introduction, summary, "
        "or conclusion sentence) surrounding a parallel enumeration, you MUST preserve the frame "
        "sentences as normal prose paragraphs and format ONLY the parallel enumeration items as "
        "a list. The correct output form is: prose paragraph → list lines → prose paragraph. "
        "DO NOT delete the introduction or conclusion merely because a list appears in the middle."
    )
    f3 = f3.replace(insert_after, insert_after + insert_text)
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
        f3,
    ]
    return "\n\n".join(sections)

# ── API caller ─────────────────────────────────────────────────
headers = {
    "Content-Type": "application/json",
    "Authorization": f"Bearer {API_KEY}",
}

def call_deepseek(system_prompt, user_text):
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
    
    bullet_count = sum(1 for l in lines if l.strip().startswith("- "))
    ordered_count = sum(1 for l in lines if re.match(r'^\d+\.\s', l.strip()))
    
    if bullet_count >= 2:
        return "bullet"
    if ordered_count >= 2:
        return "ordered"
    return "para"

def run_variant(var_name, builder, inputs, runs=3):
    sys_prompt = builder()
    print(f"\n{'='*60}")
    print(f"VARIANT {var_name} -- prompt_len={len(sys_prompt)}")
    print(f"{'='*60}")
    var_results = {}
    for inp_key, inp_data in inputs.items():
        inp_text = inp_data["verbatim"]
        run_outputs = []
        for run in range(runs):
            print(f"  [{var_name}][{inp_key}] run {run+1}/{runs}...", end=" ", flush=True)
            result = call_deepseek(sys_prompt, inp_text)
            content = result["content"].replace("\r", "")
            fr = result["finish_reason"]
            pt = result["prompt_tokens"]
            ct = result["completion_tokens"]
            cls = classify(content, inp_key)
            print(f"OK fr={fr} pt={pt} ct={ct} cls={cls}")
            # Show first 120 chars of raw
            raw_preview = content.replace("\n", "\\n")[:120]
            print(f"    RAW: {raw_preview}")
            run_outputs.append({**result, "class": cls})
            time.sleep(1.0)
        var_results[inp_key] = run_outputs
    return var_results

# ── Inputs ─────────────────────────────────────────────────────
# Task B: IN-B, IN-C
# Task C: C1, C2, C3
task_b_inputs = {k: VERBATIM[k] for k in ["IN-B", "IN-C"]}
task_c_inputs = {
    "C1": VERBATIM["C1"],
    "C2": {"verbatim": "今天雨下得很大，比如早上那阵就特别急", "len": 20},
    "C3": VERBATIM["C3"],
}

# ── Run Task B: V7, V8, V7+V8 ───────────────────────────────────
print("=" * 60)
print("TASK B: Candidate fixes on IN-B/IN-C")
print("=" * 60)

RESULTS = {}

# Baseline for reference (just IN-B/C, not full matrix)
print("\n--- Baseline (current HEAD prompt) ---")
RESULTS["baseline"] = run_variant("baseline", build_baseline, task_b_inputs, runs=3)

# V7
print("\n--- V7: Keep marker words in unordered few-shot ---")
RESULTS["V7"] = run_variant("V7", build_v7, task_b_inputs, runs=3)

# V8
print("\n--- V8: Explicit mixed-structure authorization ---")
RESULTS["V8"] = run_variant("V8", build_v8, task_b_inputs, runs=3)

# Check if either V7 or V8 helped IN-B or IN-C
v7_bullet = sum(1 for r in RESULTS["V7"]["IN-B"] if r["class"] == "bullet")
v7_c_bullet = sum(1 for r in RESULTS["V7"]["IN-C"] if r["class"] == "bullet")
v8_bullet = sum(1 for r in RESULTS["V8"]["IN-B"] if r["class"] == "bullet")
v8_c_bullet = sum(1 for r in RESULTS["V8"]["IN-C"] if r["class"] == "bullet")

print(f"\n=== TASK B SUMMARY ===")
print(f"Baseline: IN-B={sum(1 for r in RESULTS['baseline']['IN-B'] if r['class']=='bullet')}/3 bullet, IN-C={sum(1 for r in RESULTS['baseline']['IN-C'] if r['class']=='bullet')}/3 bullet")
print(f"V7:       IN-B={v7_bullet}/3 bullet, IN-C={v7_c_bullet}/3 bullet")
print(f"V8:       IN-B={v8_bullet}/3 bullet, IN-C={v8_c_bullet}/3 bullet")

# If either helped, run V7+V8 combo
if v7_bullet >= 1 or v7_c_bullet >= 1 or v8_bullet >= 1 or v8_c_bullet >= 1:
    print("\n--- V7+V8: Combined ---")
    RESULTS["V7+V8"] = run_variant("V7+V8", build_v7v8, task_b_inputs, runs=3)

# ── Run Task C: Reverse controls for ALL variants that showed promise ──
print("\n" + "=" * 60)
print("TASK C: Reverse controls")
print("=" * 60)

# Determine which variants need Task C
variants_to_test_c = []
if "baseline" in RESULTS:
    variants_to_test_c.append(("baseline", build_baseline))
if "V7" in RESULTS:
    variants_to_test_c.append(("V7", build_v7))
if "V8" in RESULTS:
    variants_to_test_c.append(("V8", build_v8))
if "V7+V8" in RESULTS:
    variants_to_test_c.append(("V7+V8", build_v7v8))

for var_name, builder in variants_to_test_c:
    print(f"\n--- {var_name}: C1/C2/C3 ---")
    c_results = run_variant(var_name + "-C", builder, task_c_inputs, runs=3)
    RESULTS[var_name + "_C"] = c_results

# Save
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-v7v8-025b.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump(RESULTS, f, ensure_ascii=False, indent=2)
print(f"\n\nSaved results to: {out_path}")

# Summary
print("\n" + "=" * 60)
print("FINAL SUMMARY")
print("=" * 60)
for var_name in ["baseline", "V7", "V8", "V7+V8"]:
    if var_name not in RESULTS:
        continue
    data = RESULTS[var_name]
    print(f"\n{var_name} (Task B):")
    for k in ["IN-B", "IN-C"]:
        if k in data:
            bullet = sum(1 for r in data[k] if r["class"] == "bullet")
            ordered = sum(1 for r in data[k] if r["class"] == "ordered")
            para = sum(1 for r in data[k] if r["class"] == "para")
            inline = sum(1 for r in data[k] if r["class"] == "inline")
            status = "✅ LIST" if bullet >= 2 else "❌ PARA"
            print(f"  {k}: {bullet}b/{ordered}o/{para}p/{inline}i {status}")
    
    c_key = var_name + "_C"
    if c_key in RESULTS:
        c_data = RESULTS[c_key]
        print(f"{var_name} (Task C):")
        for k in ["C1", "C2", "C3"]:
            if k in c_data:
                bullet = sum(1 for r in c_data[k] if r["class"] == "bullet")
                ordered = sum(1 for r in c_data[k] if r["class"] == "ordered")
                para = sum(1 for r in c_data[k] if r["class"] == "para")
                inline = sum(1 for r in c_data[k] if r["class"] == "inline")
                if k == "C1":
                    status = "✅ ORDERED" if ordered >= 2 else "❌ FAIL"
                elif k == "C2":
                    status = "✅ PARA" if para >= 2 else "❌ FAIL"
                elif k == "C3":
                    status = "✅ INLINE" if inline >= 2 else "❌ FAIL"
                print(f"  {k}: {bullet}b/{ordered}o/{para}p/{inline}i {status}")
