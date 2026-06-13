use crate::shared::error::{AppError, AppResult};

pub fn normalize_site_url(value: &str) -> AppResult<String> {
    let url = parse_http_url(value, "site_url must be a valid absolute http/https URL")?;
    ensure_root_path(&url)?;
    Ok(url.origin().unicode_serialization())
}

pub fn normalize_admin_url(value: &str) -> AppResult<String> {
    let url = parse_http_url(value, "admin_url must be a valid absolute http/https URL")?;
    ensure_admin_path(&url)?;
    Ok(format!("{}/admin", url.origin().unicode_serialization()))
}

pub fn canonical_admin_url_from_site_url(site_url: &str) -> AppResult<String> {
    let url = parse_http_url(site_url, "site_url must be a valid absolute http/https URL")?;
    ensure_root_path(&url)?;
    Ok(format!("{}/admin", url.origin().unicode_serialization()))
}

pub fn normalize_bool_string(value: &str, field: &str) -> AppResult<String> {
    match value.trim() {
        "true" => Ok("true".to_string()),
        "false" => Ok("false".to_string()),
        _ => Err(AppError::BadRequest(format!(
            "{field} must be true or false"
        ))),
    }
}

fn parse_http_url(value: &str, message: &str) -> AppResult<url::Url> {
    let trimmed = value.trim();
    let url = url::Url::parse(trimmed).map_err(|_| AppError::BadRequest(message.into()))?;
    ensure_supported_scheme(&url)?;
    ensure_no_auth_query_fragment(&url)?;
    Ok(url)
}

fn ensure_supported_scheme(url: &url::Url) -> AppResult<()> {
    if matches!(url.scheme(), "http" | "https") {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "URL scheme must be http or https".into(),
    ))
}

fn ensure_no_auth_query_fragment(url: &url::Url) -> AppResult<()> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::BadRequest(
            "URL must not contain username or password".into(),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(AppError::BadRequest(
            "URL must not contain query or fragment".into(),
        ));
    }
    Ok(())
}

fn ensure_root_path(url: &url::Url) -> AppResult<()> {
    if url.path() == "/" {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "site_url must not contain any path".into(),
    ))
}

fn ensure_admin_path(url: &url::Url) -> AppResult<()> {
    let path = url.path().trim_end_matches('/');
    if path == "/admin" {
        return Ok(());
    }
    Err(AppError::BadRequest("admin_url path must be /admin".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_site_url_strips_trailing_slash() {
        let result = normalize_site_url("https://example.com/").unwrap();
        assert_eq!(result, "https://example.com");
    }

    #[test]
    fn normalize_site_url_accepts_plain_origin() {
        let result = normalize_site_url("https://example.com").unwrap();
        assert_eq!(result, "https://example.com");
    }

    #[test]
    fn normalize_site_url_rejects_path() {
        assert!(normalize_site_url("https://example.com/blog").is_err());
    }

    #[test]
    fn normalize_site_url_rejects_query() {
        assert!(normalize_site_url("https://example.com/?x=1").is_err());
    }

    #[test]
    fn normalize_site_url_rejects_fragment() {
        assert!(normalize_site_url("https://example.com/#top").is_err());
    }

    #[test]
    fn normalize_site_url_rejects_ftp_scheme() {
        assert!(normalize_site_url("ftp://example.com").is_err());
    }

    #[test]
    fn normalize_site_url_rejects_credentials() {
        assert!(normalize_site_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn normalize_admin_url_accepts_valid() {
        let result = normalize_admin_url("https://example.com/admin").unwrap();
        assert_eq!(result, "https://example.com/admin");
    }

    #[test]
    fn normalize_admin_url_strips_trailing_slash() {
        let result = normalize_admin_url("https://example.com/admin/").unwrap();
        assert_eq!(result, "https://example.com/admin");
    }

    #[test]
    fn normalize_admin_url_rejects_wrong_path() {
        assert!(normalize_admin_url("https://example.com/dashboard").is_err());
    }

    #[test]
    fn canonical_admin_url_from_site_url_appends_admin() {
        let result = canonical_admin_url_from_site_url("https://example.com").unwrap();
        assert_eq!(result, "https://example.com/admin");
    }

    #[test]
    fn normalize_bool_string_accepts_true() {
        assert_eq!(normalize_bool_string("true", "field").unwrap(), "true");
    }

    #[test]
    fn normalize_bool_string_accepts_false() {
        assert_eq!(normalize_bool_string("false", "field").unwrap(), "false");
    }

    #[test]
    fn normalize_bool_string_trims_whitespace() {
        assert_eq!(normalize_bool_string("  true  ", "field").unwrap(), "true");
    }

    #[test]
    fn normalize_bool_string_rejects_invalid() {
        assert!(normalize_bool_string("yes", "field").is_err());
        assert!(normalize_bool_string("1", "field").is_err());
    }

    #[test]
    fn normalize_site_url_http_scheme_accepted() {
        let result = normalize_site_url("http://localhost:3000").unwrap();
        assert_eq!(result, "http://localhost:3000");
    }
}
