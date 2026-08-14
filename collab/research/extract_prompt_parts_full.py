#!/usr/bin/env python3
"""
Extract ALL prompt parts from src/llm/mod.rs for PROMPT-ARCH-025 phase 2.
Includes the 8 missing segments that harness V0 lacks.
"""
import re
import json
import codecs

SRC = r"D:\Workspace\CodeLab\voice-ime\src\llm\mod.rs"
OUT = r"D:\Workspace\CodeLab\voice-ime\collab\research\prompt-parts-025-full.json"

def unescape_rust_str(s):
    import re as regex
    s = regex.sub(r'\\\\', '\x00', s)
    s = s.replace('\\n', '\n')
    s = s.replace('\\"', '"')
    s = s.replace('\x00', '\\')
    return s

with codecs.open(SRC, "r", "utf-8") as f:
    content = f.read()

def extract_const(name):
    pat = r'const ' + name + r': &str = "(.*?)";'
    m = re.search(pat, content, re.DOTALL)
    if not m:
        raise ValueError(f"Cannot find const {name}")
    return unescape_rust_str(m.group(1))

# Extract existing parts
META = extract_const("META_RULE_PRECEDENCE")
L0_1 = extract_const("L0_1_FIDELITY")
L0_2 = extract_const("L0_2_FIDELITY_OVER_FLUENCY")
L0_3 = extract_const("L0_3_SUSPECT_INPUT")
L0_4 = extract_const("L0_4_NOT_A_PROMPT")
USER_PREFS = extract_const("USER_PREFS_HEADER")

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

# Extract F1F2 (build_format_instruction_block)
# The function body contains the string literal directly
f1f2_match = re.search(r"fn build_format_instruction_block\(_multiline_safe: bool\) -> &'static str \{(.*?)\}", content, re.DOTALL)
if f1f2_match:
    f1f2_body = f1f2_match.group(1)
    # Find the string literal after the comment
    f1f2_str_match = re.search(r'"((?:[^"\\]|\\.)*)"', f1f2_body, re.DOTALL)
    if f1f2_str_match:
        F1F2 = unescape_rust_str(f1f2_str_match.group(1))
    else:
        F1F2 = ""
else:
    F1F2 = ""

# Extract constants
CODESWITCH = extract_const("CODESWITCH_FIX")
ADD_PUNCT = extract_const("ADD_PUNCT")
UNIT_SYMBOL = extract_const("UNIT_SYMBOL_PROTECTION")

# SUGGESTION_INSTRUCTION is multi-line with continuations - need special handling
sugg_match = re.search(r'const SUGGESTION_INSTRUCTION: &str = "(.*?)";', content, re.DOTALL)
SUGGESTION = unescape_rust_str(sugg_match.group(1)) if sugg_match else ""

# For scene_f4, construct a representative document scene block
# Based on task description: document scene with semantic并列判据
SCENE_F4 = """Scene-Specific Formatting Rules (Document):
When the active window is a document editor (word processor, note-taking app, Markdown editor, etc.), the output may use multiple lines when appropriate.
Narrative exemplification (e.g., several clauses each introducing a distinct example of a stated problem) is appropriate for bullet lists in document contexts.
Items standing in a PARALLEL relation should be formatted as lists when they are LONG clauses."""

# Wordbook placeholder (12 entries as task requires)
WORDBOOK = """User Vocabulary List: The following are user-defined vocabulary words (names, brands, technical terms).
When the transcription contains a word with similar pronunciation but incorrect spelling, silently correct it to the standard form from this list.
Words already matching the list should be kept as-is. Do NOT explain or reference these corrections.
<wordbook>
  <word>PPT</word>
  <word>GPT</word>
  <word>API</word>
  <word>UI</word>
  <word>URL</word>
  <word>APP</word>
  <word>PDF</word>
  <word>Excel</word>
  <word>Word</word>
  <word>PowerPoint</word>
  <word>Markdown</word>
  <word>VSCode</word>
</wordbook>"""

# Base prompt from config.toml (already in harness)
CONFIG_PATH = r"D:\Workspace\CodeLab\voice-ime\target\release\config.toml"
import tomllib
with open(CONFIG_PATH, "rb") as f:
    cfg = tomllib.load(f)
BASE_PROMPT = cfg["llm"]["system_prompt"].strip()

parts = {
    "META": META,
    "L0_1": L0_1, "L0_2": L0_2, "L0_3": L0_3, "L0_4": L0_4,
    "USER_PREFS": USER_PREFS,
    "CONTRACT_TRUE": CONTRACT_TRUE,
    "F3_TRUE": F3_TRUE,
    # The 8 missing segments
    "EXTRA_INSTRUCTION": "",
    "WORDBOOK": WORDBOOK,
    "F1F2": F1F2,
    "CODESWITCH": CODESWITCH,
    "UNIT_SYMBOL": UNIT_SYMBOL,
    "ADD_PUNCT": ADD_PUNCT,
    "SUGGESTION": SUGGESTION,
    "SCENE_F4": SCENE_F4,
    "BASE_PROMPT": BASE_PROMPT,
}

# Validate lengths
for k, v in parts.items():
    print(f"{k}={len(v)}")

total = sum(len(v) for v in parts.values() if v)
print(f"TOTAL={total}")

with codecs.open(OUT, "w", "utf-8") as f:
    json.dump(parts, f, ensure_ascii=False, indent=2)
print(f"Saved to {OUT}")
