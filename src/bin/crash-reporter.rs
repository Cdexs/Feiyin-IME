#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[path = "../config/mod.rs"]
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
