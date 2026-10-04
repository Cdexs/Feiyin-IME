//! STATS-475：设置界面「我的」页 —— 读取本周使用统计（与主程序共用 `src/usage_stats.rs`，
//! 主程序每次输出最终文字记一条，这里只读汇总）。

#[allow(dead_code)] // 写入接口只在主程序用
#[path = "../../src/usage_stats.rs"]
mod usage_stats_core;

pub use usage_stats_core::WeekUsage;

#[tauri::command]
pub fn get_usage_week() -> Result<WeekUsage, String> {
    usage_stats_core::week_usage().map_err(|err| format!("读取使用统计失败：{}", err))
}
