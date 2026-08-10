use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, output);
        } else if let Ok(relative) = path.strip_prefix(root) {
            output.push((relative.to_string_lossy().replace('\\', "/"), path));
        }
    }
}

fn generate_player_assets() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let root = manifest.join("../../player-web/dist");
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut generated = String::from("pub fn embedded_player_asset(path: &str) -> Option<(&'static [u8], &'static str)> {\n    match path {\n");
    for (relative, absolute) in files {
        generated.push_str(&format!(
            "        {:?} => Some((include_bytes!({:?}), {:?})),\n",
            relative,
            absolute,
            content_type(&absolute)
        ));
    }
    generated.push_str("        _ => None,\n    }\n}\n");
    let out = PathBuf::from(env::var("OUT_DIR").expect("output directory"));
    fs::write(out.join("player_assets.rs"), generated).expect("write embedded player assets");
    println!("cargo:rerun-if-changed={}", root.display());
}

fn main() {
    generate_player_assets();
    tauri_build::build();
}
