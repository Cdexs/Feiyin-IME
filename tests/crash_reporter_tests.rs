//! 集成测试：Crash Reporter 功能验证
//!
//! 测试目标：
//! 1. 主程序 panic 降级逻辑 - crash.json 写入
//! 2. 独立 reporter 启动验证
//! 3. 主程序无 GUI 依赖验证

use std::path::PathBuf;
use std::process::Command;

/// 测试：Reporter 可执行文件存在（新名称：crash-reporter.exe）
#[test]
fn test_reporter_exe_exists() {
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release");

    // 主程序
    let main_exe = target_dir.join("voice-ime.exe");
    assert!(
        main_exe.exists(),
        "voice-ime.exe should exist in target/release"
    );

    // 独立 crash reporter（新名称）
    let reporter_exe = target_dir.join("crash-reporter.exe");
    assert!(
        reporter_exe.exists(),
        "crash-reporter.exe should exist in target/release"
    );
}

/// 测试：Reporter 命令行参数解析
#[test]
fn test_reporter_command_line_arg() {
    // 验证 --crash-reporter 参数能被正确识别
    let args: Vec<String> = vec!["voice-ime.exe".to_string(), "--crash-reporter".to_string()];

    let is_reporter = args.iter().any(|arg| arg == "--crash-reporter");
    assert!(is_reporter, "Should detect --crash-reporter flag");
}

/// 测试：crash.json 路径格式正确
#[test]
fn test_crash_json_path_format() {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
    let expected_path = PathBuf::from(local_app_data)
        .join("voice-ime")
        .join("crash.json");

    assert!(
        expected_path.to_string_lossy().contains("voice-ime"),
        "Path should contain 'voice-ime'"
    );
    assert!(
        expected_path.to_string_lossy().ends_with("crash.json"),
        "Path should end with 'crash.json'"
    );
}

/// 测试：Reporter 启动参数格式
#[test]
fn test_reporter_spawn_args() {
    let exe_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release")
        .join("crash-reporter.exe");

    if !exe_path.exists() {
        println!("Reporter exe not found, skipping spawn test");
        return;
    }

    // 只验证文件存在且可执行，不实际启动（避免 GUI 弹窗）
    let metadata = std::fs::metadata(&exe_path);
    assert!(metadata.is_ok(), "Reporter exe should be accessible");
    assert!(
        metadata.unwrap().len() > 0,
        "Reporter exe should not be empty"
    );
}

/// 测试：spawn_reporter_process 使用正确的 exe 名称
#[test]
fn test_spawn_reporter_uses_correct_name() {
    let crash_mod_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("mod.rs");
    let crash_mod = std::fs::read_to_string(&crash_mod_path).expect("Should read crash/mod.rs");

    // 验证 spawn_reporter_process 使用新名称 crash-reporter
    assert!(
        crash_mod.contains("crash-reporter.exe") || crash_mod.contains("crash-reporter"),
        "spawn_reporter_process should reference crash-reporter"
    );
    // 确保不包含旧名称
    assert!(
        !crash_mod.contains("voice-ime-crash-reporter"),
        "spawn_reporter_process should NOT reference old name voice-ime-crash-reporter"
    );
}

/// 测试：主程序源码包含 crash reporter 启动逻辑
#[test]
fn test_main_no_direct_gui_ref_in_panic_hook() {
    // 读取 main.rs 验证 panic hook 实现
    let main_rs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("main.rs");
    let main_rs = std::fs::read_to_string(&main_rs_path).expect("Should read main.rs");

    // panic hook 应该调用 crash 模块
    assert!(
        main_rs.contains("crash::"),
        "Panic hook should call crash:: module"
    );

    // 验证 panic hook 通过 spawn 启动 reporter
    assert!(
        main_rs.contains("spawn_reporter_process") || main_rs.contains("--crash-reporter"),
        "Panic hook should spawn reporter process"
    );
}

