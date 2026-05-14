use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/Cdexs/Feiyin-IME/releases/latest";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CACHE_FILE: &str = "version_check.json";
const USER_AGENT_PREFIX: &str = "feiyin-ime/";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub current: String,
    pub latest: String,
    pub url: String,
    pub checked_at: String,
}

impl VersionInfo {
    fn cache_path() -> Option<PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .map(|dir| dir.join(CACHE_FILE))
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

pub fn check_and_cache() -> Option<VersionInfo> {
    let current = env_version();
    let latest_info = match fetch_latest_release() {
        Ok(info) => info,
        Err(_) => return None,
    };
    let info = VersionInfo {
        current: current.clone(),
        latest: latest_info.tag_name,
        url: latest_info.html_url,
        checked_at: chrono::Utc::now().to_rfc3339(),
    };
    let _ = save_cache(&info);
    Some(info)
}

#[allow(dead_code)]
pub fn force_check() -> Result<VersionInfo, String> {
    let current = env_version();
    let latest_info = fetch_latest_release().map_err(|e| e.to_string())?;
    let info = VersionInfo {
        current: current.clone(),
        latest: latest_info.tag_name,
        url: latest_info.html_url,
        checked_at: chrono::Utc::now().to_rfc3339(),
    };
    let _ = save_cache(&info);
    Ok(info)
}

#[allow(dead_code)]
pub fn read_cache() -> Option<VersionInfo> {
    let path = VersionInfo::cache_path()?;
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

#[allow(dead_code)]
pub fn compare(current: &str, latest: &str) -> bool {
    let c = parse_version(current);
    let l = parse_version(latest);
    l > c
}

fn env_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn fetch_latest_release() -> Result<GithubRelease, reqwest::Error> {
    let url = format!("{}?user_agent={}", GITHUB_API_URL, USER_AGENT_PREFIX);
    let ua = format!("{}{}", USER_AGENT_PREFIX, env_version());
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    let resp: GithubRelease = client
        .get(&url)
        .header("User-Agent", &ua)
        .send()?
        .json()?;
    Ok(resp)
}

fn save_cache(info: &VersionInfo) -> std::io::Result<()> {
    let path = match VersionInfo::cache_path() {
        Some(p) => p,
        None => return Ok(()),
    };
    let json = serde_json::to_string_pretty(info)?;
    std::fs::write(&path, json)
}

#[allow(dead_code)]
fn parse_version(v: &str) -> Vec<u64> {
    let stripped = v.trim_start_matches('v');
    let version_part = stripped.split('-').next().unwrap_or(stripped);
    version_part
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_higher_version_returns_true() {
        assert!(compare("0.5.3", "v0.5.4"));
    }

    #[test]
    fn compare_equal_version_returns_false() {
        assert!(!compare("0.5.3", "v0.5.3"));
    }

    #[test]
    fn cache_path_uses_exe_directory() {
        let path = VersionInfo::cache_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("version_check.json"));
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        if let Some(dir) = exe_dir {
            assert_eq!(path.parent(), Some(dir.as_path()));
        }
    }

    #[test]
    fn compare_lower_version_returns_false() {
        assert!(!compare("0.6.0", "v0.5.4"));
    }

    #[test]
    fn compare_major_version() {
        assert!(compare("0.9.9", "v1.0.0"));
    }

    #[test]
    fn parse_version_strips_v_prefix() {
        assert_eq!(parse_version("v0.5.4"), vec![0, 5, 4]);
    }

    #[test]
    fn parse_version_without_v() {
        assert_eq!(parse_version("0.5.3"), vec![0, 5, 3]);
    }

    #[test]
    fn parse_version_ignores_prerelease() {
        assert_eq!(parse_version("0.5.3-beta.1"), vec![0, 5, 3]);
    }

    #[test]
    fn parse_version_empty_string() {
        assert_eq!(parse_version(""), Vec::<u64>::new());
    }

    #[test]
    fn parse_version_extra_segments() {
        assert_eq!(parse_version("1.2.3.4"), vec![1, 2, 3, 4]);
    }

    #[test]
    fn parse_version_invalid_numeric() {
        assert_eq!(parse_version("a.b.c"), Vec::<u64>::new());
    }

    #[test]
    fn version_info_json_roundtrip() {
        let info = VersionInfo {
            current: "0.5.3".to_string(),
            latest: "0.5.4".to_string(),
            url: "https://example.com/release".to_string(),
            checked_at: "2026-05-14T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: VersionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.current, "0.5.3");
        assert_eq!(decoded.latest, "0.5.4");
        assert_eq!(decoded.url, "https://example.com/release");
        assert_eq!(decoded.checked_at, "2026-05-14T00:00:00Z");
    }
}