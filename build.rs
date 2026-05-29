use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("plugin_registry.rs");

    let plugins_dir = Path::new("plugins");
    let mut registrations = String::new();

    if plugins_dir.exists() {
        if let Ok(dir) = fs::read_dir(plugins_dir) {
            for entry in dir {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
                let manifest_path = path.join("plugin.toml");
                let lib_path = path.join("lib.rs");

                if !manifest_path.exists() || !lib_path.exists() {
                    continue;
                }

                let content = match fs::read_to_string(&manifest_path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let id = content
                    .lines()
                    .find(|l| l.trim().starts_with("id"))
                    .map(|l| {
                        l.trim_start_matches("id")
                            .trim_start_matches(|c: char| c == '=' || c.is_whitespace())
                            .trim_matches(|c: char| c == '"' || c == '\'')
                            .to_string()
                    })
                    .unwrap_or_default();

                if id != dir_name {
                    println!(
                        "cargo:warning=plugin id '{}' != directory '{}', skipping",
                        id, dir_name
                    );
                    continue;
                }

                // #[path] 属性相对路径解析自生成文件所在目录（OUT_DIR），
                // 因此必须使用绝对路径或相对于 CARGO_MANIFEST_DIR 的路径。
                let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
                let include_path = Path::new(&manifest_dir)
                    .join(&lib_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let mod_name = format!("_plugin_{}", dir_name.replace('-', "_").replace('.', "_"));

                println!("cargo:warning=registering plugin: {id}", id = id);

                registrations.push_str(&format!(
                    r#"#[path = r"{path}"]
mod {mod_name};
crate::modules::plugin::registry::register(Box::new({mod_name}::HelloWorldPlugin::new())).await;
"#,
                    path = include_path,
                    mod_name = mod_name,
                ));
            }
        }
    }

    let code = format!(
        r#"pub async fn register_all() {{
{}
}}
"#,
        registrations
    );

    fs::write(&dest, code).unwrap();
    println!("cargo:rerun-if-changed=plugins/");
    println!("cargo:rerun-if-changed=build.rs");
}
