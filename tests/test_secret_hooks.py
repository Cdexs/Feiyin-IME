# HOOKTEST-136 · 密钥/隐私闸门自动化回归测试（coder-2，2026-09-06）
#
# 测的是 scripts/git-hooks/ 的 pre-commit / pre-push / secret-patterns.sh /
# secret-paths.sh（内容闸门 + 路径闸门 + 逃生口 + fail-closed）。
# 🔴 本文件不改 scripts/git-hooks/ 任何文件；闸门真有洞 → 报主控开新单。
#
# 硬性约束（任务书 §三）：
#   1. 全部实验在一次性临时仓库（pytest tmp_path）里做，绝不碰本仓库的索引；
#   2. 每次 git add（含 -f）之后用 `git ls-files --error-unmatch` 复核文件真进了
#      索引 —— [MSYS-GIT-ADD-FORCE-NOOP-001]：add -f 对被忽略文件偶发静默失败
#      （exit 0 但没进索引），不复核会把「没 add 上」误判成「闸门放行」，结论反向；
#   3. 用例全部用假值（重复字母拼的 key、全零手机号、明显假的身份证段），零真实凭证。
#
# 用例注释里每条写清：守的是什么 / 消融方式（改闸门哪一行会让它红）。

import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path

import pytest

pytestmark = pytest.mark.hooks

REPO_ROOT = Path(__file__).resolve().parents[1]
HOOKS_DIR = REPO_ROOT / "scripts" / "git-hooks"

# ---- 假值常量（零真实凭证；🔴 全部运行时拼接 —— 给密钥闸门写测试，用例里
#      必然要出现密钥形态字符串，字面量落盘会被闸门自己拦下（HOOKTEST-136-B）。
#      文件里只保留片段，运行时拼出的值与原始形态逐字节相同，测试效力不减。
#      与 tester-1 结构护栏用 concat! 拆串防自命中是同一个手法。） ----------------

FAKE_SK = "sk" + "-" + "a" * 32
FAKE_SK_ANT = "sk" + "-ant-" + "b" * 32
FAKE_GHP = "ghp" + "_" + "c" * 32
FAKE_GITHUB_PAT = "github" + "_pat_" + "d" * 30
FAKE_AWS = "AKIA" + "A" * 16
FAKE_GENERIC_KEY = "0123456789abcdefghij" + "0123456789AB"
FAKE_PHONE = "138" + "0" * 8  # 运行时 11 位；全零段，显然假
FAKE_IDCARD = "110000" + "1990" + "0101" + "001X"  # 运行时 18 位；110000 区段未启用，显然假
FAKE_EMAIL = "alice" + chr(64) + "test-domain.org"  # 非白名单域名
# git 提交身份用 noreply 段（闸门白名单内），且 at-sign 运行时拼接双保险
TEST_EMAIL = "hooktest" + chr(64) + "users.noreply.github.com"


# ---- 一次性临时仓库基建 ----------------------------------------------------


def _run(cmd, cwd, env=None):
    e = os.environ.copy()
    if env:
        e.update(env)
    return subprocess.run(
        cmd,
        cwd=str(cwd),
        env=e,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def _force_rmtree(path):
    """Windows 下 git 对象文件带只读位，普通 rmtree 会失败，onerror 里补权限。"""

    def _onerror(func, target, _exc):
        os.chmod(target, stat.S_IWRITE)
        func(target)

    shutil.rmtree(str(path), onerror=_onerror)


@pytest.fixture
def workdir(tmp_path):
    """一次性临时目录：用完立即删（任务书红线：临时仓库/远端必须清理干净）。"""
    yield tmp_path
    _force_rmtree(tmp_path)


def _init_repo(base: Path, name: str = "repo") -> Path:
    repo = base / name
    repo.mkdir(parents=True, exist_ok=True)
    r = _run(["git", "init", "-b", "main"], cwd=repo)
    assert r.returncode == 0, r.stderr
    # 本地身份 + 关 gpg，避免宿主全局配置漂移影响判定（邮箱用白名单段 + chr(64) 拼接）
    _run(["git", "config", "user.name", "hooktest"], cwd=repo)
    _run(["git", "config", "user.email", TEST_EMAIL], cwd=repo)
    _run(["git", "config", "commit.gpgsign", "false"], cwd=repo)
    # 🔴 钩子指向本仓库真实的 scripts/git-hooks（绝对路径），不改钩子本体
    r = _run(
        ["git", "config", "core.hooksPath", str(HOOKS_DIR)],
        cwd=repo,
    )
    assert r.returncode == 0, r.stderr
    return repo


def _write(repo: Path, relpath: str, content: str) -> Path:
    p = repo / relpath
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content, encoding="utf-8")
    return p


