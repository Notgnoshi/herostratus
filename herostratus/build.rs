use std::path::Path;
use std::{env, fmt, fs};

fn collect_files(dir: &Path, relative_to: &Path, out: &mut Vec<(String, String)>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            panic!("failed to read directory {dir:?}: {e}");
        }
    };
    for entry in entries {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, relative_to, out);
        } else {
            let rel = path
                .strip_prefix(relative_to)
                .expect("file should be under templates dir");
            // Use forward slashes for template names, even on Windows
            let name = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/");
            let absolute = path.canonicalize().expect("failed to canonicalize path");
            out.push((name, absolute.display().to_string()));
        }
    }
}

struct Asset {
    name: String,
    path: String,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(\"{}\", include_str!(\"{}\"))", self.name, self.path)
    }
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let templates_dir = Path::new(&manifest_dir).join("../templates");
    let templates_dir = templates_dir
        .canonicalize()
        .expect("templates directory should exist");

    println!("cargo::rerun-if-changed={}", templates_dir.display());

    let mut files = Vec::new();
    collect_files(&templates_dir, &templates_dir, &mut files);
    files.sort();

    let mut templates = Vec::new();
    let mut static_assets = Vec::new();
    for (name, path) in &files {
        let asset = Asset {
            name: name.clone(),
            path: path.clone(),
        };
        if name.ends_with(".html") {
            templates.push(asset);
        } else {
            static_assets.push(asset);
        }
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("embedded_assets.rs");

    let mut code = String::new();
    code.push_str("pub const TEMPLATES: &[(&str, &str)] = &[\n");
    for t in &templates {
        code.push_str(&format!("    {t},\n"));
    }
    code.push_str("];\n\n");

    code.push_str("pub const STATIC_ASSETS: &[(&str, &str)] = &[\n");
    for a in &static_assets {
        code.push_str(&format!("    {a},\n"));
    }
    code.push_str("];\n");

    fs::write(&out_path, code).expect("failed to write generated assets file");
}
