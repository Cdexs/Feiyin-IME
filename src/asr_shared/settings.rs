//! ASR-SERVER-491：识别服务设置（`asr-service.json`）。
//!
//! - `enabled` = 唯一的总开关（DEC-031 / 设计 §8.1）：设置页「启动 / 停止」与托盘菜单写它，内核据此拉起 / 结束网关。
//! - 暂停**不**存这里（设计 §16.2：暂停只是临时的，重启输入法恢复为运行中）。

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::PathBuf;

/// 默认端口（TCP；`wss://` 与二期 LiveKit 信令共用入口）。
pub const DEFAULT_PORT: u16 = 7860;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServiceSettings {
    /// 总开关。
    pub enabled: bool,
    /// 本机监听端口（内网直接用；外网经路由器端口映射到它）。
    pub port: u16,
    /// 外网 IP 或域名（选填；空 = 只能内网接入）。
    pub wan_host: String,
    /// 外网端口（0 = 与 `port` 相同）。
    pub wan_port: u16,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            wan_host: String::new(),
            wan_port: 0,
        }
    }
}

impl ServiceSettings {
    pub fn path() -> PathBuf {
        super::data_dir().join("asr-service.json")
    }

    /// 读设置；文件不存在 / 损坏 ⇒ 默认值（服务关）。
    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        super::write_atomic(&Self::path(), &bytes)
    }

    /// 内网 `本机地址:端口`（取不到 ⇒ `None`）。
    /// 先枚举物理网卡（以太网 / Wi-Fi，排除 TUN / 隧道 / 回环 / 169.254）按规则挑；
    /// 挑不出再退回「默认路由」法：对公网地址做一次 UDP `connect`（只选路由，不发包）读本端地址。
    /// 全局 TUN 代理会把默认路由劫持到虚拟网卡，所以路由法只作兜底与同级比较。
    pub fn lan_endpoint(&self) -> Option<String> {
        let route = route_local_v4();
        let ip = enumerate_nics()
            .and_then(|nics| pick_lan_ip(&nics, route))
            .or(route)?;
        Some(format!("{ip}:{}", self.port))
    }

    /// 外网 `主机:端口`（未填 ⇒ `None`）。
    pub fn wan_endpoint(&self) -> Option<String> {
        let host = self.wan_host.trim();
        if host.is_empty() {
            return None;
        }
        let port = if self.wan_port == 0 {
            self.port
        } else {
            self.wan_port
        };
        Some(if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        })
    }
}

/// 网卡类别（平台枚举负责映射；只有 `Physical` 能入选）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NicKind {
    /// 以太网 / Wi-Fi 等真实网卡。
    Physical,
    /// TUN / Wintun / 隧道 / 回环 / PPP 等虚拟网卡。
    Virtual,
}

/// 一个 IPv4 单播地址及其网卡属性（平台枚举 → 纯函数选择的中间表示）。
#[derive(Debug, Clone)]
pub(crate) struct NicV4 {
    pub ip: Ipv4Addr,
    pub kind: NicKind,
    pub up: bool,
    /// 优先级标记：Windows = 有 IPv4 网关；unix = 网卡名以 `en` 开头。
    pub preferred: bool,
    pub name: String,
}

/// 默认路由法：对公网地址 UDP `connect`（不发包）读本端 IPv4。
fn route_local_v4() -> Option<Ipv4Addr> {
    let s = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("8.8.8.8", 53)).ok()?;
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// 纯函数：从候选里挑内网 IPv4。
/// 候选 = 已启用 + 物理类 + 非 169.254 / 127；优先级：`preferred` > 等于路由法结果 > 枚举顺序。
pub(crate) fn pick_lan_ip(nics: &[NicV4], route: Option<Ipv4Addr>) -> Option<Ipv4Addr> {
    nics.iter()
        .filter(|n| {
            n.up && n.kind == NicKind::Physical
                && !n.ip.is_link_local()
                && !n.ip.is_loopback()
                && !n.ip.is_unspecified()
        })
        // min_by_key 同分取第一个 ⇒ 天然保持枚举顺序
        .min_by_key(|n| (!n.preferred, Some(n.ip) != route))
        .map(|n| n.ip)
}

