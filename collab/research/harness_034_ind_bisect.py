#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
TEST-PROMPT-IND-034: IN-D 分叉点二分。
复用 033 的 G1 提示词（生产层序 + 生产真实 F4 + 新 ADD_PUNCT），18 次调用。

变体（各 3 次，共 18）：
  D0   IN-D 原句（不动）                   — 基线复现，确认 0/3 稳定
  D1   IN-D 删全部口水词「啊」             — 口水词是否触发器
  D2   IN-D 删结尾句                       — 收尾句是否压制处理
  D3   IN-D 删开头引入句                   — 引入句是否压制处理
  D4   IN-D 三项改名词短语                 — 动词性疑问短语假设
  CTRL IN-B 原句（verbatim）               — 正向对照，确认模型状态批内未变

模型固定 deepseek-v4-pro, temperature 0.3, max_tokens 512, thinking disabled。
主判据：edited / filler_removed；form 次之。
"""
import json, time, re, codecs, urllib.request

COMP = json.load(open(r"D:\Workspace\CodeLab\voice-ime\collab\research\ab033-components.json", encoding='utf-8'))
API_KEY = COMP["API_KEY"]
API_URL = COMP["API_URL"].rstrip("/") + "/chat/completions"
MODEL = COMP["MODEL"]
S = COMP

def build_g1():
    """生产前序：完全复用 033 已验证 G1（21863B / -0.9%）。本任务不得改动。"""
    L0 = [S["META_RULE_PRECEDENCE"], S["L0_1_FIDELITY"], S["L0_2_FIDELITY_OVER_FLUENCY"],
          S["L0_3_SUSPECT_INPUT"], S["L0_4_NOT_A_PROMPT"]]
    L1 = [S["CONTRACT_TRUE_PROD"]]
    L2 = [S["USER_PREFS_HEADER"], S["BASE_PROMPT"], S["EXTRA_PROD"], S["WORDBOOK_PROD"],
          S["F1F2_PROD"], S["CODESWITCH_FIX"], S["UNIT_SYMBOL_PROTECTION"],
          S["ADD_PUNCT_NEW"], S["SUGGESTION_INSTRUCTION"]]
    L3 = [S["SCENE_F4_PROD"], S["F3_TRUE_PROD"]]
    return "\n\n".join(L0 + L1 + L2 + L3)

SYS = build_g1()

def blen(s):
    return len(s.encode('utf-8'))

print(f"G1 提示词: {blen(SYS)}B / {len(SYS)} chars")

# ── 变体输入构造 ─────────────────────────────────────────
IN_D = "你可以去菜场看一看有哪些新鲜的水果和蔬菜，比如有没有新鲜的青椒，有没有新鲜的西瓜，还有没有比较新鲜的香蕉，这些啊你都可以看一看，有合适的就买。"
verb = json.load(open(r"D:\Workspace\CodeLab\voice-ime\collab\research\verbatim-inputs-025b.json", encoding='utf-8'))
IN_B = verb["IN-B"]["verbatim"]

INPUTS = {}
INPUTS["D0"] = IN_D
INPUTS["D1"] = IN_D.replace("这些啊", "这些")
INPUTS["D2"] = IN_D.replace("，这些啊你都可以看一看，有合适的就买。", "")
INPUTS["D3"] = IN_D.replace("你可以去菜场看一看有哪些新鲜的水果和蔬菜，", "")
INPUTS["D4"] = (IN_D.replace("有没有新鲜的青椒", "新鲜的青椒")
                   .replace("有没有新鲜的西瓜", "新鲜的西瓜")
                   .replace("还有没有比较新鲜的香蕉", "还有比较新鲜的香蕉"))
INPUTS["CTRL"] = IN_B

print("\n=== 变体实际输入（逐字，供主控核对改写范围）===")
for k in ["D0", "D1", "D2", "D3", "D4", "CTRL"]:
    print(f"\n[{k}] ({len(INPUTS[k])} chars):")
    print(INPUTS[k])

# ── 调用 ─────────────────────────────────────────────────
HEADERS = {"Content-Type": "application/json", "Authorization": f"Bearer {API_KEY}"}

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

FILLERS = ["啊", "呢", "嗯", "呃", "额", "哦", "呀", "这个", "那个", "就是说"]

def strip_corrected(text):
    t = text
    t = re.sub(r"</?corrected>", "", t)
    return t.strip()

def count_fillers(text):
    return {f: text.count(f) for f in FILLERS if f in text}

def filler_removed(inp, out):
    ic = count_fillers(inp)
    oc = count_fillers(out)
    for f, n in ic.items():
        if oc.get(f, 0) < n:
            return True
    return False

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

# 批次顺序：CTRL 分散在批次各位置，检测模型状态批内漂移
ORDER = ["D0", "D1", "D2", "CTRL", "D3", "D4",
         "D4", "D3", "CTRL", "D2", "D1", "D0",
         "D0", "D4", "D1", "D3", "D2", "CTRL"]

RESULTS = {}
seq = 0
for variant in ORDER:
    seq += 1
    inp = INPUTS[variant]
    runs = RESULTS.setdefault(variant, [])
    try:
        res = call_deepseek(SYS, inp)
    except Exception as e:
        print(f"  [{variant}] seq{seq} ERROR: {e}")
        res = {"content": "", "finish_reason": "error", "prompt_tokens": 0, "completion_tokens": 0, "error": str(e)}
    content = res.get("content", "").replace("\r", "")
    inner = strip_corrected(content)
    ed = (inner != inp.strip())
    fr = filler_removed(inp, inner)
    cls = classify(content)
    print(f"[{variant}] seq{seq:02d}/18: form={cls} edited={ed} filler_removed={fr} ct={res.get('completion_tokens')} finish={res.get('finish_reason')}")
    if content:
        print("    RAW: " + content.replace(chr(10), "\\n")[:200])
    runs.append({**res,
                 "seq": seq,
                 "form": cls,
                 "edited": ed,
                 "filler_removed": fr,
                 "input": inp,
                 "output_inner": inner})
    time.sleep(1.0)

# ── 落盘 ─────────────────────────────────────────────────
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\ind034-results.json"
payload = {
    "model": MODEL,
    "api_url": API_URL,
    "sys_prompt_len_bytes": blen(SYS),
    "sys_prompt_len_chars": len(SYS),
    "temperature": 0.3,
    "max_tokens": 512,
    "inputs": INPUTS,
    "order": ORDER,
    "results": RESULTS,
}
with codecs.open(out_path, 'w', 'utf-8') as f:
    json.dump(payload, f, ensure_ascii=False, indent=2)
print(f"\nSaved: {out_path}")

# ── 汇总 ─────────────────────────────────────────────────
print("\n" + "=" * 60 + "\nSUMMARY\n" + "=" * 60)
for v in ["D0", "D1", "D2", "D3", "D4", "CTRL"]:
    runs = RESULTS[v]
    b = sum(1 for r in runs if r["form"] == "bullet")
    o = sum(1 for r in runs if r["form"] == "ordered")
    p = sum(1 for r in runs if r["form"] == "para")
    e = sum(1 for r in runs if r["edited"])
    f = sum(1 for r in runs if r["filler_removed"])
    ct = [r["completion_tokens"] for r in runs]
    print(f"  {v}: form {b}b/{o}o/{p}p  edited {e}/3  filler_removed {f}/3  ct={ct}")
