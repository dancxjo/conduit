use std::{fs, path::Path};

#[test]
fn production_conduit_sources_use_the_frozen_surface() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask lives under the repository tools directory");
    let mut sources = Vec::new();
    for root in ["forms", "bodies", "products", "targets"] {
        collect_conduit_sources(&repository.join(root), &mut sources);
    }
    sources.sort();
    assert!(!sources.is_empty(), "production source inventory is empty");

    let mut failures = Vec::new();
    for path in sources {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let document = conduit_form::parse_syntax_document(&source);
        for diagnostic in document.diagnostics {
            failures.push(format!(
                "{}:{}:{} [{}] {}",
                path.strip_prefix(repository).unwrap_or(&path).display(),
                diagnostic.span.line,
                diagnostic.span.column,
                diagnostic.code,
                diagnostic.message
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "production .conduit source violates the frozen canonical surface:\n{}",
        failures.join("\n")
    );
}

fn collect_conduit_sources(directory: &Path, sources: &mut Vec<std::path::PathBuf>) {
    if !directory.is_dir() {
        return;
    }
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot inspect {}: {error}", directory.display()))
    {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            collect_conduit_sources(&path, sources);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "conduit")
        {
            sources.push(path);
        }
    }
}
