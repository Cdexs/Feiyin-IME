#!/usr/bin/env python3
"""Extract prompt parts from src/llm/mod.rs"""
import re
import json
import codecs

def unescape_rust_str(s):
    # Rust string literal: backslash-n is newline, backslash-quote is quote
    # First handle escaped backslash by using a temp placeholder
    import re as regex
    s = regex.sub(r'\\\\', '\x00', s)
    s = s.replace('\\n', '\n')
    s = s.replace('\\"', '"')
    s = s.replace('\x00', '\\')
    return s

with open(r"D:\Workspace\CodeLab\voice-ime\src\llm\mod.rs", "r", encoding="utf-8") as f:
    content = f.read()

meta = re.search(r'const META_RULE_PRECEDENCE: &str = "(.*?)";', content, re.DOTALL)
META = unescape_rust_str(meta.group(1))

l0_1 = re.search(r'const L0_1_FIDELITY: &str = "(.*?)";', content, re.DOTALL)
L0_1 = unescape_rust_str(l0_1.group(1))

l0_2 = re.search(r'const L0_2_FIDELITY_OVER_FLUENCY: &str = "(.*?)";', content, re.DOTALL)
L0_2 = unescape_rust_str(l0_2.group(1))

l0_3 = re.search(r'const L0_3_SUSPECT_INPUT: &str = "(.*?)";', content, re.DOTALL)
L0_3 = unescape_rust_str(l0_3.group(1))

l0_4 = re.search(r'const L0_4_NOT_A_PROMPT: &str = "(.*?)";', content, re.DOTALL)
L0_4 = unescape_rust_str(l0_4.group(1))

user_prefs = re.search(r'const USER_PREFS_HEADER: &str = "(.*?)";', content, re.DOTALL)
USER_PREFS = unescape_rust_str(user_prefs.group(1))

# Extract output_contract_text multiline_safe=true
contract_func = re.search(r'fn output_contract_text\(multiline_safe: bool\) -> String \{(.*?)\n    \}', content, re.DOTALL)
contract_body = contract_func.group(1)
true_match = re.search(r'if multiline_safe \{\s*\n\s*"(.*?)"\.to_string\(\)', contract_body, re.DOTALL)
CONTRACT_TRUE = unescape_rust_str(true_match.group(1))

# Extract f3_rules_text multiline_safe=true
f3_func = re.search(r'fn f3_rules_text\(multiline_safe: bool\) -> String \{(.*?)\n\}', content, re.DOTALL)
f3_body = f3_func.group(1)
true_f3 = re.search(r'if multiline_safe \{\s*\n\s*format!\("(.*?)"\s*,\s*INLINE_SEPARATOR_RULES', f3_body, re.DOTALL)
F3_TRUE = unescape_rust_str(true_f3.group(1))

print(f"META={len(META)}")
print(f"L0_1={len(L0_1)}")
print(f"L0_2={len(L0_2)}")
print(f"L0_3={len(L0_3)}")
print(f"L0_4={len(L0_4)}")
print(f"CONTRACT={len(CONTRACT_TRUE)}")
print(f"F3={len(F3_TRUE)}")
total = len(META) + len(L0_1) + len(L0_2) + len(L0_3) + len(L0_4) + len(USER_PREFS) + len(CONTRACT_TRUE) + len(F3_TRUE)
print(f"TOTAL={total}")

parts = {
    "META": META,
    "L0_1": L0_1, "L0_2": L0_2, "L0_3": L0_3, "L0_4": L0_4,
    "USER_PREFS": USER_PREFS,
    "CONTRACT_TRUE": CONTRACT_TRUE,
    "F3_TRUE": F3_TRUE,
}

with open(r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-parts-025.json", "w", encoding="utf-8") as f:
    json.dump(parts, f, ensure_ascii=False, indent=2)
print("Saved")