def _add(repo: Path, relpath: str, force: bool = False) -> None:
    cmd = ["git", "add"]
    if force:
        cmd.append("-f")
    cmd.append(relpath)
    r = _run(cmd, cwd=repo)
    assert r.returncode == 0, f"git add 失败: {r.stderr}"
    # 🔴 [MSYS-GIT-ADD-FORCE-NOOP-001]：add -f 偶发 exit 0 但文件没进索引。
    # 不复核的话，「文件压根没进 staging」会被误判成「闸门放行」，结论方向正好反过来。
    chk = _run(["git", "ls-files", "--error-unmatch", relpath], cwd=repo)
    assert chk.returncode == 0, (
        f"[MSYS-GIT-ADD-FORCE-NOOP-001] 防线触发：git add 后 {relpath} 不在索引里，"
        f"本用例的判定无效（这不是闸门放行）。ls-files: {chk.stderr}"
    )


def _commit(repo: Path, env_extra=None):
    env = {
        "GIT_AUTHOR_NAME": "hooktest",
        "GIT_AUTHOR_EMAIL": TEST_EMAIL,
    } if env_extra is None else env_extra
    return _run(["git", "commit", "-m", "hooktest"], cwd=repo, env=env)


def _init_bare_remote(base: Path, name: str = "remote.git") -> Path:
    remote = base / name
    r = _run(["git", "init", "--bare", "-b", "main", str(remote)], cwd=base)
    assert r.returncode == 0, r.stderr
    return remote


def _push(repo: Path, remote: Path, env_extra=None):
    _run(["git", "remote", "add", "origin", str(remote)], cwd=repo)
    return _run(["git", "push", "origin", "main:refs/heads/main"], cwd=repo, env=env_extra)


def _commit_blocked(r) -> bool:
    """pre-commit 拦截 = exit != 0 且 stderr 带拦截标记（与普通 git 失败区分）。"""
    return r.returncode != 0 and ("❌" in r.stderr or "拦截" in r.stderr)


# ---- 第 1 类 · 内容闸门：该拦的必须拦 ---------------------------------------
# 守的是 secret-patterns.sh 的五组正则（SECRET_PREFIX / GENERIC / EMAIL / PHONE / IDCARD）。
# 消融：把对应正则改松（如 {20,}→{40,}、删掉某前缀）→ 对应用例红。

CONTENT_BLOCK_CASES = [
    ("sk_prefix", f'key = "{FAKE_SK}"'),
    ("sk_ant_prefix", f'key = "{FAKE_SK_ANT}"'),
    ("ghp_prefix", f'token = "{FAKE_GHP}"'),
    ("github_pat_prefix", f'token = "{FAKE_GITHUB_PAT}"'),
    ("aws_akia", f"aws_access_key_id = {FAKE_AWS}"),
    ("generic_api_key_assign", f'api_key = "{FAKE_GENERIC_KEY}"'),
    ("generic_secret_key_assign", f'secret_key = "{FAKE_GENERIC_KEY}"'),
    ("phone", f"联系电话 {FAKE_PHONE} 请回电"),
    ("idcard", f"证件号 {FAKE_IDCARD} 已核验"),
    ("email_non_whitelist", f"联系邮箱：{FAKE_EMAIL}"),
]


@pytest.mark.parametrize("case_name,content", CONTENT_BLOCK_CASES, ids=[c[0] for c in CONTENT_BLOCK_CASES])
def test_content_gate_blocks(workdir, case_name, content):
    """该拦的必须拦：pre-commit 对 staged 新增行命中即拒（exit != 0）。"""
    repo = _init_repo(workdir)
    _write(repo, "src/sample.txt", content + "\n")
    _add(repo, "src/sample.txt")
    r = _commit(repo)
    assert _commit_blocked(r), (
        f"[{case_name}] 闸门放行了疑似敏感内容！stderr={r.stderr}"
    )


