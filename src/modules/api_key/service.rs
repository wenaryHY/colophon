use sha2::{Digest, Sha256};

// API Key 权限固定为 read_only，仅能访问需要 AuthUser 的公开内容 API。
// 管理操作（/api/v1/admin/*）需要 AdminUser (JWT session)，API Key 无法访问。

/// 生成新的 API Key
/// 返回：(完整明文key仅展示一次, key_prefix, key_hash)
pub fn generate_api_key_and_hash() -> (String, String, String) {
    let random_bytes: [u8; 32] = rand::random();
    let raw = format!("ink_{}", hex::encode(random_bytes));
    // 取前 12 个字符作为前缀，用于 UI 展示：ink_xxxxxxxx
    let prefix = raw[..12.min(raw.len())].to_string();
    let hash = hex::encode(Sha256::digest(raw.as_bytes()));
    (raw, prefix, hash)
}

/// 计算给定明文 key 的 SHA-256 hash
pub fn hash_api_key(plaintext: &str) -> String {
    hex::encode(Sha256::digest(plaintext.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_key_starts_with_ink_prefix() {
        let (raw, _, _) = generate_api_key_and_hash();
        assert!(raw.starts_with("ink_"));
    }

    #[test]
    fn generated_key_has_expected_length() {
        let (raw, _, _) = generate_api_key_and_hash();
        // "ink_" (4) + 64 hex chars = 68
        assert_eq!(raw.len(), 68);
    }

    #[test]
    fn prefix_is_first_twelve_chars() {
        let (raw, prefix, _) = generate_api_key_and_hash();
        assert_eq!(prefix, &raw[..12]);
    }

    #[test]
    fn hash_is_sha256_hex() {
        let (_, _, hash) = generate_api_key_and_hash();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_matches_manual_computation() {
        let (raw, _, hash) = generate_api_key_and_hash();
        assert_eq!(hash, hash_api_key(&raw));
    }

    #[test]
    fn hash_api_key_deterministic() {
        let h1 = hash_api_key("test_key");
        let h2 = hash_api_key("test_key");
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_keys_produce_different_hashes() {
        let h1 = hash_api_key("key_a");
        let h2 = hash_api_key("key_b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn two_generated_keys_are_unique() {
        let (raw1, _, _) = generate_api_key_and_hash();
        let (raw2, _, _) = generate_api_key_and_hash();
        assert_ne!(raw1, raw2);
    }
}
