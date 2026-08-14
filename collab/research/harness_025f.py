#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Phase 6: Model probe (fixed baseline prompt, varying model)
12-18 API calls budget. Test deepseek-v4-pro (priority) + deepseek-chat (light对照).
Inputs: IN-B, IN-C (target) + C2, C4 (reverse controls).
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

# ── Inputs ──────────────────────────────────────────────────────
INPUTS = {
    "IN-B": "今天的讲话呢是要指出我们现在学校里面出现的一些现象。啊，比如说有些同学啊下课之后徘徊在校园里不走，三五成群。还有啊，比如有的同学烫发、染黄头发、染头发，这个不符合校规。呃，比如还有啊，有的同学不遵守课堂纪律，啊，顶撞老师。啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正。",
    "IN-C": "我现在就要说出我们公司的一些不好的现象，比如有的同事迟到早退，啊，不遵守公司的规章制度，啊，比如还有啊，有的上班时间摸鱼玩手机，还有的同事呢就是，嗯，工作的这个成果呀不及时提交，不及时审核，造成了一定的损失，上面这些啊都需要注意。",
    "C2": "今天雨下得很大，比如早上那阵就特别急",
    "C4": "今天的讲话呢是要指出我们现在学校里面出现的一些现象，就是有些同学不遵守课堂纪律，这个问题很严重，希望大家注意。",
}

# ── Models ─────────────────────────────────────────────────────
MODELS = {
    "deepseek-v4-pro": {"model": "deepseek-v4-pro", "desc": "Priority target"},
    "deepseek-chat": {"model": "deepseek-chat", "desc": "Light对照"},
}

# ── API caller ─────────────────────────────────────────────────
headers = {
    "Content-Type": "application/json",
    "Authorization": f"Bearer {API_KEY}",
}

call_count = 0

def call_deepseek(model_name, user_text):
    global call_count
    call_count += 1
    body = json.dumps({
        "model": model_name,
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

def classify(text, inp_key):
    t = text.strip()
    lines = [l for l in t.splitlines() if l.strip()]
    
    if inp_key == "C2":
        return "para" if len(lines) <= 1 else "list"
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

def run_model(model_key, model_cfg, inputs, runs=3):
    print(f"\n{'='*60}")
    print(f"MODEL {model_key} ({model_cfg['desc']})")
    print(f"{'='*60}")
    model_results = {}
    for inp_key, inp_text in inputs.items():
        run_outputs = []
        for run in range(runs):
            print(f"  [{model_key}][{inp_key}] run {run+1}/{runs}...", end=" ", flush=True)
            result = call_deepseek(model_cfg["model"], inp_text)
            content = result["content"].replace("\r", "")
            fr = result["finish_reason"]
            pt = result["prompt_tokens"]
            ct = result["completion_tokens"]
            cls = classify(content, inp_key)
            print(f"pt={pt} ct={ct} cls={cls}")
            raw_preview = content.replace("\n", "\\n")[:120]
            print(f"    RAW: {raw_preview}")
            run_outputs.append({**result, "class": cls})
            time.sleep(1.0)
        model_results[inp_key] = run_outputs
        bullet = sum(1 for r in run_outputs if r["class"] == "bullet")
        ordered = sum(1 for r in run_outputs if r["class"] == "ordered")
        para = sum(1 for r in run_outputs if r["class"] == "para")
        print(f"  → {bullet}b/{ordered}o/{para}p")
    RESULTS[model_key] = model_results
    return model_results

# Run v4-pro on all 4 inputs
run_model("deepseek-v4-pro", MODELS["deepseek-v4-pro"], INPUTS, runs=3)

# Run chat on IN-B and IN-C only (light对照)
run_model("deepseek-chat", MODELS["deepseek-chat"], 
          {"IN-B": INPUTS["IN-B"], "IN-C": INPUTS["IN-C"]}, runs=3)

print(f"\n\nTotal API calls: {call_count}")

# Save
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-model-probe-025f.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump({
        "call_count": call_count,
        "sys_prompt_len": len(SYS_PROMPT),
        "results": RESULTS,
    }, f, ensure_ascii=False, indent=2)
print(f"Saved to: {out_path}")

# Summary
print("\n" + "="*60)
print("SUMMARY")
print("="*60)
for model_key in ["deepseek-v4-pro", "deepseek-chat"]:
    print(f"\n{model_key}:")
    data = RESULTS[model_key]
    for inp_key in ["IN-B", "IN-C", "C2", "C4"]:
        if inp_key not in data:
            continue
        bullet = sum(1 for r in data[inp_key] if r["class"] == "bullet")
        ordered = sum(1 for r in data[inp_key] if r["class"] == "ordered")
        para = sum(1 for r in data[inp_key] if r["class"] == "para")
        
        if inp_key in ["IN-B", "IN-C"]:
            status = "LIST" if bullet >= 2 else ("ORDERED" if ordered >= 2 else "PARA")
            expected = "LIST"
        elif inp_key == "C2":
            status = "PARA" if para >= 2 else "FAIL"
            expected = "PARA"
        elif inp_key == "C4":
            status = "PARA" if para >= 2 else "FAIL"
            expected = "PARA"
        
        ok = "OK" if status == expected else "DEAD" if expected == "PARA" and status != "PARA" else "UNEXPECTED"
        print(f"  {inp_key}: {bullet}b/{ordered}o/{para}p = {status} [{ok}]")