# ---- 第 2 类 · 内容闸门：该放的必须放 ---------------------------------------
# 🔴 这类比第 1 类更重要：闸门乱拦会逼人绕过它，绕着绕着闸门就废了。
# 消融：删掉 SECRET_PLACEHOLDER 对应词、或把邮箱白名单收紧 → 对应用例红。

CONTENT_PASS_CASES = [
    ("pure_code", 'fn main() { println!("hello world"); }\n'),
    ("whitelist_email_gavin_hotmail", "contact: cdexs@hotmail.com\n"),
    ("whitelist_email_gavin_outlook", "author: gavinshare6@outlook.com\n"),
    ("whitelist_email_noreply", "contact: noreply@service.io\n"),
    ("whitelist_email_noreply_github", "reply-to: bot@users.noreply.github.com\n"),
    ("placeholder_your_key", 'api_key = "YOUR_API_KEY_HERE_REPLACE_ME_0123456789"\n'),
    ("placeholder_redacted", 'client_secret = "REDACTEDREDACTEDREDACTED"\n'),
    ("placeholder_angle", 'api_key = "<请在此填入你的真实密钥0123456789>"\n'),
    ("placeholder_change_me", 'password = "CHANGE_ME_TO_A_LONG_RANDOM_VALUE_0123456789"\n'),
    ("env_read_rust", 'let key = std::env::var("API_KEY").unwrap();\n'),
    ("env_read_node", 'const key = process.env.API_KEY;\n'),
    ("env_read_python", 'key = os.getenv("SECRET_KEY_VALUE", "")\n'),
]


@pytest.mark.parametrize("case_name,content", CONTENT_PASS_CASES, ids=[c[0] for c in CONTENT_PASS_CASES])
def test_content_gate_allows(workdir, case_name, content):
    """该放的必须放：占位符 / 白名单邮箱 / 环境变量读法不拦（exit == 0）。"""
    repo = _init_repo(workdir)
    _write(repo, "src/sample.txt", content)
    _add(repo, "src/sample.txt")
    r = _commit(repo)
    assert r.returncode == 0, (
        f"[{case_name}] 闸门误拦合法内容（乱拦会逼人绕过闸门）！stderr={r.stderr}"
    )


# ---- 第 3 类 · 路径闸门（SECRET-126） ---------------------------------------
# 守的是 secret-paths.sh scan_paths 的类别判定。全部走 git add -f：
# .gitignore 拦不住强塞（这正是路径闸门存在的理由），必须测强塞路径。
# 消融：删掉/改松对应 case 分支 → 对应用例红。

# (相对路径, 文件内容) —— 载荷用 f-string + 常量：字面量不出完整密钥形态
PATH_BLOCK_CASES = [
    ("root_config_toml", "config.toml", f'api_key = "{FAKE_GENERIC_KEY}"\n'),
    ("root_config_toml_uppercase", "CONFIG.TOML", "# Windows 同一文件，大小写不归一化=绕过\n"),
    # 🔴 2026-09-06 补：真正装着 api_key 的是这两份运行时副本，此前只靠 .gitignore
    # 挡着，git add -f 即可绕过（Gavin 追问「config 不能提交」时主控发现的缺口）。
    ("runtime_config_target", "target/release/config.toml", f'api_key = "{FAKE_GENERIC_KEY}"\n'),
    ("runtime_config_publish", "Publish/config.toml", f'api_key = "{FAKE_GENERIC_KEY}"\n'),
    ("dotenv", ".env", f"API_KEY={FAKE_GENERIC_KEY}\n"),
    ("dotenv_local", ".env.local", f"API_KEY={FAKE_GENERIC_KEY}\n"),
    ("debug_log", "debug.log", "2026-09-06 用户口述转写原文……\n"),
    ("sqlite", "wordbook.sqlite", "SQLite format 3\x00\n"),
    ("db", "wordbook.db", "SQLite format 3\x00\n"),
    ("version_check_json", "version_check.json", '{"latest": "0.9.0"}\n'),
    ("collab_evidence", "collab/evidence/dump.json", f'{{"api_key": "{FAKE_GENERIC_KEY}"}}\n'),
    ("collab_research", "collab/research/notes.md", "实验记录\n"),
]


