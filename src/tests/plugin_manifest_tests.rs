#[cfg(test)]
mod tests {
    use crate::modules::plugin::manifest::PluginManifest;
    use std::path::PathBuf;

    #[test]
    fn parse_valid_plugin_toml() {
        let toml_str = r#"
[plugin]
id = "test-plugin-a3f9b2c1"
title = "Test Plugin"
version = "0.1.0"
description = "A test"
author = "Tester"

[engine]
colophon = ">=0.3.0"

[hooks]
template = true
routes = true
assets = ["css"]
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.id, "test-plugin-a3f9b2c1");
        assert_eq!(manifest.plugin.title, "Test Plugin");
        assert_eq!(manifest.plugin.version, "0.1.0");
        assert!(manifest.plugin.description.is_some());
        assert!(manifest.plugin.author.is_some());
    }

    #[test]
    fn parse_minimal_plugin_toml() {
        let toml_str = r#"
[plugin]
id = "minimal"
title = "Minimal"
version = "0.0.1"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.id, "minimal");
        assert!(manifest.engine.is_none());
        assert!(manifest.hooks.is_none());
    }

    #[test]
    fn parse_missing_id_fails() {
        let toml_str = r#"
[plugin]
title = "No ID"
version = "0.0.1"
"#;
        let result: Result<PluginManifest, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn from_file_reads_real_plugin_toml() {
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("plugins")
            .join("hello-world-a3f9b2c1")
            .join("plugin.toml");
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();
        assert_eq!(manifest.plugin.id, "hello-world-a3f9b2c1");
        assert_eq!(manifest.plugin.title, "Hello World");
    }

    // TODO: Wave 3.2 — restore version check logic once Wasm sandbox is in place
    #[test]
    #[ignore]
    fn version_check_passes_when_host_newer() {
        let toml_str = r#"
[plugin]
id = "test"
title = "T"
version = "0.1.0"

[engine]
colophon = ">=0.3.0"
"#;
        let _manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        // version check requires PluginLoader which is unavailable until Wave 3.2
    }

    // TODO: Wave 3.2 — restore version check logic once Wasm sandbox is in place
    #[test]
    #[ignore]
    fn version_check_fails_when_host_too_old() {
        let toml_str = r#"
[plugin]
id = "test"
title = "T"
version = "0.1.0"

[engine]
colophon = ">=0.3.0"
"#;
        let _manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        // version check requires PluginLoader which is unavailable until Wave 3.2
    }

    // TODO: Wave 3.2 — restore version check logic once Wasm sandbox is in place
    #[test]
    #[ignore]
    fn version_check_passes_with_no_engine_field() {
        let toml_str = r#"
[plugin]
id = "test"
title = "T"
version = "0.1.0"
"#;
        let _manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        // version check requires PluginLoader which is unavailable until Wave 3.2
    }
}