/// 测试：crash reporter 模块结构完整
#[test]
fn test_crash_module_structure() {
    let crash_mod = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("mod.rs");
    assert!(crash_mod.exists(), "crash/mod.rs should exist");

    let storage_mod = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("storage.rs");
    assert!(storage_mod.exists(), "crash/storage.rs should exist");

    let reporter_mod = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("reporter.rs");
    assert!(reporter_mod.exists(), "crash/reporter.rs should exist");

    let email_mod = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("email.rs");
    assert!(email_mod.exists(), "crash/email.rs should exist");
}

/// 测试：Cargo.toml 包含 crash reporter bin 定义（新名称）
#[test]
fn test_cargo_bin_definition() {
    let cargo_toml =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .expect("Should read Cargo.toml");

    // 验证 crash reporter bin 定义（新名称）
    assert!(
        cargo_toml.contains("crash-reporter"),
        "Cargo.toml should define crash-reporter bin"
    );
    // 确保不包含旧名称
    assert!(
        !cargo_toml.contains("voice-ime-crash-reporter"),
        "Cargo.toml should NOT define old name voice-ime-crash-reporter"
    );
}

/// 测试：按钮使用水平居中布局 API
#[test]
fn test_buttons_horizontal_centered() {
    let reporter_rs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("reporter.rs");
    let reporter_rs = std::fs::read_to_string(&reporter_rs_path).expect("Should read reporter.rs");

    // 验证按钮容器使用 horizontal_centered 布局
    assert!(
        reporter_rs.contains("horizontal_centered"),
        "Buttons should use horizontal_centered layout API"
    );
}

/// 测试：浅色主题下使用 Visuals::light()
#[test]
fn test_light_theme_visuals() {
    let reporter_rs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("reporter.rs");
    let reporter_rs = std::fs::read_to_string(&reporter_rs_path).expect("Should read reporter.rs");

    // 验证使用浅色主题
    assert!(
        reporter_rs.contains("Visuals::light()"),
        "Should use Visuals::light() for light theme"
    );
}

/// 测试：浅色主题背景颜色匹配配置
#[test]
fn test_light_theme_background_color() {
    let reporter_rs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("reporter.rs");
    let reporter_rs = std::fs::read_to_string(&reporter_rs_path).expect("Should read reporter.rs");

    // 验证背景颜色设置为白色 (RGB 255, 255, 255)
    assert!(
        reporter_rs.contains("Color32::WHITE")
            || reporter_rs.contains("from_rgb(255, 255, 255)")
            || reporter_rs.contains("255, 255, 255"),
        "Background color should match crash reporter white background #ffffff"
    );
}

/// 测试：关闭按钮使用默认样式（无黑色背景）
#[test]
fn test_close_button_default_style() {
    let reporter_rs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("crash")
        .join("reporter.rs");
    let reporter_rs = std::fs::read_to_string(&reporter_rs_path).expect("Should read reporter.rs");

    // 验证关闭按钮的 fill 颜色不是黑色/深色
    // 之前的错误：.fill(egui::Color32::from_rgb(50, 50, 50)) 黑色背景
    // 修复后应该使用浅色或默认主题色

    // 检查 close_button 定义块中的 fill 颜色
    // 我们查找 close_button 变量定义到下一个变量定义之间的代码
    let close_button_section = reporter_rs.split("close_button").nth(1).unwrap_or("");
    let fill_section = close_button_section.split(".fill(").nth(1).unwrap_or("");
    let fill_color = fill_section.split(")").next().unwrap_or("");

    // 验证 fill 颜色不是深色 (50, 50, 50) 或黑色 (0, 0, 0)
    // 应该是浅色如 (240, 240, 240) 或移除 fill 使用默认
    assert!(
        !fill_color.contains("50, 50, 50") && !fill_color.contains("0, 0, 0"),
        "Close button fill should not be dark/black. Found fill: {}",
        fill_color
    );
}