@pytest.mark.parametrize("case_name,relpath,content", PATH_BLOCK_CASES, ids=[c[0] for c in PATH_BLOCK_CASES])
def test_path_gate_blocks(workdir, case_name, relpath, content):
    """按路径即判危险的文件，即使 git add -f 强塞也必须拦（pre-commit，SECRET-126）。"""
    repo = _init_repo(workdir)
    _write(repo, relpath, content)
    _add(repo, relpath, force=True)
    r = _commit(repo)
    assert _commit_blocked(r), f"[{case_name}] 路径闸门放行了危险类别文件！stderr={r.stderr}"
    assert "路径" in r.stderr or "SECRET-126" in r.stderr, (
        f"[{case_name}] 拦截了但不是路径闸门拦的（应是内容闸门误打误撞）：{r.stderr}"
    )


PATH_PASS_CASES = [
    ("cargo_config", ".cargo/config.toml", "[build]\njobs = 4\n"),
    ("tauri_cargo_config", "src-tauri/.cargo/config.toml", "[build]\njobs = 4\n"),
    ("default_config_template", "assets/default-config.toml", 'api_key = ""  # 脱敏模板，留空\n'),
    ("logs_md_not_log", "logs/20260906.md", "# 改动日志\n今天完成了某任务。\n"),
    ("scene_rules_toml", "scene-rules.toml", '[[rule]]\nname = "sample"\n'),
    ("itn_rules_toml", "itn-rules.toml", '[[rule]]\nname = "sample"\n'),
    ("cargo_toml", "Cargo.toml", "[package]\nname = \"x\"\n"),
    ("src_main_rs", "src/main.rs", "fn main() {}\n"),
    ("collab_todo_md", "collab/todo.md", "# todo\n- [x] something\n"),
]


@pytest.mark.parametrize("case_name,relpath,content", PATH_PASS_CASES, ids=[c[0] for c in PATH_PASS_CASES])
def test_path_gate_allows(workdir, case_name, relpath, content):
    """构建配置 / 脱敏模板 / 改动日志(.md) / 规则文件必须放行 —— 拦它们会让开发停摆。"""
    repo = _init_repo(workdir)
    _write(repo, relpath, content)
    _add(repo, relpath, force=True)
    r = _commit(repo)
    assert r.returncode == 0, (
        f"[{case_name}] 路径闸门误拦合法文件！stderr={r.stderr}"
    )


# ---- 第 4 类 · 真实事故回归（本单最值钱的一类） -------------------------------
# 前三类是「设计上应该这样」，这一类是「这些事真的发生过，绝不许再发生」。

