//! Development transport for the exact checked wrapper and sealed ordinary Plan.
//! Content/seal correspondence does not admit resources, Native laws, or a target.
use super::*;

pub(super) struct CheckedEpochSource {
    pub(super) text: String,
    pub(super) identity: SourceDocumentId,
}

pub(super) fn plan_image(
    plan: &Plan,
    source: &CheckedEpochSource,
) -> Result<Vec<u8>, &'static str> {
    if !verify_plan(plan)
        || source.identity != plan.source_document_id
        || syntax_source_document_identity(&source.text) != source.identity
    {
        return Err("sealed Plan and exact checked Source do not correspond");
    }
    let bytes = serde_json::to_vec(plan).map_err(|_| "Plan image encode")?;
    let decoded: Plan = serde_json::from_slice(&bytes).map_err(|_| "Plan image decode")?;
    if decoded != *plan || !verify_plan(&decoded) {
        return Err("Plan image round trip differs");
    }
    Ok(bytes)
}

pub(super) fn export(directory: &std::path::Path, plan: &Plan, context: &EpochProfiles) {
    use sha2::{Digest, Sha256};
    let source = context
        .checked_source
        .as_ref()
        .expect("exact checked wrapper retained");
    let image = plan_image(plan, source).unwrap();
    std::fs::write(directory.join("checked-epoch-source.conduit"), &source.text).unwrap();
    std::fs::write(directory.join("sealed-epoch-plan.json"), &image).unwrap();
    let manifest = serde_json::json!({
        "schema": "conduit/development-checked-source-plan-image@1",
        "plan_encoding": "conduit/core-plan-serde-json-development@1",
        "plan_id": plan.plan_id.as_str(),
        "source_document_id": source.identity.as_str(),
        "source_bytes": source.text.len(),
        "source_sha256": format!("{:x}", Sha256::digest(source.text.as_bytes())),
        "plan_bytes": image.len(),
        "plan_sha256": format!("{:x}", Sha256::digest(&image)),
        "fragments": plan.fragments.len(),
        "placements": plan.fragments.iter().map(|f| f.placements.len()).sum::<usize>(),
        "scope": "exact checked wrapper and sealed ordinary Plan; resource/native/target admission separate"
    });
    std::fs::write(
        directory.join("checked-plan-image-manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn exact_checked_wrapper_and_sealed_plan_image_refuse_correspondence_drift() {
    let (mut plan, context) = prepared_epoch_plan(true).unwrap();
    let source = context.checked_source.as_ref().unwrap();
    assert!(source.text.contains("plot epoch-runtime-proof"));
    let checked =
        check_syntax_document(&parse_syntax_document(&source.text), &context.startup).unwrap();
    assert_eq!(checked.source_document_id, source.identity);
    let image = plan_image(&plan, source).unwrap();
    eprintln!(
        "exact checked Source {} bytes; Plan image {} bytes; {} placements",
        source.text.len(),
        image.len(),
        plan.fragments[0].placements.len()
    );
    let foreign = CheckedEpochSource {
        text: format!("{}\n", source.text),
        identity: source.identity.clone(),
    };
    assert!(plan_image(&plan, &foreign).is_err());
    let foreign = CheckedEpochSource {
        text: source.text.clone(),
        identity: "foreign/source".into(),
    };
    assert!(plan_image(&plan, &foreign).is_err());
    plan.fragments[0].placements[0].kind_contract_revision = "foreign/revision".into();
    assert!(plan_image(&plan, source).is_err());
}
