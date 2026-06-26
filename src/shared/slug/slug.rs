use slug::slugify;

pub fn generate_slug(name: &str, custom: Option<&str>) -> String {
    custom
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| slugify(name))
}
// bench
