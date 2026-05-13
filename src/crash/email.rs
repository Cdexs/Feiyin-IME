//! 崩溃报告 SMTP 发送
//!
//! 使用 lettre crate 发送邮件到开发者邮箱

use anyhow::{anyhow, Context, Result};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{
    message::{header::ContentType, Mailbox, MessageBuilder},
    SmtpTransport, Transport,
};

use super::storage::CrashReport;

/// SMTP 配置（占位符，后续确认服务商后更新）
/// TODO: 从环境变量或安全存储读取
struct SmtpConfig {
    /// SMTP 服务器地址
    server: String,
    /// SMTP 端口
    port: u16,
    /// 发件邮箱
    sender_email: String,
    /// SMTP 密码/授权码
    password: String,
    /// 收件邮箱（开发者）
    recipient_email: String,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        // 占位符配置（后续确认服务商后更新）
        Self {
            server: "smtp.example.com".to_string(),
            port: 587,
            sender_email: "voice-ime-crash@example.com".to_string(),
            password: String::new(), // 占位符
            recipient_email: "dev@example.com".to_string(),
        }
    }
}

/// 发送崩溃报告邮件
pub fn send_crash_email(report: &CrashReport) -> Result<()> {
    let config = SmtpConfig::default();

    // 检查配置是否有效
    if config.password.is_empty() {
        return Err(anyhow!("SMTP 配置未完成，无法发送邮件"));
    }

    // 构建邮件主题
    let subject = format!(
        "[voice-ime] 崩溃报告 - v{} - {}",
        report.version,
        report
            .timestamp
            .split('T')
            .next()
            .unwrap_or(&report.timestamp)
    );

    // 构建邮件内容
    let body = format_crash_email_body(report);

    // 创建邮件消息
    let message = MessageBuilder::new()
        .from(Mailbox::new(
            Some("飞音语音输入".to_string()),
            config
                .sender_email
                .parse()
                .context("Invalid sender email")?,
        ))
        .to(Mailbox::new(
            None,
            config
                .recipient_email
                .parse()
                .context("Invalid recipient email")?,
        ))
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .context("Failed to build email message")?;

    // 创建 SMTP 连接
    let creds = Credentials::new(config.sender_email, config.password);

    let transport = SmtpTransport::relay(&config.server)
        .context("Failed to create SMTP transport")?
        .credentials(creds)
        .port(config.port)
        .build();

    // 发送邮件
    transport.send(&message).context("Failed to send email")?;

    log::info!("Crash report email sent successfully");
    Ok(())
}

/// 格式化邮件内容
fn format_crash_email_body(report: &CrashReport) -> String {
    let mut body = String::new();

    body.push_str("=== 崩溃报告 ===\n\n");

    body.push_str(&format!("版本: {}\n", report.version));
    body.push_str(&format!("时间: {}\n", report.timestamp));
    body.push_str(&format!("类型: {}\n", report.crash_type));
    body.push_str(&format!("消息: {}\n", report.crash_message));
    body.push_str(&format!("线程: {}\n\n", report.thread));

    body.push_str("--- 系统信息 ---\n");
    body.push_str(&format!("OS: {}\n", report.system.os_version));
    body.push_str(&format!("架构: {}\n", report.system.arch));
    body.push_str(&format!("语言: {}\n\n", report.system.language));

    body.push_str("--- 运行时状态 ---\n");
    body.push_str(&format!("模式: {}\n", report.runtime.mode));
    body.push_str(&format!("LLM 启用: {}\n", report.runtime.llm_enabled));
    body.push_str(&format!("LLM 验证: {}\n", report.runtime.llm_verified));
    body.push_str(&format!("ASR 模型: {}\n\n", report.runtime.asr_model));

    body.push_str("--- 堆栈追踪 ---\n");
    for line in &report.backtrace {
        body.push_str(&format!("{}\n", line));
    }
    body.push_str("\n");

    if !report.recent_logs.is_empty() {
        body.push_str("--- 最近日志 ---\n");
        for log in &report.recent_logs {
            body.push_str(&format!("{}\n", log));
        }
    }

    body.push_str("\n=== 报告结束 ===\n");

    body
}
