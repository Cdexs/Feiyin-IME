fn main() {
    // ACC-ENGINE-LLAMACPP-452：Qwen3-ASR 精解引擎适配层（运行时动态加载 llama.cpp 官方预编译库，
    // 头文件为同版本 b11207 官方头文件，见 native/llama_asr/）。
    // 🔴 不输出 rerun-if-changed：保持原「包内任一文件变更即重跑」语义（winres 版本资源依赖此行为）。
    cc::Build::new()
        .cpp(true)
        .file("native/llama_asr/shim.cpp")
        .include("native/llama_asr/include")
        .flag_if_supported("/std:c++17")
        .flag_if_supported("/EHsc")
        .flag_if_supported("/utf-8")
        .flag_if_supported("-std=c++17")
        .warnings(false)
        .emit_rerun_if_env_changed(false)
        .compile("llama_asr_shim");

    #[cfg(target_os = "windows")]
    {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let parts: Vec<&str> = version.split('.').collect();
        let win_ver = format!(
            "{}.{}.{}.0",
            parts.first().unwrap_or(&"0"),
            parts.get(1).unwrap_or(&"0"),
            parts.get(2).unwrap_or(&"0")
        );
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icons/app.ico");
        res.set("ProductName", "飞音智能语音输入");
        res.set("FileDescription", "飞音智能语音输入");
        res.set("FileVersion", &win_ver);
        res.set("ProductVersion", &win_ver);
        res.set("OriginalFilename", "feiyin-ime.exe");
        res.compile().expect("winres compile failed");
    }

    // MACOS-P4-RPATH-001: 主程序缺 LC_RPATH，直接执行二进制时 dyld 无法按 @rpath 展开
    // 动态库路径，必然终止（cargo run 自动注入 DYLD_FALLBACK_LIBRARY_PATH 才掩盖了它；
    // .app 打包后双击即直接执行二进制，是致命缺陷）。
    // 链接期注入 rpath，dyld 在二进制所在目录（@executable_path）查找 dylib：
    // - target/debug|release/ 直接执行：dylib 已与二进制同级
    // - .app 打包：dylib 若放 Contents/MacOS（当前 BUNDLE 布局）或 Contents/Frameworks
    //   （未来布局），两条 rpath 并存覆盖两者
    // 用 build.rs 而非共享的 .cargo/config.toml（根 crate + src-tauri 双项目共用，
    // DEC-033 附则三高风险文件）：cargo:rustc-link-arg 仅作用于本 package 全部 target
    // （feiyin-ime / crash-reporter / tests），作用域精确且对 Windows 是 cfg 级 no-op。
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
    }
}
