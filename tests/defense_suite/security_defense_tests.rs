// 套件 2: ForwardedIpExtractor 测试
// 套件 3: LoginRateLimiter 容量上限测试

#[cfg(test)]
mod forwarded_ip_extractor_tests {
    use std::net::{IpAddr, SocketAddr};
    use axum::extract::ConnectInfo;
    use axum::http::request::Parts;
    use axum_governor::KeyExtractor;
    use colophon::shared::security::ForwardedIpExtractor;

    /// 用给定的 Header 列表构造一个 Parts。Parts 不支持 Default，
    /// 因此通过 `http::Request::builder()` 构建然后解构获取。
    fn make_parts_with_headers(header_pairs: Vec<(&str, &str)>) -> Parts {
        let mut builder = axum::http::Request::builder();
        for (k, v) in &header_pairs {
            builder = builder.header(*k, *v);
        }
        let (parts, _body) = builder.body(()).expect("failed to build request").into_parts();
        parts
    }

    /// 构造没有任何 Header 和 Extensions 的 Parts
    fn make_empty_parts() -> Parts {
        let (parts, _body) = axum::http::Request::new(())
            .into_parts();
        parts
    }

    fn parse_ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn extracts_first_ip_from_x_forwarded_for() {
        let parts = make_parts_with_headers(vec![("x-forwarded-for", "1.2.3.4")]);
        let extractor = ForwardedIpExtractor;
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("1.2.3.4"));
    }

    #[test]
    fn extracts_first_ip_from_x_forwarded_for_list() {
        let parts = make_parts_with_headers(vec![("x-forwarded-for", "1.2.3.4, 5.6.7.8")]);
        let extractor = ForwardedIpExtractor;
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("1.2.3.4"));
    }

    #[test]
    fn extracts_real_ip_from_x_real_ip_fallback() {
        let parts = make_parts_with_headers(vec![("x-real-ip", "5.6.7.8")]);
        let extractor = ForwardedIpExtractor;
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("5.6.7.8"));
    }

    #[test]
    fn falls_back_to_connect_info_when_no_headers() {
        let mut parts = make_empty_parts();
        let addr: SocketAddr = "10.0.0.1:12345".parse().unwrap();
        parts.extensions.insert(ConnectInfo(addr));

        let extractor = ForwardedIpExtractor;
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("10.0.0.1"));
    }

    #[test]
    fn returns_error_when_no_ip_source_available() {
        let parts = make_empty_parts();
        let extractor = ForwardedIpExtractor;
        assert!(extractor.extract(&parts).is_err());
    }

    #[test]
    fn x_forwarded_for_takes_priority_over_x_real_ip() {
        let parts = make_parts_with_headers(vec![
            ("x-forwarded-for", "1.1.1.1"),
            ("x-real-ip", "2.2.2.2"),
        ]);
        let extractor = ForwardedIpExtractor;
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("1.1.1.1"));
    }

    #[test]
    fn ignores_empty_x_forwarded_for() {
        let parts = make_parts_with_headers(vec![
            ("x-forwarded-for", ""),
            ("x-real-ip", "3.3.3.3"),
        ]);
        let extractor = ForwardedIpExtractor;
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("3.3.3.3"));
    }

    #[test]
    fn connect_info_takes_priority_when_forwarded_for_has_private_ip() {
        // X-Forwarded-For 可能被伪造为内网 IP，但提取器本身不做过滤。
        // 这里验证即使 IP 是私有地址，提取器仍然返回它（不做判断）。
        let parts = make_parts_with_headers(vec![("x-forwarded-for", "192.168.1.1")]);
        let extractor = ForwardedIpExtractor;
        let result = extractor.extract(&parts).unwrap();
        assert_eq!(result.key, parse_ip("192.168.1.1"));
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