REGRESSION_CASES = [
    # (id, 内容, 期望blocked?, 出处/说明)
    (
        "secret105_decorator_not_email",
        "@pytest.hookimpl\n",  # staged diff 行首 +/- 标记紧贴 at-sign 与 pytest.hookimpl（SECRET-105 原始形态）
        False,
        "SECRET-105：diff 行首的 +/- 剥离前，装饰器行的标记字符被当成邮箱 local-part，"
        "任何「at-sign + 域名」形态的新增行都误报。scan_diff 入口 sed 's/^[+-]//' 修的正是这个。"
        "消融：删掉那行 sed → 本用例红（装饰器行被当邮箱误拦）。",
    ),
    (
        "secret104_cargolock_checksum_not_phone",
        'checksum = "9a19749016521db8f3c2e1a0b4d5e6f7"\n',
        False,
        "SECRET-104：Cargo.lock 校验和里的某 11 位连号被字母包夹（本载荷本身就是现行边界下"
        "不命中的活证据），旧 PHONE 边界「非数字」会误拦；现行边界「非字母数字」放过。"
        "消融：把 PRIVACY_PHONE_PATTERN 的 [^0-9A-Za-z] 改回 [^0-9] → 本用例红。",
    ),
    (
        "secret105_plusplus_content_line_scanned",
        "++" + FAKE_SK + "\n",  # 内容行以 ++ 开头，diff 里呈现为 "+++sk-..."（无空格）
        True,
        "SECRET-105 漏报洞：旧代码用裸 ^+++ 排文件头，把 +++sk-...（内容行以 ++ 开头）"
        "整行当文件头丢弃，真密钥溜过。现行白名单只排 '+++ b/、/dev/null、引号路径'。"
        "消融：把 -vE 的白名单改回裸 ^\\+\\+\\+ → 本用例红（密钥漏过，测试反而变绿=危险方向）。",
    ),
    (
        "secret_in_repo_001_config_dump_json",
        '{\n  "api_key": "' + FAKE_GENERIC_KEY + '",\n  "app_exe": "voice-ime.exe"\n}\n',
        True,
        "[SECRET-IN-REPO-001]：2026-08-14 真实事故的原始形态 —— 运行时配置整份 dump 成 "
        "JSON（含 api_key）进仓库，暴露 20 天被盗刷，真实金钱损失。内容闸门必须拦下这个形态。",
    ),
    (
        "tauri_icon_2x_known_false_positive",
        '{ "icon": ["icons/128x128' + chr(64) + '2x.png"] }\n',
        True,
        "🔴 已知误报，如实断言「会被拦」：Tauri 图标名（128x128 两个 x 画质后缀、文件名内含"
        " at-sign）被邮箱正则命中（2026-09-06 主控自己的提交被拦）。现状如此，不为让用例好看"
        "改闸门 —— 要修请主控开新单（例：给邮箱正则的 domain 段加字符类约束或加文件名白名单）。",
    ),
]


@pytest.mark.parametrize("case_name,content,expect_blocked,origin", REGRESSION_CASES, ids=[c[0] for c in REGRESSION_CASES])
def test_real_incident_regressions(workdir, case_name, content, expect_blocked, origin):
    """真实事故回归：每条注释写出处（任务书 §二 第 4 类）。"""
    repo = _init_repo(workdir)
    _write(repo, "src/incident_replay.txt", content)
    _add(repo, "src/incident_replay.txt")
    r = _commit(repo)
    if expect_blocked:
        assert r.returncode != 0, f"[{case_name}] 事故形态被放行了！出处：{origin}\nstderr={r.stderr}"
    else:
        assert r.returncode == 0, (
            f"[{case_name}] 事故（误报）复发了！出处：{origin}\nstderr={r.stderr}"
        )


# ---- 第 5 类 · 元行为（逃生口 + 纵深防御 + fail-closed） ----------------------


def test_skip_env_commit_passes_with_warning(workdir):
    """SECRET_SCAN_SKIP=1 逃生口：pre-commit 显式跳过且打印警告留痕。
    消融：删掉 pre-commit 顶部的 SKIP 分支 → 本用例红（commit 被拦）。"""
    repo = _init_repo(workdir)
    _write(repo, "src/bypass.txt", f'api_key = "{FAKE_GENERIC_KEY}"\n')
    _add(repo, "src/bypass.txt")
    r = _commit(repo, env_extra={"SECRET_SCAN_SKIP": "1"})
    assert r.returncode == 0, f"逃生口失效：{r.stderr}"
    assert "SECRET_SCAN_SKIP=1" in r.stderr, f"逃生口必须留痕打印警告：{r.stderr}"
    assert "pre-commit" in r.stderr


def test_push_blocks_what_commit_skipped(workdir):
    """🔴 纵深防御核心：commit 侧被 SECRET_SCAN_SKIP=1 绕过的假 key，
    push 侧必须兜住（pre-push 是公开仓库暴露前的最后一道网，f58af96 有真实前科）。
    消融：删掉 pre-push 的 scan_diff 调用 / 删掉情况 3 的内容扫描 → 本用例红。"""
    base = workdir
    repo = _init_repo(base)
    _write(repo, "src/leaked.txt", f'api_key = "{FAKE_GENERIC_KEY}"\n')
    _add(repo, "src/leaked.txt")
    r = _commit(repo, env_extra={"SECRET_SCAN_SKIP": "1"})
    assert r.returncode == 0, f"前置失败：SKIP 逃生口没生效，commit 应该被放行：{r.stderr}"
    remote = _init_bare_remote(base)
    pr = _push(repo, remote)
    assert pr.returncode != 0, f"push 侧没兜住 commit 侧被绕过的假 key！{pr.stdout}"
    assert ("拦截" in pr.stderr) or ("疑似密钥" in pr.stderr) or ("pre-push" in pr.stderr), pr.stderr


