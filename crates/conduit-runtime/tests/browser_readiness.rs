use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("runtime crate has repository root")
        .to_path_buf()
}

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

#[test]
fn v1_has_one_runtime_and_no_compiled_compatibility_closure() {
    let root = repository_root();
    let root_manifest = read(&root, "Cargo.toml");
    let runtime_manifest = read(&root, "crates/conduit-runtime/Cargo.toml");
    let runtime_root = read(&root, "crates/conduit-runtime/src/lib.rs");
    let std_manifest = read(&root, "hosts/std/Cargo.toml");
    let signal_manifest = read(&root, "crates/conduit-signal/Cargo.toml");
    let composite_manifest = read(&root, "crates/conduit-composite/Cargo.toml");

    for (surface, source) in [
        ("workspace", root_manifest.as_str()),
        ("runtime", runtime_manifest.as_str()),
        ("runtime root", runtime_root.as_str()),
        ("std host", std_manifest.as_str()),
        ("signal", signal_manifest.as_str()),
        ("composite", composite_manifest.as_str()),
    ] {
        assert!(
            !source.contains("compatibility-executor"),
            "{surface} must not restore the removed second executor"
        );
        assert!(
            !source.contains("legacy-fixture-driver"),
            "{surface} must not restore the removed legacy driver"
        );
        assert!(
            !source.contains("compatibility-fixture"),
            "{surface} must not restore a feature-selected compatibility runtime"
        );
    }

    assert!(!root.join("fixtures/browser-sim/Cargo.toml").exists());
    assert!(!root.join("fixtures/pico-sim/Cargo.toml").exists());
    assert!(!root
        .join("crates/conduit-runtime/src/compatibility_executor.rs")
        .exists());
    assert!(!root
        .join("crates/conduit-composite/src/compatibility.rs")
        .exists());
}

#[test]
fn current_hosts_depend_only_on_exact_lowering_and_kernel_execution() {
    let root = repository_root();
    for relative in [
        "hosts/std/src/lib.rs",
        "hosts/browser-runtime/src/lib.rs",
        "crates/conduit-composite/src/kernel_executor.rs",
        "firmware/conduit-pico-w-signal/build.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            !source.contains("HostRuntime"),
            "{relative} restored HostRuntime"
        );
    }

    let runtime_root = read(&root, "crates/conduit-runtime/src/lib.rs");
    assert!(runtime_root.contains("pub mod lowering;"));
    let composite_root = read(&root, "crates/conduit-composite/src/lib.rs");
    assert!(composite_root.contains("mod kernel_executor;"));
}
