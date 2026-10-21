use super::*;
fn specimen() -> (Value, Value, Value, Vec<u8>, Vec<u8>) {
    let mut kernel = vec![0; 184];
    kernel[..6].copy_from_slice(b"\x7fELF\x02\x01");
    for (offset, value, size) in [
        (16, 2_u64, 2),
        (18, 62, 2),
        (24, 0x400000_u64, 8),
        (32, 64, 8),
        (40, 120, 8),
        (54, 56, 2),
        (56, 1, 2),
        (58, 64, 2),
        (60, 1, 2),
        (64, 1, 4),
        (68, 5, 4),
        (96, 184, 8),
        (104, 184, 8),
    ] {
        kernel[offset..offset + size].copy_from_slice(&value.to_le_bytes()[..size]);
    }
    let map = b"Root symbol map\n".to_vec();
    let hash = digest(&kernel);
    let build = json!({"schema":"conduit.conduitos.build/v2", "artifact_role":"product-host", "base_commit":"a".repeat(40),"architecture":"x86_64","rust_target":"x86_64-unknown-none","elf_sha256":hash});
    let manifest = json!({"schema":"conduit.host/target-build-manifest@3","artifact_role":"product-host","target":"conduitos/x86_64/pc","source_identity":"a".repeat(40),"kernel":{"sha256":hash}});
    let inventory = json!({"schema":"conduit.conduitos.privilege-inventory/v2","proof_class":"build-dependency-and-link-observation","build":build,"requested_features":["native-compositor"],"resolved_dependencies":"conduitos v0.1.0 (fixture) features=[default,native-compositor,native-owner-network]","root_link_map":format!("privilege-inventory-{hash}-product-host.map"),"root_link_map_sha256":digest(&map),"working_tree_status":"","service_isolation_established":false,"linked_images":elf::observe(&kernel).unwrap(),"linked_responsibilities":super::super::linked_responsibilities(&map)});
    (manifest, build, inventory, kernel, map)
}
#[test]
fn exact_product_association_and_replaced_directory_record() {
    let (manifest, build, inventory, kernel, map) = specimen();
    associate(&manifest, &build, &inventory, &kernel, &map).unwrap();
    let mut replacement = build.clone();
    replacement["elf_sha256"] = json!("b".repeat(64));
    assert!(associate(&manifest, &replacement, &inventory, &kernel, &map).is_err());
}
#[test]
fn corrupt_or_foreign_kernel_source_role_map_and_features_refuse() {
    let (manifest, build, inventory, kernel, map) = specimen();
    for (field, value) in [
        ("base_commit", json!("b".repeat(40))),
        ("artifact_role", json!("architecture-proof-appliance")),
        ("architecture", json!("ia32")),
        ("rust_target", json!("i686-unknown-uefi")),
    ] {
        let mut wrong = build.clone();
        wrong[field] = value;
        let mut related = inventory.clone();
        related["build"] = wrong.clone();
        assert!(
            associate(&manifest, &wrong, &related, &kernel, &map).is_err(),
            "{field}"
        );
    }
    let mut corrupt = kernel.clone();
    corrupt[100] ^= 1;
    assert!(associate(&manifest, &build, &inventory, &corrupt, &map).is_err());
    assert!(associate(&manifest, &build, &inventory, &kernel, b"foreign map").is_err());
    for (field, value) in [
        (
            "requested_features",
            json!(["native-compositor", "scripted-keyboard-proof"]),
        ),
        (
            "resolved_dependencies",
            json!("conduitos features=[default]"),
        ),
        ("root_link_map", json!("another.map")),
        ("working_tree_status", json!(" M src/main.rs")),
        ("linked_images", json!([])),
    ] {
        let mut wrong = inventory.clone();
        wrong[field] = value;
        assert!(
            associate(&manifest, &build, &wrong, &kernel, &map).is_err(),
            "{field}"
        );
    }
}
#[test]
fn dependency_or_module_evidence_cannot_become_isolation_or_execution_proof() {
    let (manifest, build, inventory, kernel, map) = specimen();
    for (field, value) in [
        ("service_isolation_established", json!(true)),
        ("execution_established", json!(true)),
        ("service_isolation_established", Value::Null),
        ("proof_class", json!("executed-hardware-isolation")),
        ("schema", json!("conduit.conduitos.privilege-inventory/v1")),
    ] {
        let mut forged = inventory.clone();
        forged[field] = value;
        assert!(
            associate(&manifest, &build, &forged, &kernel, &map).is_err(),
            "{field}"
        );
    }
}
#[test]
fn malformed_embedded_elf_is_not_a_service_and_root_segments_are_checked() {
    let (_, _, _, mut kernel, _) = specimen();
    kernel.extend_from_slice(b"\x7fELF\x02\x01 not a linked image");
    assert_eq!(elf::observe(&kernel).unwrap().len(), 1);
    kernel[96..104].copy_from_slice(&99999_u64.to_le_bytes());
    assert!(elf::observe(&kernel).is_err());
}
#[test]
fn reviewed_source_contract_covers_current_service_profiles_and_authority() {
    let paths = Paths::new(ConduitosArch::X86_64).unwrap();
    assert_eq!(source::observe(&paths.root, None).unwrap().len(), 21);
}

#[test]
fn transitive_only_feature_substitution_refuses_with_unchanged_root_line() {
    let (_, _, mut inventory, _, _) = specimen();
    let actual = format!(
        "{}\n  serde v1 features=[std]\n",
        inventory["resolved_dependencies"].as_str().unwrap()
    );
    inventory["resolved_dependencies"] = json!(actual);
    match_closure(&inventory, &actual).unwrap();
    inventory["resolved_dependencies"] =
        json!(actual.replace("serde v1 features=[std]", "serde v1 features=[std,derive]"));
    assert_eq!(
        inventory["resolved_dependencies"]
            .as_str()
            .unwrap()
            .lines()
            .next(),
        actual.lines().next()
    );
    assert!(match_closure(&inventory, &actual).is_err());
}