def test_skip_env_push_passes_with_warning(workdir):
    """SECRET_SCAN_SKIP=1 逃生口在 push 侧同样生效且打印警告（两钩子同名同语义）。
    消融：删掉 pre-push 顶部的 SKIP 分支 → 本用例红。
    前置用 --no-verify 提交（模拟没装钩子的 clone 绕过 commit 侧），push 侧若没有
    SKIP 逃生口必然拦截 —— 因此本用例同时证明「逃生口生效」与「命中是真的」。"""
    base = workdir
    repo = _init_repo(base)
    _write(repo, "src/fp2.txt", f"mail: {FAKE_EMAIL}\n")
    _add(repo, "src/fp2.txt")
    _run(["git", "commit", "-m", "trigger", "--no-verify"], cwd=repo)
    remote = _init_bare_remote(base)
    pr = _push(repo, remote, env_extra={"SECRET_SCAN_SKIP": "1"})
    assert pr.returncode == 0, f"push 侧逃生口失效：{pr.stderr}"
    assert "SECRET_SCAN_SKIP=1" in pr.stderr, f"push 侧逃生口必须留痕：{pr.stderr}"
    assert "pre-push" in pr.stderr


def test_push_clean_history_passes(workdir):
    """干净历史正常 push 放行（保证闸门没有把正常开发也拦死）。"""
    base = workdir
    repo = _init_repo(base)
    _write(repo, "src/clean.rs", "fn main() { println!(\"hello\"); }\n")
    _add(repo, "src/clean.rs")
    assert _commit(repo).returncode == 0
    remote = _init_bare_remote(base)
    pr = _push(repo, remote)
    assert pr.returncode == 0, f"干净历史被误拦：{pr.stderr}"


def test_pre_push_fail_closed_on_diverged_remote(workdir):
    """fail-closed（HOOK-FAIL-OPEN-001）：pre-push 取不到 diff 时必须拒绝放行。
    构造：同一 bare 远端被两段不相干历史先后推送 —— 第二个仓库没有远端已有提交的
    对象，hook_diff remote_sha..local_sha 必然 fatal，钩子应走 fail-closed 分支
    （拒绝放行 + 指引），而不是静默放过。
    消融：把 pre-push 里 `if ! DIFF_RAW=$(hook_diff ...)` 的 ! 去掉（失败继续走）→ 本用例红。
    （pre-commit 侧的 fail-closed 无法在不污染判定的前提下构造：让 `git diff --cached`
    失败的现实手段只有破坏索引，但索引坏了 commit 本身也会失败，断言失去意义 ——
    如实说明：pre-commit fail-closed 未构造，靠代码评审 + pre-push 同源逻辑兜底。）"""
    base = workdir
    remote = _init_bare_remote(base)
    # 仓库 A：干净提交并推上去，远端 main 现在指向 A 的提交
    repo_a = _init_repo(base, "repo_a")
    _write(repo_a, "src/a.txt", "a\n")
    _add(repo_a, "src/a.txt")
    assert _commit(repo_a).returncode == 0
    pa = _push(repo_a, remote)
    assert pa.returncode == 0, f"前置失败：A 的干净 push 应放行：{pa.stderr}"
    # 仓库 B：不相干的独立历史，推同一分支 —— B 没有远端已有对象
    repo_b = _init_repo(base, "repo_b")
    _write(repo_b, "src/b.txt", "b\n")
    _add(repo_b, "src/b.txt")
    assert _commit(repo_b).returncode == 0
    pb = _push(repo_b, remote)
    assert pb.returncode != 0, "B 的 push 被放行了 —— fail-closed 失效（取不到 diff 必须拒绝放行）"
    assert ("拒绝放行" in pb.stderr) or ("fail-closed" in pb.stderr) or ("无法取得" in pb.stderr), (
        f"push 失败但不是 fail-closed 拦的（可能是 git 自身拒绝），钩子的 fail-closed 分支没走到：{pb.stderr}"
    )