#[cfg(windows)]
fn enumerate_nics() -> Option<Vec<NicV4>> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST,
        GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

    const IF_TYPE_ETHERNET: u32 = 6;
    const IF_TYPE_WIFI: u32 = 71;
    let flags = GAA_FLAG_INCLUDE_GATEWAYS
        | GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_MULTICAST
        | GAA_FLAG_SKIP_DNS_SERVER;

    // 先给个初始大小，溢出时按返回的 size 重试（网卡热插拔可能让大小变化）。
    let mut size: u32 = 16 * 1024;
    let mut buf: Vec<u64> = Vec::new(); // u64 保证 8 字节对齐
    let mut ok = false;
    for _ in 0..4 {
        buf.resize((size as usize).div_ceil(8), 0);
        let rc = unsafe {
            GetAdaptersAddresses(
                AF_INET.0 as u32,
                flags,
                None,
                Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut size,
            )
        };
        match rc {
            0 => {
                ok = true;
                break;
            }
            111 => continue, // ERROR_BUFFER_OVERFLOW：size 已更新
            _ => return None,
        }
    }
    if !ok {
        return None;
    }

    let mut out = Vec::new();
    let mut cur = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !cur.is_null() {
        let a = unsafe { &*cur };
        let kind = if a.IfType == IF_TYPE_ETHERNET || a.IfType == IF_TYPE_WIFI {
            NicKind::Physical
        } else {
            NicKind::Virtual
        };
        let up = a.OperStatus == IfOperStatusUp;
        let has_gateway = !a.FirstGatewayAddress.is_null();
        let name = unsafe { a.FriendlyName.to_string().unwrap_or_default() };
        let mut u = a.FirstUnicastAddress;
        while !u.is_null() {
            let ua = unsafe { &*u };
            let sa = ua.Address.lpSockaddr;
            if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                let v4 = unsafe { &*(sa as *const SOCKADDR_IN) };
                let octets = unsafe { v4.sin_addr.S_un.S_addr }.to_ne_bytes();
                out.push(NicV4 {
                    ip: Ipv4Addr::from(octets),
                    kind,
                    up,
                    preferred: has_gateway,
                    name: name.clone(),
                });
            }
            u = ua.Next;
        }
        cur = a.Next;
    }
    Some(out)
}

#[cfg(unix)]
fn enumerate_nics() -> Option<Vec<NicV4>> {
    use std::ffi::CStr;

    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut cur = head;
    while !cur.is_null() {
        let ifa = unsafe { &*cur };
        cur = ifa.ifa_next;
        if ifa.ifa_addr.is_null() || unsafe { (*ifa.ifa_addr).sa_family } as i32 != libc::AF_INET {
            continue;
        }
        let flags = ifa.ifa_flags as i32;
        let up = flags & libc::IFF_UP != 0 && flags & libc::IFF_RUNNING != 0;
        let virt = flags & (libc::IFF_LOOPBACK | libc::IFF_POINTOPOINT) != 0;
        let sin = unsafe { &*(ifa.ifa_addr as *const libc::sockaddr_in) };
        let name = unsafe { CStr::from_ptr(ifa.ifa_name) }
            .to_string_lossy()
            .into_owned();
        out.push(NicV4 {
            ip: Ipv4Addr::from(sin.sin_addr.s_addr.to_ne_bytes()),
            kind: if virt {
                NicKind::Virtual
            } else {
                NicKind::Physical
            },
            up,
            preferred: name.starts_with("en"),
            name,
        });
    }
    unsafe { libc::freeifaddrs(head) };
    Some(out)
}

