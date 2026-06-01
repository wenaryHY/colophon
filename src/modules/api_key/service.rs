use sha2::{Digest, Sha256};

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
