#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Ablation Experiment — Full Prompt Text
Uses real prompt parts extracted from src/llm/mod.rs.
Reads API key from config.toml (runtime only, never logged).
"""
import json
import tomllib
import urllib.request
import time
import sys

CONFIG_PATH = r"D:\Workspace\CodeLab\voice-ime\target\release\config.toml"

with open(CONFIG_PATH, "rb") as f:
    cfg = tomllib.load(f)
llm = cfg["llm"]
API_KEY = llm["api_key"]
API_URL = llm["api_url"].rstrip("/") + "/chat/completions"
MODEL = llm["model"]

if not API_KEY or not API_URL or not MODEL:
    print("FATAL: missing api_key/api_url/model", file=sys.stderr)
    sys.exit(1)

# ── Load real prompt parts ─────────────────────────────────────
with open(r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-parts-025.json", "r", encoding="utf-8") as f:
    PARTS = json.load(f)

META = PARTS["META"]
L0_1 = PARTS["L0_1"]
L0_2 = PARTS["L0_2"]
L0_3 = PARTS["L0_3"]
L0_4 = PARTS["L0_4"]
USER_PREFS = PARTS["USER_PREFS"]
CONTRACT_FULL = PARTS["CONTRACT_TRUE"]
F3_FULL = PARTS["F3_TRUE"]

# The actual system_prompt from config.toml (the "base_prompt")
BASE_PROMPT = cfg["llm"]["system_prompt"].strip()

# Build a no-authority version of F3 header
F3_NO_AUTH = F3_FULL.replace(
    "the SINGLE authority on lists, line structure, and output tags",
    "the authority on list content and enumeration markers"
)

# Build a contract without line-structure language
CONTRACT_NO_LINE = (
    "Output format: Put the opening <corrected> and closing </corrected> "
    "around the whole text. After the closing tag, optionally ONE final "
    'line {"suggestions":["correct_word"]}.'
    "\nOutput NOTHING else. No explanations, no commentary."
)

# ── Input corpus ─────────────────────────────────────────────────
INPUTS = {
    "IN-A": "建议啊从以下方面入手啊，比如英语学习要多读多背，再比如多听视频节目，还有比如就是多出去和别人交流",
    "IN-B": "今天的讲话指出学校出现的现象。比如说有些同学下课徘徊，还有啊，比如有的同学烫发，呃，比如还有啊，有的同学不遵守课堂纪律。上面的现象都是不好的",
    "IN-C": "我现在就要说出公司的一些不好的现象，比如有的同事迟到早退，比如还有啊，上班摸鱼玩手机，还有的同事成果不及时提交。上面这些都需要注意",
    "C1": "现在有三点现象要注意，第一点怎样怎样，第二点怎样怎样，第三点怎样怎样",
    "C2": "今天雨下得很大，比如早上那阵就特别急",
    "C3": "今天出去买菜了，买了3斤土豆，一个西瓜，20斤大米，还有3斤香蕉",
}

# ── Variant builders ────────────────────────────────────────────
def build_v0():
    """Baseline: L0→L1(contract)→L2(base)→L3(F3)"""
    return "\n\n".join([META, L0_1, L0_2, L0_3, L0_4, CONTRACT_FULL, USER_PREFS, BASE_PROMPT, F3_FULL])

def build_v1():
    """V1: Delete line-structure from contract, F3 stays L3"""
    return "\n\n".join([META, L0_1, L0_2, L0_3, L0_4, CONTRACT_NO_LINE, USER_PREFS, BASE_PROMPT, F3_FULL])

def build_v2():
    """V2: F3 promoted to L1 (merged with contract), full line-structure kept"""
    merged = F3_FULL + "\n" + CONTRACT_FULL
    return "\n\n".join([META, L0_1, L0_2, L0_3, L0_4, merged, USER_PREFS, BASE_PROMPT])

def build_v3():
    """V3: V1+V2: no line-structure + F3 in L1 (merged)"""
    merged = F3_FULL + "\n" + CONTRACT_NO_LINE
    return "\n\n".join([META, L0_1, L0_2, L0_3, L0_4, merged, USER_PREFS, BASE_PROMPT])

def build_v4():
    """V4: Reproduce 012 structure: merge at prompt END, META retained"""
    merged = F3_FULL + "\n" + CONTRACT_FULL
    return "\n\n".join([META, L0_1, L0_2, L0_3, L0_4, USER_PREFS, BASE_PROMPT, merged])

def build_v6():
    """V6: Fix false authority claim only, rest same as V0"""
    return "\n\n".join([META, L0_1, L0_2, L0_3, L0_4, CONTRACT_FULL, USER_PREFS, BASE_PROMPT, F3_NO_AUTH])

VARIANTS = {
    "V0": build_v0,
    "V1": build_v1,
    "V2": build_v2,
    "V3": build_v3,
    "V4": build_v4,
    "V6": build_v6,
}

# ── API caller ───────────────────────────────────────────────────
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

# ── Runner ──────────────────────────────────────────────────────
RESULTS = {}

for var_name, builder in VARIANTS.items():
    sys_prompt = builder()
    print(f"\n{'='*60}")
    print(f"VARIANT {var_name} -- prompt_len={len(sys_prompt)}")
    print(f"{'='*60}")
    RESULTS[var_name] = {}
    for inp_key, inp_text in INPUTS.items():
        run_outputs = []
        for run in range(3):
            print(f"  [{var_name}][{inp_key}] run {run+1}/3...", end=" ", flush=True)
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
                print(f"OK fr={fr} pt={pt} ct={ct}")
                run_outputs.append(result)
                time.sleep(1.0)
        RESULTS[var_name][inp_key] = run_outputs

# Save
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-layer-ablation-025-full.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump(RESULTS, f, ensure_ascii=False, indent=2)
print(f"\n\nResults saved to: {out_path}")
