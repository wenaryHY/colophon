#[cfg(test)]
mod tests {
    use crate::modules::plugin::id_strategy::{PluginIdStrategy, ShortHashIdStrategy};

    #[test]
    fn generated_id_is_valid() {
        let id = ShortHashIdStrategy::generate("hello-world");
        assert!(
            ShortHashIdStrategy::validate(&id),
            "generated id '{}' should pass validation",
            id
        );
    }

    #[test]
    fn generated_id_starts_with_plugin_name() {
        let id = ShortHashIdStrategy::generate("my-plugin");
        assert!(
            id.starts_with("my-plugin-"),
            "generated id '{}' should start with plugin name",
            id
        );
    }

    #[test]
    fn generated_id_has_hash_suffix() {
        let id = ShortHashIdStrategy::generate("test");
        let parts: Vec<&str> = id.split('-').collect();
        assert!(parts.len() >= 2, "id should have name-hash format");
    }

    #[test]
    fn same_name_different_time_generates_different_ids() {
        let id1 = ShortHashIdStrategy::generate("unique");
        std::thread::sleep(std::time::Duration::from_millis(1));
        let id2 = ShortHashIdStrategy::generate("unique");
        assert_ne!(id1, id2, "sequential IDs should differ");
    }

    #[test]
    fn validate_rejects_empty() {
        assert!(!ShortHashIdStrategy::validate(""));
    }

    #[test]
    fn validate_rejects_leading_dash() {
        assert!(!ShortHashIdStrategy::validate("-my-plugin"));
    }

    #[test]
    fn validate_rejects_trailing_dash() {
        assert!(!ShortHashIdStrategy::validate("my-plugin-"));
    }

    #[test]
    fn validate_rejects_too_long() {
        let long = "a".repeat(65);
        assert!(!ShortHashIdStrategy::validate(&long));
    }

    #[test]
    fn validate_rejects_special_chars() {
        assert!(!ShortHashIdStrategy::validate("my plugin"));
        assert!(!ShortHashIdStrategy::validate("my_plugin"));
        assert!(!ShortHashIdStrategy::validate("my.plugin"));
    }
}
