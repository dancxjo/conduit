use std::{fs, path::Path};

#[test]
fn architecture_rung_entrypoints_are_proof_owned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ordinary_bins = fs::read_dir(root.join("src/bin"))
        .expect("ordinary ConduitOS bin directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        ordinary_bins,
        [
            "aarch64_orange_pi_5.rs".into(),
            "aarch64_product.rs".into(),
            "ia32_product.rs".into(),
            "riscv64_product.rs".into(),
            "loongarch64_product.rs".into(),
        ]
        .into()
    );

    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("ConduitOS manifest");
    assert_eq!(manifest.matches("path = \"proof/appliances/").count(), 17);
    assert_eq!(manifest.matches("path = \"src/bin/").count(), 5);
    assert!(manifest.contains("path = \"src/bin/aarch64_orange_pi_5.rs\""));
    assert!(manifest.contains("path = \"src/bin/aarch64_product.rs\""));
    assert!(manifest.contains("path = \"src/bin/ia32_product.rs\""));
    assert!(manifest.contains("path = \"src/bin/riscv64_product.rs\""));
    assert!(manifest.contains("path = \"src/bin/loongarch64_product.rs\""));

    let ordinary_linkers = fs::read_dir(root.join("firmware/linker"))
        .expect("ordinary ConduitOS linker directory")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        ordinary_linkers,
        [
            "aarch64_orange_pi_5.ld".into(),
            "aarch64_product.ld".into(),
            "ia32_product.ld".into(),
            "riscv64_product.ld".into(),
            "loongarch64_product.ld".into(),
            "x86_64.ld".into(),
        ]
        .into()
    );
}
