//! 崩溃报告本地存储
//!
//! 存储路径：{exe所在目录}/crash.json
//! 每次崩溃覆盖（只保留一份）

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 崩溃类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrashType {
    /// Rust panic
    Panic,
    /// Windows 异常（如访问违规）
    Win32Exception,
    /// 线程 panic
    ThreadPanic,
}

impl std::fmt::Display for CrashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrashType::Panic => write!(f, "Panic"),
            CrashType::Win32Exception => write!(f, "Win32 Exception"),
            CrashType::ThreadPanic => write!(f, "Thread Panic"),
        }
    }
}

/// 系统信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// 操作系统版本（如 "Windows 10 Pro 21H2"）
    pub os_version: String,
    /// 系统架构（如 "x86_64"）
    pub arch: String,
    /// 系统语言（如 "zh-CN"）
    pub language: String,
}

/// 运行时状态（不含敏感数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    /// 当前模式（如 "recording", "processing", "idle"）
    pub mode: String,
    /// LLM 是否启用
    pub llm_enabled: bool,
    /// LLM 是否已验证连接
    pub llm_verified: bool,
    /// ASR 模型类型
    pub asr_model: String,
}

impl Default for RuntimeInfo {
    fn default() -> Self {
        Self {
            mode: "unknown".to_string(),
            llm_enabled: false,
            llm_verified: false,
            asr_model: "unknown".to_string(),
        }
    }
}

/// 崩溃报告结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    /// 程序版本
    pub version: String,
    /// 崩溃时间（ISO 8601 格式）
    pub timestamp: String,
    /// 崩溃类型
    pub crash_type: CrashType,
    /// 用户友好的错误消息
    pub crash_message: String,
    /// 崩溃线程名
    pub thread: String,
    /// 堆栈追踪（函数名+文件+行号）
    pub backtrace: Vec<String>,
    /// 系统信息
    pub system: SystemInfo,
    /// 运行时状态（不含敏感数据）
    pub runtime: RuntimeInfo,
    /// 最近日志（不含用户数据）
    pub recent_logs: Vec<String>,
}

/// 获取崩溃报告存储路径
fn get_crash_report_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("crash.json")
}

/// 确保存储目录存在
fn ensure_storage_dir() -> Result<PathBuf> {
    let path = get_crash_report_path();
    let dir = path.parent().context("Invalid crash report path")?;

    if !dir.exists() {
        std::fs::create_dir_all(dir).context("Failed to create crash report directory")?;
    }

    Ok(path)
}

/// 保存崩溃报告到本地
pub fn save_crash_report(report: &CrashReport) -> Result<()> {
    let path = ensure_storage_dir()?;

    let json = serde_json::to_string_pretty(report).context("Failed to serialize crash report")?;

    std::fs::write(&path, json).context("Failed to write crash report")?;

    log::info!("Crash report saved to: {:?}", path);
    Ok(())
}

/// 从本地加载崩溃报告
pub fn load_crash_report() -> Result<Option<CrashReport>> {
    let path = get_crash_report_path();

    if !path.exists() {
        return Ok(None);
    }

    let json = std::fs::read_to_string(&path).context("Failed to read crash report")?;

    let report: CrashReport =
        serde_json::from_str(&json).context("Failed to parse crash report")?;

    Ok(Some(report))
}

/// 删除崩溃报告（发送成功后清理）
pub fn delete_crash_report() -> Result<()> {
    let path = get_crash_report_path();

    if path.exists() {
        std::fs::remove_file(&path).context("Failed to delete crash report")?;
    }

    Ok(())
}
