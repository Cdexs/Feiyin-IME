#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Phase 2 A1-bis: Re-run V9-full with VERBATIM inputs.
"""
import json
import tomllib
import urllib.request
import time
import sys

CONFIG_PATH = r"D:\Workspace\CodeLab\voice-ime\target\release\config.toml"
PARTS_PATH = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-parts-025-full.json"
SCENE_F4_PATH = r"D:\Workspace\CodeLab\voice-ime\collab\research\scene_f4_document.txt"
VERBATIM_PATH = r"D:\Workspace\CodeLab\voice-ime\collab\research\verbatim-inputs-025b.json"

with open(CONFIG_PATH, "rb") as f:
    cfg = tomllib.load(f)
llm = cfg["llm"]
API_KEY = llm["api_key"]
API_URL = llm["api_url"].rstrip("/") + "/chat/completions"
MODEL = llm["model"]

with open(PARTS_PATH, "r", encoding="utf-8") as f:
    PARTS = json.load(f)
with open(SCENE_F4_PATH, "r", encoding="utf-8") as f:
    PARTS["SCENE_F4"] = f.read().strip()
PARTS["BASE_PROMPT"] = cfg["llm"]["system_prompt"].strip()

with open(VERBATIM_PATH, "r", encoding="utf-8") as f:
    VERBATIM = json.load(f)

# Build V9-full exactly as before
def build_v9_full():
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
        PARTS["SCENE_F4"],
        PARTS["F3_TRUE"],
    ]
    return "\n\n".join(sections)

SYS_PROMPT = build_v9_full()
print(f"V9-full prompt_len={len(SYS_PROMPT)} tokens≈{len(SYS_PROMPT)//4}")

headers = {
    "Content-Type": "application/json",
    "Authorization": f"Bearer {API_KEY}",
}

def call_deepseek(user_text):
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
    ordered = sum(1 for l in lines if __import__('re').match(r'^\d+\.\s', l.strip()))
    if ordered >= 2:
        return "ordered"
    return "para"

# A1-bis: IN-A verbatim x3
ina_verbatim = VERBATIM["IN-A"]["verbatim"]
print(f"\n=== A1-bis: IN-A verbatim ({len(ina_verbatim)} chars) ===")
print(f"Text: {ina_verbatim}")

results = []
for i in range(3):
    print(f"  run {i+1}/3...", end=" ", flush=True)
    r = call_deepseek(ina_verbatim)
    cls = classify(r["content"])
    print(f"pt={r['prompt_tokens']} ct={r['completion_tokens']} fr={r['finish_reason']} cls={cls}")
    raw = r["content"].replace("\n", "\\n")
    print(f"    RAW: {raw}")
    results.append({**r, "class": cls})
    time.sleep(1.0)

# Save
out = {
    "variant": "V9-full",
    "input_key": "IN-A",
    "input_type": "verbatim",
    "input_text": ina_verbatim,
    "results": results,
}
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-suppressor-bisect-025b-a1bis.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=2)
print(f"\nSaved to: {out_path}")

bullet_runs = sum(1 for r in results if r["class"] == "bullet")
para_runs = sum(1 for r in results if r["class"] == "para")
print(f"\n=== A1-bis SUMMARY: IN-A verbatim V9-full = {bullet_runs}/3 bullet, {para_runs}/3 para ===")
if bullet_runs >= 2:
    print("IN-A STILL produces bullets even with verbatim text + full prompt.")
    print("→ Suppressor is NOT in prompt delta NOR in input cleaning.")
else:
    print("IN-A now outputs PARAGRAPH with verbatim text.")
    print("→ Input cleaning (removing ending sentence) was the suppressor.")
    print("→ Proceed to V8 (mixed structure hypothesis confirmed).")
