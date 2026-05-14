fn main() {
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
}