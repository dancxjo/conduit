use super::*;

const EXACT_PROPOSAL: &str = r#"{"proposal_id":"proposal/live","request_identity":"request/live","run_identity":"run/live","checked_form_id":"checked/live","placements":[{"gear_id":"advised/pulse","host_id":"advice/host-b","boot_id":"advice/boot-b","offer_generation":7,"capability_id":"advice/pulse-b"}]}"#;

#[test]
fn wire_schema_refuses_extra_plan_shaped_fields_and_empty_placements() {
    let with_plan = r#"{"proposal_id":"p","request_identity":"r","run_identity":"x","checked_form_id":"c","placements":[],"plan_id":"forged"}"#;
    assert!(serde_json::from_str::<WireProposal>(with_plan).is_err());
    let empty: WireProposal = serde_json::from_str(
        r#"{"proposal_id":"p","request_identity":"r","run_identity":"x","checked_form_id":"c","placements":[]}"#,
    )
    .unwrap();
    assert!(convert(empty).is_err());
}

#[test]
fn configuration_refuses_credentials_tls_promotion_and_unbounded_model_identity() {
    assert!(validate_configuration("http://forebrain.local:11434", "gpt-oss:20b").is_ok());
    assert!(validate_configuration("https://forebrain.local", "gpt-oss:20b").is_err());
    assert!(validate_configuration("http://token@forebrain.local", "gpt-oss:20b").is_err());
    assert!(validate_configuration("http://forebrain.local", &"x".repeat(129)).is_err());
}

#[test]
fn exact_gpt_oss_proposal_converts_without_line_or_plan_invention() {
    let advice = convert(serde_json::from_str(EXACT_PROPOSAL).unwrap()).unwrap();
    assert_eq!(advice.placements.len(), 1);
    assert!(advice.lines.is_empty());
    assert_eq!(advice.placements[0].host_id.as_str(), "advice/host-b");
}

#[test]
fn validated_proposal_serializes_as_bounded_advisory_evidence() {
    let proposal: WireProposal = serde_json::from_str(EXACT_PROPOSAL).unwrap();
    let encoded = serde_json::to_vec(&proposal).unwrap();
    assert!(encoded.len() <= MAXIMUM_PROPOSAL_BYTES);
    let retained: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(retained["proposal_id"], "proposal/live");
    assert_eq!(retained["request_identity"], "request/live");
    assert_eq!(retained["run_identity"], "run/live");
    assert_eq!(retained["checked_form_id"], "checked/live");
    assert_eq!(retained["placements"][0]["host_id"], "advice/host-b");
    assert!(retained.get("plan_id").is_none());
    assert!(retained.get("lines").is_none());
}
