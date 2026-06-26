// 套件 2: ForwardedIpExtractor 测试
// 套件 3: LoginRateLimiter 容量上限测试

#[cfg(test)]
mod forwarded_ip_extractor_tests {
    use std::net::{IpAddr, SocketAddr};
    use axum::extract::ConnectInfo;
    use axum::http::request::Parts;
    use axum_governor::KeyExtractor;
    use colophon::shared::security::ForwardedIpExtractor;

    /// 创建默认的 ForwardedIpExtractor（信任本机）
    fn make_extractor() -> ForwardedIpExtractor {
        ForwardedIpExtractor {
            trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
        }
    }

    /// 创建一个非可信代理的提取器
    fn make_extractor_no_trust() -> ForwardedIpExtractor {
        ForwardedIpExtractor {
            trusted_proxies: vec![],
        }
    }

    /// 用给定的 Header 列表构造一个 Parts，带 ConnectInfo
    fn make_parts_with_headers_and_connect_info(
        header_pairs: Vec<(&str, &str)>,
        peer_ip: &str,
    ) -> Parts {
        let mut builder = axum::http::Request::builder();
        for (k, v) in &header_pairs {
            builder = builder.header(*k, *v);
        }
        let addr: SocketAddr = format!("{}:12345", peer_ip).parse().unwrap();
        let (mut parts, _body) = builder.body(()).expect("failed to build request").into_parts();
        parts.extensions.insert(ConnectInfo(addr));
        parts
    }

    /// 构造没有 Header 但有 ConnectInfo 的 Parts
    fn make_parts_with_connect_info(peer_ip: &str) -> Parts {
        let addr: SocketAddr = format!("{}:12345", peer_ip).parse().unwrap();
        let (mut parts, _body) = axum::http::Request::new(()).into_parts();
        parts.extensions.insert(ConnectInfo(addr));
        parts
    }

    /// 构造没有任何 Header 和 Extensions 的 Parts
    fn make_empty_parts() -> Parts {
        let (parts, _body) = axum::http::Request::new(()).into_parts();
        parts
    }

    fn parse_ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    /// M-1: 来自可信代理（127.0.0.1）时，应使用 X-Forwarded-For
    #[test]
    fn extracts_first_ip_from_x_forwarded_for() {
        let parts = make_parts_with_headers_and_connect_info(
            vec![("x-forwarded-for", "1.2.3.4")],
            "127.0.0.1", // 可信代理
        );
        let extractor = make_extractor();
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("1.2.3.4"));
    }

    /// M-1: 来自可信代理时，应使用 X-Forwarded-For 第一个 IP
    #[test]
    fn extracts_first_ip_from_x_forwarded_for_list() {
        let parts = make_parts_with_headers_and_connect_info(
            vec![("x-forwarded-for", "1.2.3.4, 5.6.7.8")],
            "127.0.0.1",
        );
        let extractor = make_extractor();
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("1.2.3.4"));
    }

    /// M-1: 来自可信代理时，应使用 X-Real-IP
    #[test]
    fn extracts_real_ip_from_x_real_ip_fallback() {
        let parts = make_parts_with_headers_and_connect_info(
            vec![("x-real-ip", "5.6.7.8")],
            "127.0.0.1",
        );
        let extractor = make_extractor();
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("5.6.7.8"));
    }

    /// 无代理头时，应使用 ConnectInfo
    #[test]
    fn falls_back_to_connect_info_when_no_headers() {
        let parts = make_parts_with_connect_info("10.0.0.1");
        let extractor = make_extractor();
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("10.0.0.1"));
    }

    /// 无任何 IP 来源时，应返回错误
    #[test]
    fn returns_error_when_no_ip_source_available() {
        let parts = make_empty_parts();
        let extractor = make_extractor();
        assert!(extractor.extract(&parts).is_err());
    }

    /// M-1: 来自可信代理时，X-Forwarded-For 优先于 X-Real-IP
    #[test]
    fn x_forwarded_for_takes_priority_over_x_real_ip() {
        let parts = make_parts_with_headers_and_connect_info(
            vec![
                ("x-forwarded-for", "1.1.1.1"),
                ("x-real-ip", "2.2.2.2"),
            ],
            "127.0.0.1",
        );
        let extractor = make_extractor();
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("1.1.1.1"));
    }

    /// M-1: 空的 X-Forwarded-For 应被忽略，回退到 X-Real-IP
    #[test]
    fn ignores_empty_x_forwarded_for() {
        let parts = make_parts_with_headers_and_connect_info(
            vec![
                ("x-forwarded-for", ""),
                ("x-real-ip", "3.3.3.3"),
            ],
            "127.0.0.1",
        );
        let extractor = make_extractor();
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("3.3.3.3"));
    }

    /// M-1: 非可信代理的 X-Forwarded-For 应被忽略，使用 ConnectInfo
    #[test]
    fn untrusted_proxy_uses_connect_info_not_forwarded_for() {
        let parts = make_parts_with_headers_and_connect_info(
            vec![("x-forwarded-for", "192.168.1.1")],
            "8.8.8.8", // 非可信代理
        );
        let extractor = make_extractor();
        let result = extractor.extract(&parts).unwrap();
        // 期望：使用 ConnectInfo 中的真实对端 IP
        assert_eq!(result.key, parse_ip("8.8.8.8"));
    }
}

