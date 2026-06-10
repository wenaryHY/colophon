//! SSRF 防护：URL / IP 地址安全检查
//!
//! 由两个调用方共享：
//! - `service` 在创建 / 更新 webhook 时校验目标 URL（拒绝内网地址）
//! - `dispatcher` 在投递时做 DNS 重绑定与重定向二次检查
//!
//! 集中于此，使安全敏感的 IP 段判定逻辑只有一处来源。

use std::net::Ipv6Addr;

use crate::shared::error::AppError;

/// 检查 URL 是否指向私有 IP 或 localhost
///
/// 防止 SSRF 攻击：拒绝 webhook 指向内网地址
/// 设置 `INKFORGE_TEST_MODE=true` 环境变量可跳过检查（仅供集成测试使用）
pub(super) fn is_private_or_local_url(url: &str) -> Result<bool, AppError> {
    // 集成测试模式下跳过 SSRF 检查，允许 webhook 使用 localhost 进行端到端测试
    if std::env::var("INKFORGE_TEST_MODE").is_ok() {
        return Ok(false);
    }

    let parsed = url::Url::parse(url)
        .map_err(|_| AppError::BadRequest("无效的 URL 格式".into()))?;

    // url::Host 枚举区分 Domain / Ipv4 / Ipv6，避免手动处理 IPv6 的方括号
    let host = parsed
        .host()
        .ok_or_else(|| AppError::BadRequest("URL 缺少 host".into()))?;

    match host {
        url::Host::Domain(domain) => {
            let lowered = domain.to_ascii_lowercase();
            // localhost 及其子域
            if lowered == "localhost" || lowered.ends_with(".localhost") {
                return Ok(true);
            }
            // 域名走 DNS，无法在此判定是否解析到内网；交给后续传输层即可
            // 注：理想方案是 resolve 后再比对 IP，但 DNS 重绑定攻击需要更深防御
            Ok(false)
        }
        url::Host::Ipv4(ipv4) => {
            // 0.0.0.0/8
            if ipv4.octets()[0] == 0 {
                return Ok(true);
            }
            // 10.0.0.0/8
            if ipv4.octets()[0] == 10 {
                return Ok(true);
            }
            // 127.0.0.0/8 (loopback)
            if ipv4.octets()[0] == 127 {
                return Ok(true);
            }
            // 169.254.0.0/16 (链路本地)
            if ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254 {
                return Ok(true);
            }
            // 172.16.0.0/12
            if ipv4.octets()[0] == 172 && (ipv4.octets()[1] >= 16 && ipv4.octets()[1] <= 31) {
                return Ok(true);
            }
            // 192.168.0.0/16
            if ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168 {
                return Ok(true);
            }
            Ok(false)
        }
        url::Host::Ipv6(ipv6) => {
            // ::1 (loopback)
            if ipv6 == Ipv6Addr::LOCALHOST {
                return Ok(true);
            }
            // :: (unspecified)
            if ipv6.is_unspecified() {
                return Ok(true);
            }
            // fe80::/10 (链路本地)
            if ipv6.segments()[0] & 0xffc0 == 0xfe80 {
                return Ok(true);
            }
            // fc00::/7 (唯一本地)
            if ipv6.segments()[0] & 0xfe00 == 0xfc00 {
                return Ok(true);
            }
            // ::ffff:0:0/96 (IPv4-mapped) — 转回 IPv4 检查
            if let Some(v4) = ipv6.to_ipv4_mapped() {
                let mapped = format!("http://{}/", v4);
                return is_private_or_local_url(&mapped);
            }
            Ok(false)
        }
    }
}

/// 检查 IP 地址是否为私有或本地地址
pub(super) fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            // 0.0.0.0/8
            ipv4.octets()[0] == 0
                // 10.0.0.0/8
                || ipv4.octets()[0] == 10
                // 127.0.0.0/8 (loopback)
                || ipv4.octets()[0] == 127
                // 169.254.0.0/16 (link-local)
                || (ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254)
                // 172.16.0.0/12
                || (ipv4.octets()[0] == 172 && (ipv4.octets()[1] >= 16 && ipv4.octets()[1] <= 31))
                // 192.168.0.0/16
                || (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168)
        }
        std::net::IpAddr::V6(ipv6) => {
            // ::1 (loopback)
            *ipv6 == Ipv6Addr::LOCALHOST
                // :: (unspecified)
                || ipv6.is_unspecified()
                // fe80::/10 (link-local)
                || (ipv6.segments()[0] & 0xffc0 == 0xfe80)
                // fc00::/7 (unique local)
                || (ipv6.segments()[0] & 0xfe00 == 0xfc00)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_private_or_local_url() {
        // localhost
        assert!(is_private_or_local_url("http://localhost/api").unwrap());
        assert!(is_private_or_local_url("http://127.0.0.1:8080/").unwrap());
        assert!(is_private_or_local_url("http://[::1]/").unwrap());
        assert!(is_private_or_local_url("http://app.localhost/").unwrap());

        // 私有 IP (10.x.x.x)
        assert!(is_private_or_local_url("http://10.0.0.1/").unwrap());
        assert!(is_private_or_local_url("http://10.255.255.255/").unwrap());

        // 私有 IP (172.16-31.x.x)
        assert!(is_private_or_local_url("http://172.16.0.1/").unwrap());
        assert!(is_private_or_local_url("http://172.31.255.255/").unwrap());

        // 私有 IP (192.168.x.x)
        assert!(is_private_or_local_url("http://192.168.1.1/").unwrap());

        // 链路本地 (169.254.x.x)
        assert!(is_private_or_local_url("http://169.254.1.1/").unwrap());

        // 0.0.0.0/8
        assert!(is_private_or_local_url("http://0.0.0.0/").unwrap());

        // IPv6 私有/链路本地/唯一本地
        assert!(is_private_or_local_url("http://[fe80::1]/").unwrap());
        assert!(is_private_or_local_url("http://[fc00::1]/").unwrap());
        assert!(is_private_or_local_url("http://[fd00::1]/").unwrap());

        // 公网 IP（允许）
        assert!(!is_private_or_local_url("https://api.example.com/webhook").unwrap());
        assert!(!is_private_or_local_url("http://8.8.8.8/").unwrap());
        assert!(!is_private_or_local_url("https://1.1.1.1/").unwrap());

        // 边界：172.15.x.x 与 172.32.x.x 不在私有段
        assert!(!is_private_or_local_url("http://172.15.0.1/").unwrap());
        assert!(!is_private_or_local_url("http://172.32.0.1/").unwrap());

        // 边界：192.169.x.x 不在私有段
        assert!(!is_private_or_local_url("http://192.169.1.1/").unwrap());

        // 无效 URL
        assert!(is_private_or_local_url("not a url").is_err());
    }

    #[test]
    fn test_is_private_ip() {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

        // IPv4 私有地址
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))));

        // IPv4 公网地址
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 15, 0, 1))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1))));

        // IPv6 私有地址
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1))));

        // IPv6 公网地址
        assert!(!is_private_ip(&IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888))));
    }
}
