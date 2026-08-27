use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("lowering crate has repository root")
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
    let lowering_manifest = read(&root, "crates/conduit-plan-lowering/Cargo.toml");
    let lowering_root = read(&root, "crates/conduit-plan-lowering/src/lib.rs");
    let std_manifest = read(&root, "hosts/std/Cargo.toml");
    let signal_manifest = read(&root, "crates/conduit-signal/Cargo.toml");
    let composite_manifest = read(&root, "crates/conduit-composite/Cargo.toml");

    for (surface, source) in [
        ("workspace", root_manifest.as_str()),
        ("lowering", lowering_manifest.as_str()),
        ("lowering root", lowering_root.as_str()),
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
        .join("crates/conduit-plan-lowering/src/compatibility_executor.rs")
        .exists());
    assert!(!root
        .join("crates/conduit-composite/src/compatibility.rs")
        .exists());
}

#[test]
fn current_hosts_depend_only_on_exact_lowering_and_kernel_execution() {
    let root = repository_root();
    let lowering_manifest = read(&root, "crates/conduit-plan-lowering/Cargo.toml");
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

    let lowering_root = read(&root, "crates/conduit-plan-lowering/src/lib.rs");
    let lowering_doc = read(&root, "docs/architecture/plan-kernel-lowering.md");
    assert!(lowering_root.contains("pub mod lowering;"));
    assert!(lowering_doc.contains("does not plan, schedule, execute semantic implementations"));
    for forbidden in [
        "conduit-std-host",
        "conduit-browser-host",
        "conduit-observatory",
        "clap",
        "std::thread",
    ] {
        assert!(
            !lowering_manifest.contains(forbidden) && !lowering_root.contains(forbidden),
            "lowering package accumulated product lifecycle or Host adapter responsibility: {forbidden}"
        );
    }
    let composite_root = read(&root, "crates/conduit-composite/src/lib.rs");
    assert!(composite_root.contains("mod kernel_executor;"));
}

#[test]
fn product_entrance_does_not_reason_in_numeric_kernel_identity() {
    let root = repository_root();
    for relative in [
        "crates/conduit/src/main.rs",
        "crates/conduit/src/product_execution.rs",
        "crates/conduit/src/body_product.rs",
    ] {
        let source = read(&root, relative);
        for numeric_identity in ["NodeId", "CordId", "conduit_kernel::PortId"] {
            assert!(
                !source.contains(numeric_identity),
                "{relative} reached below Plan-to-kernel lowering for {numeric_identity}"
            );
        }
    }
}
