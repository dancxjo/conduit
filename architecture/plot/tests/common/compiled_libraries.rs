//! Bind emitted-code proof to this test's exact Cargo dependency graph.
//! Modification time cannot distinguish host/build/target feature variants.
use std::{fs, path::Path};
pub fn resolve(dependencies: &Path, names: &[&str]) -> Vec<String> {
    let executable = std::env::current_exe().unwrap();
    let stem = executable.file_stem().unwrap().to_str().unwrap();
    let hash = stem.rsplit_once('-').expect("Cargo test artifact hash").1;
    let root = dependencies.parent().unwrap().join(".fingerprint");
    let owner = root.join(format!("{}-{hash}", env!("CARGO_PKG_NAME")));
    let manifests: Vec<_> = fs::read_dir(&owner)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|f| {
            f.file_name().to_string_lossy().starts_with("test-")
                && f.path().extension().is_some_and(|e| e == "json")
        })
        .collect();
    assert_eq!(manifests.len(), 1, "one exact Cargo test fingerprint");
    let manifest = fs::read_to_string(manifests[0].path()).unwrap();
    names
        .iter()
        .map(|name| {
            let marker = format!("\"{name}\",");
            assert_eq!(
                manifest.matches(&marker).count(),
                1,
                "exact dependency {name}"
            );
            let tail = manifest.split_once(&marker).unwrap().1;
            let (_, tail) = tail.split_once(',').expect("Cargo dependency target flag");
            let value = tail
                .trim_start()
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .unwrap()
                .parse::<u64>()
                .expect("Cargo dependency fingerprint");
            let expected = value
                .to_le_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let prefix = format!("{}-", name.replace('_', "-"));
            let mut found = Vec::new();
            for entry in fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            {
                let identity = entry.path().join(format!("lib-{name}"));
                if fs::read_to_string(identity).is_ok_and(|s| s.trim() == expected) {
                    let folder = entry.file_name();
                    let folder = folder.to_str().unwrap();
                    let hash = folder.rsplit_once('-').unwrap().1;
                    let library = dependencies.join(format!("lib{name}-{hash}.rlib"));
                    if library.is_file() {
                        found.push(library);
                    }
                }
            }
            assert_eq!(found.len(), 1, "one built rlib for exact dependency {name}");
            format!("{name}={}", found[0].display())
        })
        .collect()
}
