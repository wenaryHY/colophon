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
        let suffix = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash[..6]);
        let safe_name = name.to_lowercase().replace(' ', "-");
        format!("{}-{}", safe_name, &suffix[..8])
    }

    fn validate(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !id.starts_with('-')
            && !id.ends_with('-')
    }
}
