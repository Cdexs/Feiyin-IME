#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Phase 2 Bisection Harness
Builds V9-full and runs bisection to locate IN-A suppressor.
"""
import json
import tomllib
import urllib.request
import time
import sys
import os
import re

CONFIG_PATH = r"D:\Workspace\CodeLab\voice-ime\target\release\config.toml"
PARTS_PATH = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-parts-025-full.json"
SCENE_F4_PATH = r"D:\Workspace\CodeLab\voice-ime\collab\research\scene_f4_document.txt"

with open(CONFIG_PATH, "rb") as f:
    cfg = tomllib.load(f)
llm = cfg["llm"]
API_KEY = llm["api_key"]
API_URL = llm["api_url"].rstrip("/") + "/chat/completions"
MODEL = llm["model"]

if not API_KEY or not API_URL or not MODEL:
    print("FATAL: missing api_key/api_url/model", file=sys.stderr)
    sys.exit(1)

with open(PARTS_PATH, "r", encoding="utf-8") as f:
    PARTS = json.load(f)

# Read actual scene_f4 from debug.log
with open(SCENE_F4_PATH, "r", encoding="utf-8") as f:
    SCENE_F4 = f.read().strip()

# Override with actual scene_f4
PARTS["SCENE_F4"] = SCENE_F4

BASE_PROMPT = cfg["llm"]["system_prompt"].strip()
PARTS["BASE_PROMPT"] = BASE_PROMPT

# Input corpus
INPUTS = {
    "IN-A": "建议啊从以下方面入手啊，比如英语学习要多读多背，再比如多听视频节目，还有比如就是多出去和别人交流",
    "IN-B": "今天的讲话指出学校出现的现象。比如说有些同学下课徘徊，还有啊，比如有的同学烫发，呃，比如还有啊，有的同学不遵守课堂纪律。上面的现象都是不好的",
    "IN-C": "我现在就要说出公司的一些不好的现象，比如有的同事迟到早退，比如还有啊，上班摸鱼玩手机，还有的同事成果不及时提交。上面这些都需要注意",
    "C1": "现在有三点现象要注意，第一点怎样怎样，第二点怎样怎样，第三点怎样怎样",
    "C2": "今天雨下得很大，比如早上那阵就特别急",
    "C3": "今天出去买菜了，买了3斤土豆，一个西瓜，20斤大米，还有3斤香蕉",
}

# Build V9-full: matches build_prompt_layers order exactly
def build_v9_full():
    """V9-full: L0 → L1 → L2 (all) → L3 (scene_f4 + f3_lists)"""
    sections = [
        PARTS["META"],
        PARTS["L0_1"],
        PARTS["L0_2"],
        PARTS["L0_3"],
        PARTS["L0_4"],
        PARTS["CONTRACT_TRUE"],  # L1
        PARTS["USER_PREFS"],
        PARTS["BASE_PROMPT"],    # user_base (L2)
        # L2 rules in push order:
        PARTS["F1F2"],
        PARTS["CODESWITCH"],
        PARTS["UNIT_SYMBOL"],
        PARTS["ADD_PUNCT"],
        PARTS["SUGGESTION"],
        PARTS["WORDBOOK"],
        # L3:
        PARTS["SCENE_F4"],
        PARTS["F3_TRUE"],
    ]
    return "\n\n".join(sections)

def build_v9_without(omit_set):
    """Build V9 with specific segments omitted"""
    sections = []
    order = [
        ("META", PARTS["META"]),
        ("L0_1", PARTS["L0_1"]),
        ("L0_2", PARTS["L0_2"]),
        ("L0_3", PARTS["L0_3"]),
        ("L0_4", PARTS["L0_4"]),
        ("CONTRACT", PARTS["CONTRACT_TRUE"]),
        ("USER_PREFS", PARTS["USER_PREFS"]),
        ("BASE", PARTS["BASE_PROMPT"]),
        ("F1F2", PARTS["F1F2"]),
        ("CODESWITCH", PARTS["CODESWITCH"]),
        ("UNIT_SYMBOL", PARTS["UNIT_SYMBOL"]),
        ("ADD_PUNCT", PARTS["ADD_PUNCT"]),
        ("SUGGESTION", PARTS["SUGGESTION"]),
        ("WORDBOOK", PARTS["WORDBOOK"]),
        ("SCENE_F4", PARTS["SCENE_F4"]),
        ("F3", PARTS["F3_TRUE"]),
    ]
    for name, text in order:
        if name not in omit_set:
            sections.append(text)
    return "\n\n".join(sections)

# Variant builders for bisection
VARIANTS = {
    "V9-full": build_v9_full,
    # A2 bisection variants
    "V9-no-format": lambda: build_v9_without({"F1F2", "SCENE_F4", "SUGGESTION"}),
    "V9-no-unrelated": lambda: build_v9_without({"WORDBOOK", "CODESWITCH", "UNIT_SYMBOL", "ADD_PUNCT"}),
    "V9-no-f1f2": lambda: build_v9_without({"F1F2"}),
    "V9-no-scene": lambda: build_v9_without({"SCENE_F4"}),
    "V9-no-suggestion": lambda: build_v9_without({"SUGGESTION"}),
    "V9-no-wordbook": lambda: build_v9_without({"WORDBOOK"}),
    "V9-no-codeswitch": lambda: build_v9_without({"CODESWITCH"}),
    "V9-no-unitsymbol": lambda: build_v9_without({"UNIT_SYMBOL"}),
    "V9-no-addpunct": lambda: build_v9_without({"ADD_PUNCT"}),
    # B: V7/V8 (modified F3)
}

# API caller
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
    try:
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
    except Exception as e:
        return {"error": str(e)[:500]}

def classify_output(text, inp_key):
    """Classify output type"""
    t = text.strip()
    lines = [l for l in t.splitlines() if l.strip()]
    
    if inp_key == "C2":
        return "para" if len(lines) <= 1 else "list"
    if inp_key == "C3":
        has_bullet = any(l.strip().startswith("- ") for l in lines)
        return "bullet" if has_bullet else "inline"
    
    # IN-A, IN-B, IN-C, C1
    bullet_count = sum(1 for l in lines if l.strip().startswith("- "))
    ordered_count = sum(1 for l in lines if re.match(r'^\d+\.\s', l.strip()))
    
    if bullet_count >= 2:
        return "bullet"
    if ordered_count >= 2:
        return "ordered"
    return "para"

# Runner
RESULTS = {}

def run_variant(var_name, builder, inputs, runs=3):
    sys_prompt = builder()
    print(f"\n{'='*60}")
    print(f"VARIANT {var_name} -- prompt_len={len(sys_prompt)}")
    print(f"{'='*60}")
    RESULTS[var_name] = {}
    for inp_key, inp_text in inputs.items():
        run_outputs = []
        for run in range(runs):
            print(f"  [{var_name}][{inp_key}] run {run+1}/{runs}...", end=" ", flush=True)
            result = call_deepseek(sys_prompt, inp_text)
            if "error" in result:
                print(f"ERROR: {result['error'][:80]}")
                run_outputs.append(result)
                break
            else:
                content = result["content"].replace("\r", "").replace("\n", "\\n")
                fr = result["finish_reason"]
                pt = result["prompt_tokens"]
                ct = result["completion_tokens"]
                cls = classify_output(result["content"], inp_key)
                print(f"OK fr={fr} pt={pt} ct={ct} cls={cls}")
                run_outputs.append({**result, "class": cls})
                time.sleep(1.0)
        RESULTS[var_name][inp_key] = run_outputs

# Run V9-full first (Task A1)
print("=== TASK A1: V9-full (reproduce production failure) ===")
run_variant("V9-full", build_v9_full, {"IN-A": INPUTS["IN-A"]}, runs=3)

# Save intermediate
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-suppressor-bisect-025b.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump(RESULTS, f, ensure_ascii=False, indent=2)
print(f"\nSaved intermediate to: {out_path}")

# Check if IN-A still bullets (unexpected - would mean suppressor not in these 8 segments)
ina_results = RESULTS.get("V9-full", {}).get("IN-A", [])
if ina_results:
    bullet_runs = sum(1 for r in ina_results if r.get("class") == "bullet")
    print(f"\n=== V9-full IN-A results: {bullet_runs}/3 bullet ===")
    if bullet_runs >= 2:
        print("WARNING: V9-full still produces bullets! Suppressor may be elsewhere.")
    else:
        print("SUCCESS: V9-full suppresses IN-A. Proceeding with bisection...")
        
        # Task A2: Bisection
        print("\n=== TASK A2: Bisection ===")
        
        # First split: format-related vs unrelated
        print("\n--- Split 1: Remove format-related (F1F2 + scene_f4 + suggestion) ---")
        run_variant("V9-no-format", lambda: build_v9_without({"F1F2", "SCENE_F4", "SUGGESTION"}), 
                   {"IN-A": INPUTS["IN-A"]}, runs=3)
        
        print("\n--- Split 2: Remove unrelated (wordbook + codeswitch + unitsymbol + addpunct) ---")
        run_variant("V9-no-unrelated", lambda: build_v9_without({"WORDBOOK", "CODESWITCH", "UNIT_SYMBOL", "ADD_PUNCT"}),
                   {"IN-A": INPUTS["IN-A"]}, runs=3)
        
        # Save after bisection
        with open(out_path, "w", encoding="utf-8") as f:
            json.dump(RESULTS, f, ensure_ascii=False, indent=2)
        
        # Determine which half contains suppressor
        no_format = RESULTS.get("V9-no-format", {}).get("IN-A", [])
        no_unrelated = RESULTS.get("V9-no-unrelated", {}).get("IN-A", [])
        
        no_format_bullets = sum(1 for r in no_format if r.get("class") == "bullet")
        no_unrelated_bullets = sum(1 for r in no_unrelated if r.get("class") == "bullet")
        
        print(f"\nV9-no-format IN-A: {no_format_bullets}/3 bullet")
        print(f"V9-no-unrelated IN-A: {no_unrelated_bullets}/3 bullet")
        
        if no_format_bullets >= 2 and no_unrelated_bullets < 2:
            print("Suppressor is in UNRELATED group (wordbook/codeswitch/unitsymbol/addpunct)")
        elif no_format_bullets < 2 and no_unrelated_bullets >= 2:
            print("Suppressor is in FORMAT-RELATED group (F1F2/scene_f4/suggestion)")
            # Drill down into format-related
            print("\n--- Drill: Remove F1F2 only ---")
            run_variant("V9-no-f1f2", lambda: build_v9_without({"F1F2"}),
                       {"IN-A": INPUTS["IN-A"]}, runs=3)
            print("\n--- Drill: Remove scene_f4 only ---")
            run_variant("V9-no-scene", lambda: build_v9_without({"SCENE_F4"}),
                       {"IN-A": INPUTS["IN-A"]}, runs=3)
            print("\n--- Drill: Remove suggestion only ---")
            run_variant("V9-no-suggestion", lambda: build_v9_without({"SUGGESTION"}),
                       {"IN-A": INPUTS["IN-A"]}, runs=3)
        else:
            print("Both or neither halves suppress - need more granular testing")
            # Test each individually
            for name, omit in [("V9-no-f1f2", {"F1F2"}), ("V9-no-scene", {"SCENE_F4"}), 
                              ("V9-no-suggestion", {"SUGGESTION"}), ("V9-no-wordbook", {"WORDBOOK"}),
                              ("V9-no-codeswitch", {"CODESWITCH"}), ("V9-no-unitsymbol", {"UNIT_SYMBOL"}),
                              ("V9-no-addpunct", {"ADD_PUNCT"})]:
                print(f"\n--- {name} ---")
                run_variant(name, lambda o=omit: build_v9_without(o), {"IN-A": INPUTS["IN-A"]}, runs=3)

# Final save
with open(out_path, "w", encoding="utf-8") as f:
    json.dump(RESULTS, f, ensure_ascii=False, indent=2)
print(f"\n\nFinal results saved to: {out_path}")