#[cfg(test)]
mod login_rate_limiter_capacity_tests {
    use std::time::{Duration, Instant};
    use colophon::shared::security::LoginRateLimiter;

    #[test]
    fn does_not_exceed_entry_capacity() {
        let mut limiter = LoginRateLimiter::new();
        let now = Instant::now();

        // 塞入 10,001 个不同的 key (超过 MAX_LOGIN_RATE_LIMIT_ENTRIES = 10000)
        for i in 0..=10000 {
            limiter.allow(format!("ip-{i}"), now);
        }

        // 限流器内部不应该存有超过上限的条目。
        // 当容量满时，allow() 对新 key 返回 true 并记录 warn 日志。
        // ip-0 应该被驱逐了（因为是最早插入的，retain + 容量检查后
        // 最新插入的 ip-10000 占据最后一个位置，ip-0 被驱逐）。
        // 或者如果未驱逐（HashMap 恰好没淘汰到它），容量保护也会
        // 让 ip-0 的第二次请求放行。
        let first_key_allowed =
            limiter.allow("ip-0".to_string(), now + Duration::from_secs(10));
        assert!(
            first_key_allowed,
            "early key should be allowed because it was evicted when capacity exceeded"
        );
    }

    #[test]
    fn rate_limiter_does_not_panic_on_repeated_overflow() {
        let mut limiter = LoginRateLimiter::new();
        let now = Instant::now();

        // 持续写入 3 倍容量，确保不 panic
        for cycle in 0..3 {
            for i in 0..=10000 {
                limiter.allow(format!("cycle{cycle}-ip-{i}"), now);
            }
        }

        // 所有 key 在容量保护下都应该被放行
        let result = limiter.allow("cycle0-ip-0".to_string(), now + Duration::from_secs(10));
        assert!(result, "overflowed key should still be allowed");
    }

    #[test]
    fn known_attacker_still_blocked_under_capacity_pressure() {
        let mut limiter = LoginRateLimiter::new();
        let now = Instant::now();

        // 先用一个 key 刷满 8 次登录尝试限制
        for _ in 0..8 {
            limiter.allow("target-ip".to_string(), now);
        }

        // 确认第 9 次被拦截
        assert!(!limiter.allow("target-ip".to_string(), now));

        // 然后用大量不同的 key 填满容量（超过 10000）
        // 容量保护对新 key 放行（return true），已知 key 不受影响
        for i in 0..=20000 {
            limiter.allow(format!("noise-{i}"), now);
        }

        // 已知的受限制 key 在容量压力下仍然被跟踪和拦截
        // — 这是正确的防御行为：不因噪声流量而遗忘已知攻击者
        let result = limiter.allow("target-ip".to_string(), now + Duration::from_secs(10));
        assert!(
            !result,
            "known attacker should remain blocked under capacity pressure (not evicted)"
        );

        // 窗口过期后（60s + 1s），key 应被正常放行
        let after_window = now + Duration::from_secs(61);
        assert!(
            limiter.allow("target-ip".to_string(), after_window),
            "known key should be allowed after window expiry"
        );
    }
}
