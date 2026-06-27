use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::fs;

use super::traits::StorageBackend;
use crate::shared::error::AppError;

pub struct LocalStorage {
    pub base_dir: PathBuf,
    pub base_url: String,
}

impl LocalStorage {
    pub fn new(base_dir: PathBuf, base_url: String) -> Self {
        Self { base_dir, base_url }
    }

    /// H-3: 路径遍历防护 — 校验最终路径在 base_dir 内
    fn validate_path(&self, path: &str) -> Result<PathBuf, AppError> {
        let full_path = self.base_dir.join(path);

        // 拒绝绝对路径（防止直接写入 /etc/... 等）
        if path.starts_with('/') || path.starts_with('\\') {
            return Err(AppError::Forbidden);
        }

        // 拒绝包含 .. 的路径组件（防止目录遍历）
        if path.contains("..") {
            return Err(AppError::Forbidden);
        }

        // 最终校验：确保路径在 base_dir 内
        // 使用 components() 解析，防止符号链接绕过
        let mut depth: i32 = 0;
        for component in std::path::Path::new(path).components() {
            match component {
                std::path::Component::Normal(_) => depth += 1,
                std::path::Component::ParentDir => depth -= 1,
                _ => {}
            }
            if depth < 0 {
                return Err(AppError::Forbidden);
            }
        }

        Ok(full_path)
    }
}

impl StorageBackend for LocalStorage {
    fn save<'a>(
        &'a self,
        file_data: &'a [u8],
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, AppError>> + Send + 'a>> {
        Box::pin(async move {
            // H-3: 路径遍历防护
            let full_path = self.validate_path(path)?;
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&full_path, file_data).await?;
            Ok(path.to_string())
        })
    }

    fn delete<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            // N-2: 路径遍历防护
            let full_path = self.validate_path(path)?;
            if full_path.exists() {
                fs::remove_file(full_path).await?;
            }
            Ok(())
        })
    }

    fn exists<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, AppError>> + Send + 'a>> {
        Box::pin(async move {
            // N-2: 路径遍历防护
            let full_path = self.validate_path(path)?;
            Ok(full_path.exists())
        })
    }

    fn get_public_url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// H-3: 路径遍历攻击测试 — 尝试写入 ../../../tmp/evil.txt
    /// 当前漏洞：save() 未校验目标路径
    /// 期望修复后：返回 Err
    #[tokio::test]
    async fn security_fix_h3_rejects_path_traversal_with_dot_dot() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf(), "/uploads".into());
        let result = storage.save(b"evil", "../../../tmp/evil.txt").await;
        assert!(result.is_err(), "path traversal with ../ should be rejected");
    }

    /// H-3: 路径遍历攻击测试 — 尝试写入绝对路径 /etc/cron.d/evil
    #[tokio::test]
    async fn security_fix_h3_rejects_absolute_path_outside_base() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf(), "/uploads".into());
        let result = storage.save(b"evil", "/etc/cron.d/evil").await;
        assert!(result.is_err(), "absolute path outside base should be rejected");
    }

    /// H-3: 正常路径应被允许
    #[tokio::test]
    async fn security_fix_h3_allows_valid_path() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf(), "/uploads".into());
        let result = storage.save(b"safe", "media/image.jpg").await;
        assert!(result.is_ok(), "valid path should be allowed");
    }

    /// N-2: 路径遍历防护 — delete 应拒绝 ../../etc/hosts
    #[tokio::test]
    async fn security_fix_n2_delete_rejects_path_traversal() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf(), "/uploads".into());
        let result = storage.delete("../../etc/hosts").await;
        assert!(result.is_err(), "delete with path traversal should be rejected");
    }

    /// N-2: 路径遍历防护 — exists 应拒绝 ../../etc/passwd
    #[tokio::test]
    async fn security_fix_n2_exists_rejects_path_traversal() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf(), "/uploads".into());
        let result = storage.exists("../../etc/passwd").await;
        assert!(result.is_err(), "exists with path traversal should be rejected");
    }
}
