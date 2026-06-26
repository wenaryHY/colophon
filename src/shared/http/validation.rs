use crate::shared::error::{AppError, AppResult};

/// 验证字段非空，空则返回 BadRequest
pub fn require_non_empty<'a>(value: &'a str, field_name: &str) -> AppResult<&'a str> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!("{field_name} is required")));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_value_passes() {
        assert!(require_non_empty("hello", "name").is_ok());
    }

    #[test]
    fn empty_value_returns_bad_request() {
        let err = require_non_empty("", "name").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn whitespace_only_returns_bad_request() {
        let err = require_non_empty("   ", "name").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }
}
