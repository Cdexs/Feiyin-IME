use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};

/// 获取系统可用的音频输入设备列表
pub fn get_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices: Vec<String> = host
        .input_devices()
        .context("Failed to get input devices")?
        .filter_map(|d| d.name().ok())
        .collect();
    Ok(devices)
}
