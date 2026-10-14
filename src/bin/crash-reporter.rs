#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[path = "../config/mod.rs"]
#[allow(dead_code)] // 只借用配置读取；主程序专用项（快捷方式规则等，SC-LLM-485）在此必然无人用
mod config;
#[path = "../crash/email.rs"]
mod email;
#[path = "../i18n.rs"]
mod i18n;
#[path = "../crash/reporter.rs"]
mod reporter;
#[path = "../crash/storage.rs"]
mod storage;

fn main() {
    reporter::run();
}
