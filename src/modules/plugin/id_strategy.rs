use base64::Engine as _;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait PluginIdStrategy: Send + Sync {
    fn generate(name: &str) -> String;
    fn validate(id: &str) -> bool;
}

pub struct ShortHashIdStrategy;

impl PluginIdStrategy for ShortHashIdStrategy {
    fn generate(name: &str) -> String {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let hash = Sha256::digest(format!("{}-{}", name, ts));
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash);
        // 过滤 base64 特殊字符（- 和 _），取 8 个纯字母数字字符
        // 避免生成末尾带 - 的 ID（validate 拒绝尾随 -）
        let clean_suffix: String = encoded
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(8)
            .collect();
        let safe_name = name.to_lowercase().replace(' ', "-");
        format!("{}-{}", safe_name, clean_suffix)
    }

    fn validate(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !id.starts_with('-')
            && !id.ends_with('-')
    }
}
