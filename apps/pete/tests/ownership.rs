use std::{fs, path::Path};

#[test]
fn pete_is_an_application_and_proof_specimens_are_not_ordinary_api() {
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
        Some("apps")
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
    let crates = root.join("crates");
    for entry in fs::read_dir(crates).expect("read foundational crate directory") {
        let package = entry.expect("read crate entry").path();
        let package_manifest = package.join("Cargo.toml");
        if !package_manifest.is_file() {
            continue;
        }
        let contents = fs::read_to_string(&package_manifest).expect("read crate manifest");
        assert!(
            !contents.contains("conduit-pete"),
            "foundational crate {} depends on the Pete application",
            package.display()
        );
    }
}
