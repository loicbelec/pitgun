use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let data_dir = manifest_dir.join("../pitgun-simulator/data");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    let destination = out_dir.join("embedded_files.rs");

    println!("cargo:rerun-if-changed={}", data_dir.display());

    let mut files = Vec::new();
    collect_json_files(&data_dir, &data_dir, &mut files);
    files.sort();

    let mut source = String::from("const EMBEDDED_FILES: &[(&str, &str)] = &[\n");
    for relative_path in files {
        if relative_path.starts_with("profiles/") {
            continue;
        }
        source.push_str(&format!(
            "    ({relative_path:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../pitgun-simulator/data/{relative_path}\"))),\n"
        ));
    }
    source.push_str("];\n");

    fs::write(destination, source).expect("write embedded file index");
}

fn collect_json_files(root: &Path, current: &Path, output: &mut Vec<String>) {
    let entries = fs::read_dir(current).expect("read data dir");
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(root, &path, output);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .expect("strip data dir prefix")
            .to_string_lossy()
            .replace('\\', "/");
        output.push(relative);
    }
}
