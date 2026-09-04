use std::{fs, path::Path};

#[test]
fn pete_is_a_body_and_proof_specimens_are_not_ordinary_api() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        manifest.file_name().and_then(|name| name.to_str()),
        Some("pete")
    );
    assert_eq!(
        manifest
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str()),
        Some("bodies")
    );

    let library = fs::read_to_string(manifest.join("src/lib.rs")).expect("read Pete facade");
    assert!(library.contains("#[cfg(test)]\nmod proof;"));
    for forbidden in [
        "pub use capstone",
        "pub mod proof",
        "mod capstone;",
        "mod capstone_kernel;",
        "mod capstone_operations;",
        "mod capstone_play;",
    ] {
        assert!(
            !library.contains(forbidden),
            "ordinary Pete API exposes proof-only owner {forbidden}"
        );
    }
}

#[test]
fn foundational_crates_do_not_depend_on_pete() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    for owner in [
        "architecture",
        "fabrication",
        "mechanisms",
        "semantics",
        "targets",
    ] {
        assert_no_pete_dependency(&root.join(owner));
    }
}

fn assert_no_pete_dependency(directory: &Path) {
    for entry in fs::read_dir(directory).expect("read lower ownership directory") {
        let path = entry.expect("read lower ownership entry").path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some("target") {
                continue;
            }
            assert_no_pete_dependency(&path);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
            let contents = fs::read_to_string(&path).expect("read package manifest");
            assert!(
                !contents.contains("conduit-pete"),
                "lower package {} depends on the Pete application",
                path.display()
            );
        }
    }
}
