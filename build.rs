use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn main() {
    println!("cargo:rerun-if-changed=themes");

    let manifest = generate_asset_manifest();
    let manifest_json =
        serde_json::to_string_pretty(&manifest).expect("Failed to serialize asset manifest");

    let out_dir = PathBuf::from("target/generated");
    fs::create_dir_all(&out_dir).expect("Failed to create target/generated directory");

    let manifest_path = out_dir.join("asset-manifest.json");
    let mut file = fs::File::create(&manifest_path).expect("Failed to create asset-manifest.json");
    file.write_all(manifest_json.as_bytes())
        .expect("Failed to write asset-manifest.json");

    println!(
        "cargo:warning=Generated asset manifest: {} entries",
        manifest.len()
    );
}

fn generate_asset_manifest() -> HashMap<String, String> {
    let mut manifest = HashMap::new();
    let themes_dir = Path::new("themes");

    // 如果 themes 目录不存在，返回空 manifest
    if !themes_dir.exists() {
        println!("cargo:warning=themes directory not found, skipping asset manifest generation");
        return manifest;
    }

    // 遍历所有主题的 static 目录
    for theme_entry in fs::read_dir(themes_dir).into_iter().flatten() {
        let theme_entry = match theme_entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let theme_name = theme_entry.file_name();
        let theme_name_str = theme_name.to_string_lossy();
        let static_dir = theme_entry.path().join("static");

        if !static_dir.exists() || !static_dir.is_dir() {
            continue;
        }

        // 遍历 static 目录下的所有文件
        for entry in WalkDir::new(&static_dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let file_path = entry.path();

            // 读取文件内容并计算 hash
            let content = match fs::read(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut hasher = Sha256::new();
            hasher.update(&content);
            let hash = hasher.finalize();
            let hash_short = format!("{:x}", hash)[..8].to_string();

            // 获取相对于 static 目录的路径
            let relative_path = match file_path.strip_prefix(&static_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Windows 路径分隔符统一转为 /
            let relative_path_str = relative_path
                .to_string_lossy()
                .replace('\\', "/")
                .to_string();

            // manifest key: "theme_name/relative/path.ext"
            let key = format!("{}/{}", theme_name_str, relative_path_str);

            // manifest value: "relative/path.ext?v=hash"
            let value = format!("{}?v={}", relative_path_str, hash_short);

            manifest.insert(key, value);
        }
    }

    manifest
}
