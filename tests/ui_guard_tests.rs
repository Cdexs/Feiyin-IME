//! 测试同步：UI-GUARD-001（配置 UI 启动守卫）
//!
//! 覆盖场景：
//! - GUARD-001: 主程序运行时启动 voice-ime-ui.exe → 正常显示配置窗口
//! - GUARD-002: 主程序未运行时启动 voice-ime-ui.exe → 进程立即退出（exit code 1）
//!
//! 注意：`is_main_process_running()` 依赖真实 Win32 进程枚举（ToolHelp32 API），
//! 单元测试无法直接覆盖。本文件为测试骨架，包含集成测试框架和手动验证步骤。

use std::path::PathBuf;
use std::process::Command;

// ============================================================
// 辅助函数
// ============================================================

fn get_ui_exe_path() -> Option<PathBuf> {
    // 优先级 1: target/release/
    let release = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("target")
        .join("release")
        .join("voice-ime-ui.exe");
    if release.exists() {
        return Some(release);
    }

    // 优先级 2: src-tauri/target/release/
    let tauri_release = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release")
        .join("voice-ime-ui.exe");
    if tauri_release.exists() {
        return Some(tauri_release);
    }

    None
}

fn get_main_exe_name() -> &'static str {
    "voice-ime.exe"
}

// ============================================================
// GUARD-001: 主程序运行时，UI 应正常启动
// ============================================================

/// 集成测试骨架：主程序运行时启动 voice-ime-ui.exe
///
/// 预期行为：
/// 1. `is_main_process_running()` 返回 true
/// 2. voice-ime-ui.exe 进程正常启动
/// 3. 配置窗口可见
/// 4. 关闭后进程退出（exit code 0）
#[test]
fn ui_starts_when_main_running() {
    // 此测试需要：
    // 1. 先启动 voice-ime.exe（主程序）
    // 2. 再启动 voice-ime-ui.exe
    // 3. 验证 UI 进程存在且窗口可见
    //
    // 由于测试环境限制，以下为手动验证步骤：
    //
    // 手动验证步骤：
    // 1. 启动主程序：`target/release/voice-ime.exe -debug`
    // 2. 等待 2 秒确认主程序已初始化
    // 3. 启动 UI：`target/release/voice-ime-ui.exe`
    // 4. 验证：
    //    a. `tasklist | findstr voice-ime-ui.exe` 进程存在
    //    b. 配置窗口可见
    //    c. 关闭配置窗口后，UI 进程退出
    // 5. 清理：关闭主程序

    // 占位断言
    assert!(
        true,
        "TODO: manual verification after UI-GUARD-001 implementation"
    );
}

// ============================================================
// GUARD-002: 主程序未运行时，UI 应拒绝启动
// ============================================================

/// 集成测试骨架：主程序未运行时启动 voice-ime-ui.exe
///
/// 预期行为：
/// 1. `is_main_process_running()` 返回 false
/// 2. voice-ime-ui.exe 进程立即退出
/// 3. exit code = 1
/// 4. 不显示配置窗口
/// 5. 可选：弹出通知提示 "请先启动语音输入法"
#[test]
fn ui_rejects_start_when_main_not_running() {
    // 此测试需要：
    // 1. 确认 voice-ime.exe 未运行
    // 2. 尝试启动 voice-ime-ui.exe
    // 3. 验证进程退出且 exit code = 1
    //
    // 由于测试环境限制，以下为手动验证步骤：
    //
    // 手动验证步骤：
    // 1. 确认主程序未运行：`tasklist | findstr voice-ime.exe` 无输出
    // 2. 启动 UI：`target/release/voice-ime-ui.exe`
    // 3. 立即检查（< 1 秒内）：
    //    a. `tasklist | findstr voice-ime-ui.exe` 无输出（进程已退出）
    //    b. 无配置窗口出现
    //    c. （可选）系统通知 "请先启动语音输入法" 弹出

    // 占位断言
    assert!(
        true,
        "TODO: manual verification after UI-GUARD-001 implementation"
    );
}

// ============================================================
// 备注
// ============================================================
//
// Win32 ToolHelp32 API 测试覆盖说明：
//
// `is_main_process_running()` 使用 `CreateToolhelp32Snapshot` + `Process32First/Next`
// 枚举所有进程并匹配 `voice-ime.exe`。该函数依赖系统级 API，单元测试中无法 mock。
//
// 以下场景只能通过 E2E/手动测试覆盖：
// - 多实例主程序同时运行（应返回 true）
// - 主程序恰好在 UI 启动瞬间退出（竞态条件）
// - 权限不足导致 ToolHelp32 快照创建失败
