//! Compile the admitted package registry into exact embedded byte sources.
use serde::Deserialize;
use std::{collections::BTreeSet, env, fs, path::PathBuf};

#[derive(Deserialize)]
struct Registry {
    resources: Vec<Resource>,
}
#[derive(Deserialize)]
struct Resource {
    role: String,
    kind: Kind,
    path: String,
    source: String,
    maximum_bytes: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    Module,
    ClassicScript,
    Style,
    Wasm,
}

fn main() {
    let package = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = package.join("../../..").canonicalize().unwrap();
    let template = package.join("assets/patchbay.application.template.json");
    println!("cargo:rerun-if-changed={}", template.display());
    let registry: Registry = serde_json::from_slice(&fs::read(template).unwrap()).unwrap();
    let mut paths = BTreeSet::new();
    let mut roles = BTreeSet::new();
    let mut generated = String::from("pub(crate) const RESOURCES: &[Resource] = &[\n");
    for r in registry.resources {
        assert!(
            paths.insert(r.path.clone()) && roles.insert(r.role.clone()),
            "duplicate resource"
        );
        assert!(
            r.path.starts_with("assets/") && !r.path.contains(".."),
            "invalid resource path"
        );
        assert!(r.maximum_bytes > 0);
        let media = match r.kind {
            Kind::Module | Kind::ClassicScript => "text/javascript; charset=utf-8",
            Kind::Style => "text/css; charset=utf-8",
            Kind::Wasm => "application/wasm",
        };
        let source = match r.source.as_str() {
            "generated:theme" => {
                assert!(matches!(r.kind, Kind::Style));
                "Source::Theme".into()
            }
            "supplied:runtime" => {
                assert!(matches!(r.kind, Kind::Wasm));
                "Source::Runtime".into()
            }
            path => {
                let source = root
                    .join(path)
                    .canonicalize()
                    .expect("registry source must exist");
                assert!(source.starts_with(&root), "source escapes repository");
                let bytes = fs::read(&source).unwrap();
                assert!(
                    !bytes.is_empty() && bytes.len() <= r.maximum_bytes,
                    "resource exceeds bound: {}",
                    r.role
                );
                println!("cargo:rerun-if-changed={}", source.display());
                format!("Source::Embedded(include_bytes!({:?}))", source)
            }
        };
        generated.push_str(&format!(
            "Resource {{ path: {:?}, media_type: {:?}, source: {} }},\n",
            r.path, media, source
        ));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("resources.rs"),
        generated,
    )
    .unwrap();
}
