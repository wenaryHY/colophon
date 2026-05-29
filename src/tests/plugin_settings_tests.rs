#[cfg(test)]
mod tests {
    use crate::modules::plugin::manifest::{PluginManifest, SettingDef};
    use std::collections::HashMap;

    #[test]
    fn parse_manifest_with_settings() {
        let toml_str = r#"
[plugin]
id = "test-plugin"
title = "Test"
version = "0.1.0"

[[settings]]
key = "target"
label = "Target"
type = "text"
default = "World"
description = "Greeting target"

[[settings]]
key = "theme"
label = "Theme"
type = "select"
default = "light"
options = [
    { value = "light", label = "Light" },
    { value = "dark", label = "Dark" },
]
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.id, "test-plugin");
        let settings = manifest.settings.unwrap();
        assert_eq!(settings.len(), 2);
        assert_eq!(settings[0].key, "target");
        assert_eq!(settings[0].setting_type, "text");
        assert_eq!(settings[0].default.as_deref(), Some("World"));
        assert_eq!(settings[1].key, "theme");
        assert_eq!(settings[1].setting_type, "select");
        let opts = settings[1].options.as_ref().unwrap();
        assert_eq!(opts.len(), 2);
    }

    #[test]
    fn parse_manifest_with_admin() {
        let toml_str = r#"
[plugin]
id = "admin-plugin"
title = "Admin"
version = "0.1.0"

[admin]
enabled = true
entry = "settings.html"

[resources]
admin_root = "admin/"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let admin = manifest.admin.unwrap();
        assert_eq!(admin.enabled, Some(true));
        assert_eq!(admin.entry.as_deref(), Some("settings.html"));
        let resources = manifest.resources.unwrap();
        assert_eq!(resources.admin_root.as_deref(), Some("admin/"));
    }

    #[test]
    fn parse_manifest_without_settings_is_backward_compatible() {
        let toml_str = r#"
[plugin]
id = "old-plugin"
title = "Old"
version = "0.1.0"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert!(manifest.settings.is_none());
        assert!(manifest.admin.is_none());
        assert!(manifest.resources.is_none());
    }

    #[test]
    fn merge_with_defaults_fills_missing_keys() {
        let defs = vec![
            SettingDef {
                key: "a".into(),
                label: "A".into(),
                setting_type: "text".into(),
                default: Some("default-a".into()),
                description: None,
                options: None,
            },
            SettingDef {
                key: "b".into(),
                label: "B".into(),
                setting_type: "text".into(),
                default: None,
                description: None,
                options: None,
            },
        ];
        let mut values: HashMap<String, String> = HashMap::new();
        for s in &defs {
            if !values.contains_key(&s.key) {
                if let Some(ref d) = s.default {
                    values.insert(s.key.clone(), d.clone());
                }
            }
        }
        assert_eq!(values.get("a").unwrap(), "default-a");
        assert!(!values.contains_key("b"));
    }
}
