//! 角色与权限的零成本抽象。
//!
//! ## 设计原则
//! - 编译期类型安全：Role 是枚举，权限检查是方法调用，无字符串匹配
//! - 零开销：Copy + 编译器 niche 优化，Option<Role> 大小为 1 字节
//! - 无 dyn dispatch：所有方法 `#[inline(always)]`，编译期单态化
//! - 为 ABAC 预留插槽：`can_access_admin()` 作为 fallback，未来可扩展 Policy 层

use std::str::FromStr;

use crate::shared::error::AppError;

/// 用户角色。
///
/// ## 变体
/// - `Admin`: 系统管理员，拥有最高权限
/// - `Member`: 注册用户，可以发布和管理自己的内容
///
/// ## 编译器优化
/// 不标注 `#[repr(u8)]`，让编译器自行选择最优布局。
/// 当前两个变体（均为 field-less），编译器会做 niche 优化：
/// `Option<Role>` 仅占 1 字节 —— Admin=0, Member=1, None=2。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// 系统管理员
    Admin,
    /// 注册用户
    Member,
}

impl Role {
    /// 判断此角色是否允许访问管理后台。
    ///
    /// ## 返回值
    /// - `true`: Admin 角色
    /// - `false`: 其他角色（Member 等）
    ///
    /// ## ABAC 扩展点
    /// 未来引入基于属性的访问控制时，此方法作为 fallback：
    /// 先查 ABAC Policy，若无匹配策略则调用此方法。
    #[inline(always)]
    pub fn can_access_admin(self) -> bool {
        matches!(self, Role::Admin)
    }

    /// 判断此角色是否可以发布内容。
    ///
    /// 当前 Admin 和 Member 均可发布，预留未来可能的"只读"角色。
    #[inline(always)]
    pub fn can_publish(self) -> bool {
        matches!(self, Role::Admin | Role::Member)
    }

    /// 返回角色的数据库存储字符串。
    ///
    /// 用于写入 SQLite、JWT 序列化。
    /// 返回值为 `'static str`，零分配。
    #[inline(always)]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Member => "member",
        }
    }
}

impl FromStr for Role {
    type Err = AppError;

    /// 从字符串解析角色。
    ///
    /// 用于从数据库读回或 API 反序列化。
    ///
    /// ## 支持的输入
    /// - `"admin"` → `Role::Admin`
    /// - `"member"` → `Role::Member`
    ///
    /// ## 错误
    /// 不匹配任何已知值时返回 `AppError::BadRequest`。
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Role::Admin),
            "member" => Ok(Role::Member),
            "read_only" => {
                // API Key 认证的用户映射为 Member：read_only 权限等价于 Member
                Ok(Role::Member)
            }
            other => Err(AppError::BadRequest(format!(
                "invalid role value encountered during parsing: '{other}'"
            ))),
        }
    }
}

impl std::fmt::Display for Role {
    /// 与 `as_db_str()` 一致，用于日志/调试输出。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl serde::Serialize for Role {
    /// 序列化为数据库存储字符串（"admin" / "member"）。
    /// 用于 JWT Claims 和 API 响应 JSON。
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_db_str())
    }
}

impl<'de> serde::Deserialize<'de> for Role {
    /// 从字符串反序列化。
    ///
    /// 支持 JSON string 字段。遇到未知值返回反序列化错误，
    /// 而非 panic——保证数据来自不可信源时的安全性。
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_can_access_admin() {
        assert!(Role::Admin.can_access_admin());
    }

    #[test]
    fn member_cannot_access_admin() {
        assert!(!Role::Member.can_access_admin());
    }

    #[test]
    fn admin_can_publish() {
        assert!(Role::Admin.can_publish());
    }

    #[test]
    fn member_can_publish() {
        assert!(Role::Member.can_publish());
    }

    #[test]
    fn as_db_str_returns_correct_values() {
        assert_eq!(Role::Admin.as_db_str(), "admin");
        assert_eq!(Role::Member.as_db_str(), "member");
    }

    #[test]
    fn from_str_valid_inputs() {
        assert_eq!("admin".parse::<Role>().unwrap(), Role::Admin);
        assert_eq!("member".parse::<Role>().unwrap(), Role::Member);
        assert_eq!("read_only".parse::<Role>().unwrap(), Role::Member);
    }

    #[test]
    fn from_str_invalid_input() {
        assert!("superadmin".parse::<Role>().is_err());
        assert!("".parse::<Role>().is_err());
    }

    #[test]
    fn display_matches_as_db_str() {
        assert_eq!(Role::Admin.to_string(), "admin");
        assert_eq!(Role::Member.to_string(), "member");
    }

    #[test]
    fn serde_roundtrip() {
        let json = serde_json::to_string(&Role::Admin).unwrap();
        assert_eq!(json, "\"admin\"");
        let parsed: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Role::Admin);

        let json = serde_json::to_string(&Role::Member).unwrap();
        assert_eq!(json, "\"member\"");
        let parsed: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Role::Member);
    }

    #[test]
    fn serde_rejects_unknown_variant() {
        assert!(serde_json::from_str::<Role>("\"superadmin\"").is_err());
    }

    #[test]
    fn option_role_is_copy_and_small() {
        // 验证 Copy trait：赋值后原值仍可用
        let role = Role::Admin;
        let copy = role;
        assert_eq!(role, copy);
        // Option<Role> 应仅占 1 字节（niche 优化）
        assert_eq!(size_of::<Option<Role>>(), 1);
    }
}
