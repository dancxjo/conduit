use std::{fs, path::Path};

const ROOTS: &[&str] = &[
    ".github",
    "architecture",
    "bodies",
    "docs",
    "fabrication",
    "forms",
    "mechanisms",
    "products",
    "proof",
    "semantics",
    "targets",
    "tools",
];

const ROOT_FILES: &[&str] = &[
    "AGENTS.md",
    "CONTRIBUTING.md",
    "README.md",
    "STATUS.md",
    "Cargo.toml",
];

const TEXT_EXTENSIONS: &[&str] = &[
    "c", "conduit", "conf", "h", "html", "js", "json", "md", "mjs", "rs", "sh", "toml", "ts",
    "yml", "yaml",
];

#[test]
fn obsolete_host_boundary_vocabulary_is_absent() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");
    let forbidden = [
        ["host", "operation"].concat(),
        ["host", "_operation"].concat(),
        ["host", "-operation"].concat(),
        ["host", " operation"].concat(),
    ];
    let mut violations = Vec::new();
    for vocabulary in &forbidden {
        for root in ROOTS {
            inspect_directory(
                repository,
                &repository.join(root),
                vocabulary,
                &mut violations,
            );
        }
        for file in ROOT_FILES {
            inspect_file(
                repository,
                &repository.join(file),
                vocabulary,
                &mut violations,
            );
        }
    }
    violations.sort();
    violations.dedup();
    assert!(
        violations.is_empty(),
        "obsolete Host Call predecessor vocabulary remains:\n{}",
        violations.join("\n")
    );
}

#[test]
fn ordinary_interface_vocabulary_is_not_rewritten_as_front() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");
    let forbidden = ["inter", "front"].concat();
    let mut violations = Vec::new();
    for root in ROOTS {
        inspect_directory(
            repository,
            &repository.join(root),
            &forbidden,
            &mut violations,
        );
    }
    for file in ROOT_FILES {
        inspect_file(
            repository,
            &repository.join(file),
            &forbidden,
            &mut violations,
        );
    }
    assert!(
        violations.is_empty(),
        "ordinary interface vocabulary was rewritten by substring substitution:\n{}",
        violations.join("\n")
    );

    let create_protocol =
        fs::read_to_string(repository.join("mechanisms/devices/create-oi/src/device.rs"))
            .expect("Create Open Interface source remains readable");
    assert!(create_protocol.contains("Create Open Interface"));

    let form = fs::read_to_string(repository.join("architecture/form/src/checked_syntax.rs"))
        .expect("Form facade remains readable");
    assert!(form.contains("Front"));
}

fn inspect_directory(
    repository: &Path,
    directory: &Path,
    forbidden: &str,
    violations: &mut Vec<String>,
) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("read {} entry: {error}", directory.display()));
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            inspect_directory(repository, &path, forbidden, violations);
            continue;
        }
        let relative = path
            .strip_prefix(repository)
            .expect("source beneath repository");
        if relative
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains(forbidden)
        {
            violations.push(relative.display().to_string());
        }
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        if !TEXT_EXTENSIONS.contains(&extension) {
            continue;
        }
        inspect_file(repository, &path, forbidden, violations);
    }
}

fn inspect_file(repository: &Path, path: &Path, forbidden: &str, violations: &mut Vec<String>) {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    for (line_index, line) in source.lines().enumerate() {
        if line.to_ascii_lowercase().contains(forbidden) {
            let relative = path
                .strip_prefix(repository)
                .expect("source beneath repository");
            violations.push(format!("{}:{}", relative.display(), line_index + 1));
        }
    }
}
