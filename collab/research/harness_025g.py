#!/usr/bin/env python3
"""
PROMPT-ARCH-025 Phase 7: Latency measurement (fixed baseline prompt)
12 API calls: 2 models x 2 inputs x 3 runs.
Records wall-clock ms from request start to response received.
"""
import json
import tomllib
import urllib.request
import time
import sys
import statistics

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

INPUTS = {
    "IN-B": "今天的讲话呢是要指出我们现在学校里面出现的一些现象。啊，比如说有些同学啊下课之后徘徊在校园里不走，三五成群。还有啊，比如有的同学烫发、染黄头发、染头发，这个不符合校规。呃，比如还有啊，有的同学不遵守课堂纪律，啊，顶撞老师。啊，上面的现象啊都是一些不好的，希望大家听到了之后要及时纠正。",
    "IN-C": "我现在就要说出我们公司的一些不好的现象，比如有的同事迟到早退，啊，不遵守公司的规章制度，啊，比如还有啊，有的上班时间摸鱼玩手机，还有的同事呢就是，嗯，工作的这个成果呀不及时提交，不及时审核，造成了一定的损失，上面这些啊都需要注意。",
}

MODELS = {
    "deepseek-v4-pro": "deepseek-v4-pro",
    "deepseek-v4-flash": "deepseek-v4-flash",
}

headers = {
    "Content-Type": "application/json",
    "Authorization": f"Bearer {API_KEY}",
}

call_count = 0
RESULTS = {}

def call_and_measure(model_name, user_text):
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
    
    t0 = time.time()
    with urllib.request.urlopen(req, timeout=60) as resp:
        data = json.loads(resp.read().decode("utf-8"))
    t1 = time.time()
    
    choice = data.get("choices", [{}])[0]
    msg = choice.get("message", {})
    usage = data.get("usage", {})
    
    return {
        "content": msg.get("content", ""),
        "finish_reason": choice.get("finish_reason", ""),
        "prompt_tokens": usage.get("prompt_tokens", 0),
        "completion_tokens": usage.get("completion_tokens", 0),
        "latency_ms": round((t1 - t0) * 1000, 1),
    }

for model_key, model_name in MODELS.items():
    print(f"\n{'='*60}")
    print(f"MODEL {model_key}")
    print(f"{'='*60}")
    model_results = {}
    for inp_key, inp_text in INPUTS.items():
        runs = []
        for i in range(3):
            print(f"  [{model_key}][{inp_key}] run {i+1}/3...", end=" ", flush=True)
            r = call_and_measure(model_name, inp_text)
            print(f"lat={r['latency_ms']}ms pt={r['prompt_tokens']} ct={r['completion_tokens']} fr={r['finish_reason']}")
            runs.append(r)
            time.sleep(1.0)
        model_results[inp_key] = runs
    RESULTS[model_key] = model_results

print(f"\n\nTotal API calls: {call_count}")

# Summary
print("\n" + "="*60)
print("LATENCY SUMMARY")
print("="*60)
for model_key in MODELS.keys():
    print(f"\n{model_key}:")
    for inp_key in INPUTS.keys():
        data = RESULTS[model_key][inp_key]
        latencies = [r["latency_ms"] for r in data]
        prompt_tokens = [r["prompt_tokens"] for r in data]
        completion_tokens = [r["completion_tokens"] for r in data]
        
        lat_min = min(latencies)
        lat_max = max(latencies)
        lat_median = statistics.median(latencies)
        pt_avg = round(statistics.mean(prompt_tokens))
        ct_avg = round(statistics.mean(completion_tokens))
        
        print(f"  {inp_key}:")
        print(f"    latency: min={lat_min}ms / median={lat_median}ms / max={lat_max}ms")
        print(f"    tokens:  prompt={pt_avg} / completion={ct_avg}")

# Save
out_path = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-f3-model-latency-025g.json"
with open(out_path, "w", encoding="utf-8") as f:
    json.dump({
        "call_count": call_count,
        "sys_prompt_len": len(SYS_PROMPT),
        "results": RESULTS,
    }, f, ensure_ascii=False, indent=2)
print(f"\nSaved to: {out_path}")
