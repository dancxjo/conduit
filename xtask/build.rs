use std::path::Path;

#[path = "src/commands/catalog_index.rs"]
mod catalog_index;

const CATALOG_INPUTS: &[&str] = &[
    "src/commands/catalog_index.rs",
    "../crates/conduit-compile/src",
    "../crates/conduit-core/src",
    "../crates/conduit-learned/src",
    "../crates/conduit-media/src",
    "../crates/conduit-net/src",
    "../crates/conduit-runtime/src",
    "../crates/conduit-spatial/src",
    "../crates/conduit-std/src",
    "../conformance",
    "../library/catalog.json",
    "../docs/library-tour-index.md",
];

fn main() {
    for input in CATALOG_INPUTS {
        println!("cargo::rerun-if-changed={input}");
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("xtask must remain directly beneath the workspace root");

    if let Err(check_error) = catalog_index::run(workspace_root, true) {
        match catalog_index::run(workspace_root, false) {
            Ok(()) => panic!(
                "catalog build inputs changed: {check_error}; regenerated library/catalog.json \
                 and docs/library-tour-index.md; include them in this change and rebuild"
            ),
            Err(regeneration_error) => panic!(
                "catalog build failed: {check_error}; regeneration also failed: \
                 {regeneration_error}"
            ),
        }
    }
}
