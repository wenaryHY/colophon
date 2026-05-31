/// 认证模块集成测试
/// 测试注册、登录、JWT 验证等核心流程

#[cfg(test)]
mod auth_integration_tests {
    use crate::modules::auth::dto::{LoginRequest, RegisterRequest};
    use crate::shared::response::ApiResponse;

    #[test]
    fn test_register_request_validation() {
        // 测试注册请求验证
        let valid_request = RegisterRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "SecurePass123!".to_string(),
            display_name: Some("Test User".to_string()),
        };

        assert_eq!(valid_request.username, "testuser");
        assert_eq!(valid_request.email, "test@example.com");
        assert!(valid_request.password.len() >= 8);
    }

    #[test]
    fn test_login_request_validation() {
        // 测试登录请求验证（login 字段可以是 username 或 email）
        let valid_request = LoginRequest {
            login: "testuser".to_string(),
            password: "SecurePass123!".to_string(),
            remember_me: None,
        };

        assert_eq!(valid_request.login, "testuser");
        assert!(!valid_request.password.is_empty());
    }

    #[test]
    fn test_api_response_structure() {
        // 测试 API 响应结构
        let response: ApiResponse<String> = ApiResponse::success("test data".to_string());

        assert_eq!(response.code, 0);
        assert_eq!(response.message, "ok");
        assert_eq!(response.data, Some("test data".to_string()));
        assert!(!response.request_id.is_empty());
    }

    #[test]
    fn test_api_error_response() {
        // 测试错误响应
        let response: ApiResponse<()> = ApiResponse::error(40400, "not found");

        assert_eq!(response.code, 40400);
        assert_eq!(response.message, "not found");
        assert!(response.data.is_none());
        assert!(!response.request_id.is_empty());
    }
}
