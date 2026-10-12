//! Retained architecture evidence is checked without upgrading its proof tier.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
fn check_receipts(value: &Value, root: &Path) {
    match value {
        Value::Object(fields) => {
            if let Some(receipt) = fields.get("receipt").and_then(Value::as_str) {
                let relative = Path::new(receipt);
                assert!(!relative.is_absolute());
                assert!(
                    relative
                        .components()
                        .all(|p| matches!(p, std::path::Component::Normal(_)))
                );
                let expected = fields
                    .get("receipt_sha256")
                    .and_then(Value::as_str)
                    .expect("receipt must retain exact hash");
                let bytes =
                    fs::read(root.join(relative)).unwrap_or_else(|e| panic!("{receipt}: {e}"));
                assert_eq!(
                    format!("{:x}", Sha256::digest(&bytes)),
                    expected,
                    "{receipt}"
                );
            }
            for child in fields.values() {
                check_receipts(child, root)
            }
        }
        Value::Array(items) => {
            for child in items {
                check_receipts(child, root)
            }
        }
        _ => {}
    }
}
#[test]
fn published_usb_matrix_binds_existing_exact_receipts_and_explicit_target_dispositions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("plots/usb/evidence");
    let matrix: Value =
        serde_json::from_slice(&fs::read(root.join("architecture-matrix.json")).unwrap()).unwrap();
    assert_eq!(matrix["issue"], 5258);
    assert_eq!(
        matrix["schema"],
        "conduit.usb/architecture-acceptance-matrix@1"
    );
    let rows = matrix["rows"].as_array().unwrap();
    let names: BTreeSet<_> = rows
        .iter()
        .map(|r| r["architecture"].as_str().unwrap())
        .collect();
    assert_eq!(rows.len(), 5);
    assert_eq!(
        names,
        BTreeSet::from(["x86_64", "aarch64", "ia32", "riscv64", "loongarch64"])
    );
    for row in rows {
        assert_eq!(row["physical_evidence"], "unverified");
        for class in ["control", "hid", "cdc-acm", "cdc-ecm", "bot", "uac"] {
            let entry = &row["classes"][class];
            let status = entry["status"]
                .as_str()
                .expect("each class needs disposition");
            assert!(matches!(status, "incomplete" | "unsupported"));
            assert!(
                !entry["reason"].as_str().unwrap_or("").is_empty(),
                "{} {class}",
                row["architecture"]
            );
            if status == "unsupported" {
                assert_ne!(entry["acceptance"], "complete")
            }
        }
    }
    check_receipts(&matrix, &root);
}
