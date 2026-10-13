//! Evidence admission: source packaging, execution and physical proof stay distinct.
use super::*;
const TIERS: [&str; 5] = [
    "compile-only",
    "deterministic-simulation",
    "emulator-with-device",
    "hardware-in-loop",
    "physical",
];
const IDENTITIES: [&str; 8] = [
    "source_artifact_id",
    "source_document_id",
    "checked_plot_id",
    "expanded_plot_id",
    "root_request_sha256",
    "kernel_sha256",
    "image_sha256",
    "architecture",
];

pub(super) fn validate(matrix: &Value, root: &Path) -> Result<(), String> {
    let rows = matrix["rows"].as_array().ok_or("missing rows")?;
    for row in rows {
        if row["physical_evidence"] != "unverified" {
            return Err("No physical receipt is inventoried".into());
        }
        let classes = row["classes"].as_object().ok_or("missing classes")?;
        for (name, cell) in classes {
            let tiers = cell["proof_tiers"]
                .as_object()
                .ok_or("missing proof tiers")?;
            if tiers.len() != TIERS.len() || TIERS.iter().any(|t| !tiers.contains_key(*t)) {
                return Err(format!("{name}: incomplete tier inventory"));
            }
            for (tier, proof) in tiers {
                let status = proof["status"].as_str().ok_or("missing status")?;
                if !["verified", "partial", "unsupported", "unverified"].contains(&status) {
                    return Err("unknown tier status".into());
                }
                if matches!(
                    tier.as_str(),
                    "physical" | "hardware-in-loop" | "deterministic-simulation"
                ) && status != "unverified"
                {
                    return Err(format!("{name}: no independently bound {tier} receipt"));
                }
            }
            let binding = cell.get("source_binding");
            if tiers["compile-only"]["status"] == "verified" && binding.is_none() {
                return Err("compilation needs checked Source binding".into());
            }
            if let Some(binding) = binding {
                let relative = binding["receipt"].as_str().ok_or("missing image receipt")?;
                let path = Path::new(relative);
                if path.is_absolute()
                    || path
                        .components()
                        .any(|p| !matches!(p, std::path::Component::Normal(_)))
                {
                    return Err("invalid receipt path".into());
                }
                let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
                if binding["receipt_sha256"] != format!("{:x}", Sha256::digest(&bytes)) {
                    return Err("image receipt hash mismatch".into());
                }
                let image: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                for identity in IDENTITIES {
                    let expected = if identity == "architecture" {
                        &row[identity]
                    } else {
                        &binding[identity]
                    };
                    if expected.as_str().is_none() || expected != &image[identity] {
                        return Err(format!("{name}: image {identity} mismatch"));
                    }
                }
            }
            let emulator = tiers["emulator-with-device"]["status"].as_str().unwrap();
            if cell["status"] == "unsupported" && emulator != "unsupported" {
                return Err("unsupported provider cannot claim execution".into());
            }
            if emulator == "verified"
                && (cell["status"] != "emulator-verified" || binding.is_none())
            {
                return Err(
                    "verified execution needs exact Source and execution disposition".into(),
                );
            }
            if matches!(emulator, "verified" | "partial")
                && cell["retained_product_proofs"]
                    .as_array()
                    .is_none_or(Vec::is_empty)
                && cell.get("receipt").is_none()
            {
                return Err("execution needs retained product receipt".into());
            }
        }
    }
    Ok(())
}
fn fixture() -> (Value, std::path::PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("plots/usb/evidence");
    let matrix =
        serde_json::from_slice(&fs::read(root.join("architecture-matrix.json")).unwrap()).unwrap();
    (matrix, root)
}
#[test]
fn stronger_claims_cannot_be_created_by_editing_a_status() {
    let (original, root) = fixture();
    validate(&original, &root).unwrap();
    for tier in ["physical", "hardware-in-loop", "deterministic-simulation"] {
        let mut matrix = original.clone();
        matrix["rows"][0]["classes"]["cdc-acm"]["proof_tiers"][tier]["status"] = "verified".into();
        assert!(validate(&matrix, &root).is_err(), "{tier}");
    }
    let mut matrix = original.clone();
    matrix["rows"][2]["classes"]["cdc-acm"]["proof_tiers"]["emulator-with-device"]["status"] =
        "verified".into();
    assert!(validate(&matrix, &root).is_err());
    let mut matrix = original.clone();
    matrix["rows"][0]["classes"]["cdc-acm"]
        .as_object_mut()
        .unwrap()
        .remove("source_binding");
    assert!(validate(&matrix, &root).is_err());
    let mut matrix = original;
    matrix["rows"][0]["classes"]["cdc-acm"]["source_binding"]["source_artifact_id"] =
        "invented".into();
    assert!(validate(&matrix, &root).is_err());
}