#[cfg(not(any(windows, unix)))]
fn enumerate_nics() -> Option<Vec<NicV4>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nic(ip: [u8; 4], kind: NicKind, up: bool, preferred: bool, name: &str) -> NicV4 {
        NicV4 {
            ip: Ipv4Addr::from(ip),
            kind,
            up,
            preferred,
            name: name.into(),
        }
    }

    #[test]
    fn pick_skips_tun_and_link_local() {
        // Gavin 本机：路由法被 xray_tun 劫持
        let nics = vec![
            nic([169, 254, 134, 73], NicKind::Physical, true, false, "蓝牙"),
            nic([192, 168, 1, 10], NicKind::Physical, true, true, "以太网 5"),
            nic([169, 254, 1, 1], NicKind::Physical, true, false, "WLAN"),
            nic([169, 254, 2, 2], NicKind::Virtual, true, false, "本地连接*"),
            nic([172, 18, 0, 1], NicKind::Virtual, true, false, "xray_tun"),
        ];
        let route = Some(Ipv4Addr::new(172, 18, 0, 1));
        assert_eq!(
            pick_lan_ip(&nics, route),
            Some(Ipv4Addr::new(192, 168, 1, 10))
        );
    }

    #[test]
    fn pick_prefers_gateway_over_vethernet() {
        let nics = vec![
            nic(
                [172, 28, 0, 1],
                NicKind::Physical,
                true,
                false,
                "vEthernet (WSL)",
            ),
            nic([192, 168, 1, 10], NicKind::Physical, true, true, "以太网"),
        ];
        // 即便路由法恰好给了 vEthernet，网关优先
        let route = Some(Ipv4Addr::new(172, 28, 0, 1));
        assert_eq!(
            pick_lan_ip(&nics, route),
            Some(Ipv4Addr::new(192, 168, 1, 10))
        );
    }

    #[test]
    fn pick_all_link_local_falls_back() {
        let nics = vec![
            nic([169, 254, 1, 1], NicKind::Physical, true, true, "以太网"),
            nic([192, 168, 9, 9], NicKind::Physical, false, true, "down"),
        ];
        assert_eq!(
            pick_lan_ip(&nics, None),
            None,
            "无候选 ⇒ None，调用方退回路由法"
        );
        assert_eq!(pick_lan_ip(&[], None), None);
    }

    #[test]
    fn pick_same_level_prefers_route_ip() {
        let nics = vec![
            nic([192, 168, 1, 10], NicKind::Physical, true, true, "以太网"),
            nic([10, 0, 0, 5], NicKind::Physical, true, true, "WLAN"),
        ];
        assert_eq!(
            pick_lan_ip(&nics, Some(Ipv4Addr::new(10, 0, 0, 5))),
            Some(Ipv4Addr::new(10, 0, 0, 5))
        );
        assert_eq!(
            pick_lan_ip(&nics, None),
            Some(Ipv4Addr::new(192, 168, 1, 10)),
            "路由法无结果 ⇒ 枚举顺序"
        );
    }

    /// 本机真实网卡验证：`cargo test --bin feiyin-ime lan_endpoint_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn lan_endpoint_real_machine() {
        let s = ServiceSettings::default();
        println!("nics = {:#?}", enumerate_nics());
        println!("route = {:?}", route_local_v4());
        println!("lan_endpoint = {:?}", s.lan_endpoint());
    }

    #[test]
    fn wan_endpoint_rules() {
        let mut s = ServiceSettings::default();
        assert_eq!(s.wan_endpoint(), None, "未填 ⇒ 只能内网");
        s.wan_host = "asr.example.com".into();
        assert_eq!(s.wan_endpoint().as_deref(), Some("asr.example.com:7860"));
        s.wan_port = 443;
        assert_eq!(s.wan_endpoint().as_deref(), Some("asr.example.com:443"));
        s.wan_host = "2001:db8::1".into();
        assert_eq!(
            s.wan_endpoint().as_deref(),
            Some("[2001:db8::1]:443"),
            "IPv6 加方括号"
        );
    }

    #[test]
    fn missing_fields_default() {
        let s: ServiceSettings = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(s.enabled);
        assert_eq!(s.port, DEFAULT_PORT);
    }
}
