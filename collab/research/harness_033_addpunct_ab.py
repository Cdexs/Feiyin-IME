#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
TEST-PROMPT-AB-033: ADD_PUNCT 单变量 A/B 实验 + G0 历史复现对照。
组装三组提示词并按生产层序/历史层序调用 deepseek-v4-pro。

组设计（任务书 §5）：
  G0 历史复现: 无 F4, 旧 ADD_PUNCT, IN-B/IN-C 各 3 次   (共 6)
  G1 生产现状: 含 F4, 新 ADD_PUNCT, IN-D/IN-B/IN-C 各 3 (共 9)
  G2 生产回退: 含 F4, 旧 ADD_PUNCT, IN-D/IN-B/IN-C 各 3 (共 9)
总 24 次。模型固定 deepseek-v4-pro。
"""
import json, time, sys, re, codecs, urllib.request

COMP = json.load(open(r"D:\Workspace\CodeLab\voice-ime\collab\research\ab033-components.json", encoding='utf-8'))
API_KEY = COMP["API_KEY"]
API_URL = COMP["API_URL"].rstrip("/") + "/chat/completions"
MODEL = COMP["MODEL"]

# 层序常量（生产 render 逻辑, src/llm/mod.rs:297-450）
S = COMP

# ── 三组组装 ─────────────────────────────────────────────
def build_g0():
    """G0: 历史复现。完全复刻 harness_025f 层序(无 F4/无 extra/旧 ADD_PUNCT/快照 WORDBOOK)。"""
    parts = [S["SNAP_META"], S["SNAP_L0_1"], S["SNAP_L0_2"], S["SNAP_L0_3"], S["SNAP_L0_4"],
             S["SNAP_CONTRACT_TRUE"], S["SNAP_USER_PREFS"], S["BASE_PROMPT"],
             S["SNAP_F1F2"], S["SNAP_CODESWITCH"], S["SNAP_UNIT_SYMBOL"], S["SNAP_ADD_PUNCT"],
             S["SNAP_SUGGESTION"], S["SNAP_WORDBOOK"], S["SNAP_F3_TRUE"]]
    return "\n\n".join(parts)

def build_g1():  # 生产前序, 新 ADD_PUNCT
    L0 = [S["META_RULE_PRECEDENCE"], S["L0_1_FIDELITY"], S["L0_2_FIDELITY_OVER_FLUENCY"],
          S["L0_3_SUSPECT_INPUT"], S["L0_4_NOT_A_PROMPT"]]
    L1 = [S["CONTRACT_TRUE_PROD"]]
    L2 = [S["USER_PREFS_HEADER"], S["BASE_PROMPT"], S["EXTRA_PROD"], S["WORDBOOK_PROD"],
          S["F1F2_PROD"], S["CODESWITCH_FIX"], S["UNIT_SYMBOL_PROTECTION"],
          S["ADD_PUNCT_NEW"], S["SUGGESTION_INSTRUCTION"]]
    L3 = [S["SCENE_F4_PROD"], S["F3_TRUE_PROD"]]
    return "\n\n".join(L0 + L1 + L2 + L3)

def build_g2():  # 生产前序, 旧 ADD_PUNCT
    L0 = [S["META_RULE_PRECEDENCE"], S["L0_1_FIDELITY"], S["L0_2_FIDELITY_OVER_FLUENCY"],
          S["L0_3_SUSPECT_INPUT"], S["L0_4_NOT_A_PROMPT"]]
    L1 = [S["CONTRACT_TRUE_PROD"]]
    L2 = [S["USER_PREFS_HEADER"], S["BASE_PROMPT"], S["EXTRA_PROD"], S["WORDBOOK_PROD"],
          S["F1F2_PROD"], S["CODESWITCH_FIX"], S["UNIT_SYMBOL_PROTECTION"],
          S["ADD_PUNCT_OLD"], S["SUGGESTION_INSTRUCTION"]]
    L3 = [S["SCENE_F4_PROD"], S["F3_TRUE_PROD"]]
    return "\n\n".join(L0 + L1 + L2 + L3)

SYS = {"G0": build_g0(), "G1": build_g1(), "G2": build_g2()}

def blen(s): return len(s.encode('utf-8'))
print("=== 三组提示词长度（UTF-8 字节, 与 Rust len() 一致）===")
TARGET = 22055  # 生产实测
for k in ["G0", "G1", "G2"]:
    b = blen(SYS[k])
    print(f"  {k}: {b}B (chars={len(SYS[k])})  vs 目标22055: {(b-TARGET)/TARGET*100:+.1f}%")

sys.stdout.write("  保真度门禁: G1/G2 必须 ±5% 内\n")

# ── 输入 ─────────────────────────────────────────────────
IN_D = "你可以去菜场看一看有哪些新鲜的水果和蔬菜，比如有没有新鲜的青椒，有没有新鲜的西瓜，还有没有比较新鲜的香蕉，这些啊你都可以看一看，有合适的就买。"
verb = json.load(open(r"D:\Workspace\CodeLab\voice-ime\collab\research\verbatim-inputs-025b.json", encoding='utf-8'))
IN_B = verb["IN-B"]["verbatim"]
IN_C = verb["IN-C"]["verbatim"]

INPUTS = {"G0": {"IN-B": IN_B, "IN-C": IN_C},
          "G1": {"IN-D": IN_D, "IN-B": IN_B, "IN-C": IN_C},
          "G2": {"IN-D": IN_D, "IN-B": IN_B, "IN-C": IN_C}}

HEADERS = {"Content-Type": "application/json", "Authorization": f"Bearer {API_KEY}"}

def call_deepseek(system_prompt, user_text, group, inp_key, run):
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
    req = urllib.request.Request(API_URL, data=body, headers=HEADERS, method="POST")
    with urllib.request.urlopen(req, timeout=120) as resp:
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
    bullet = sum(1 for l in lines if l.strip().startswith("- ") or l.strip().startswith("• "))
    ordered = sum(1 for l in lines if re.match(r'^\d+\.\s', l.strip()))
    if bullet >= 2:
        return "bullet"
    if ordered >= 2:
        return "ordered"
    return "para"

RESULTS = {}

def run_group(gkey):
    print(f"\n{'='*60}\nRUN {gkey}\n{'='*60}")
    grp = {}
    for inp_key, text in INPUTS[gkey].items():
        runs = []
        for r in range(3):
            try:
                res = call_deepseek(SYS[gkey], text, gkey, inp_key, r)
            except Exception as e:
                print(f"  [{gkey}][{inp_key}] run {r+1} ERROR: {e}")
                res = {"content": "", "finish_reason": "error", "prompt_tokens": 0, "completion_tokens": 0, "error": str(e)}
            content = res.get("content", "").replace("\r", "")
            cls = classify(content)
            print(f"  [{gkey}][{inp_key}] run {r+1}/3: pt={res.get('prompt_tokens')} ct={res.get('completion_tokens')} cls={cls}")
            if content:
                print("    RAW: " + content.replace(chr(10), "\\n")[:150])
            runs.append({**res, "class": cls})
            time.sleep(1.0)
        b = sum(1 for r_ in runs if r_["class"] == "bullet")
        o = sum(1 for r_ in runs if r_["class"] == "ordered")
        p = sum(1 for r_ in runs if r_["class"] == "para")
        print(f"  → {b}b/{o}o/{p}p")
        grp[inp_key] = runs
    RESULTS[gkey] = grp

for g in ["G0", "G1", "G2"]:
    run_group(g)

# ── 落盘 ─────────────────────────────────────────────────
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\ab033-results.json"
payload = {
    "model": MODEL,
    "api_url": API_URL,
    "sys_prompts": {k: {"len_bytes": blen(v), "len_chars": len(v)} for k, v in SYS.items()},
    "inputs": {"IN-B": IN_B, "IN-C": IN_C, "IN-D": IN_D},
    "results": RESULTS,
}
with codecs.open(out_path, 'w', 'utf-8') as f:
    json.dump(payload, f, ensure_ascii=False, indent=2)
print(f"\nSaved: {out_path}")

# ── 汇总 ─────────────────────────────────────────────────
print("\n" + "="*60 + "\nSUMMARY\n" + "="*60)
for g in ["G0", "G1", "G2"]:
    print(f"\n{g}:")
    for inp_key in INPUTS[g]:
        runs = RESULTS[g][inp_key]
        b = sum(1 for r in runs if r["class"] == "bullet")
        o = sum(1 for r in runs if r["class"] == "ordered")
        p = sum(1 for r in runs if r["class"] == "para")
        expected = "LIST(bullet)" if (g != "G0" or inp_key in ("IN-B", "IN-C")) else "?"
        print(f"  {inp_key}: {b}b/{o}o/{p}p")