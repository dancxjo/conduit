use super::*;
use conduit_ai::{
    admit_candidate_form, llm_contract, CandidateFormRefusal, FormCompositionRequest,
    LlmDeterminismProfile, ModelDerivedResult, ModelResultDisposition, ModelResultProvenance,
    ModelWorkAccounting, LLM_COMPOSE_KIND,
};
use conduit_core::{
    BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PROTOCOL_VERSION,
};
use conduit_form::{ProfileCatalog, StartupCatalog};
use conduit_presentation::install_geometry_catalogs;
use conduit_semantic_catalog::install_vision_catalogs;

const INTENT: &str =
    "Watch this camera, recognize birds, count them by species, and show a dashboard.";
const SOURCE: &str = "form bird-observations {\n    camera: vision/deterministic-image\n    recognize: vision/deterministic-detector\n    camera.image > recognize.image\n}\n";

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_vision_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn request() -> FormCompositionRequest {
    FormCompositionRequest {
        request_identity: "request/bird-dashboard/7".into(),
        intent: INTENT.into(),
        catalog_basis_identity: "catalog/std-vision/1".into(),
    }
}

fn result(source: &str) -> ModelDerivedResult {
    let contract = llm_contract(LLM_COMPOSE_KIND).unwrap();
    ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: contract.result_payload_kind.as_str().into(),
        payload: source.as_bytes().to_vec(),
        implementation_identity: "implementation/deterministic-compose-fixture/1".into(),
        request_identity: request().request_identity,
        run_identity: "run/bird-dashboard/11".into(),
        confidence: None,
        disposition: ModelResultDisposition::Produced,
        determinism: LlmDeterminismProfile::DeterministicValidationFixture,
        accounting: ModelWorkAccounting {
            input_bytes: INTENT.len() as u64,
            context_items: 1,
            output_bytes: source.len() as u64,
            work_units: 1,
            history_items: 0,
        },
    }
}

fn candidate(source: &str) -> conduit_ai::CandidateForm {
    let (startup, profile) = catalogs();
    admit_candidate_form(
        &request(),
        result(source),
        Some("Known vision kinds produce bounded detections; species aggregation and a dashboard require later ordinary edits when exact compatible kinds exist.".into()),
        &startup,
        &profile,
    )
    .unwrap()
}

#[test]
fn natural_language_result_opens_as_visible_editable_inert_patchbay_form() {
    let admitted = candidate(SOURCE);
    assert_ne!(
        admitted.candidate_identity,
        admitted.provenance.request_identity
    );
    assert_ne!(
        admitted.candidate_identity,
        admitted.provenance.run_identity
    );
    assert!(!admitted.source.contains("implementation/deterministic"));
    assert!(!admitted.source.contains("request/bird-dashboard"));
    assert!(!admitted.source.contains("run/bird-dashboard"));

    let original_source_id = admitted.expanded.source_document_id.clone();
    let provenance = admitted.provenance.clone();
    let (startup, profile) = catalogs();
    let mut opened = PatchbayCandidateForm::open(admitted, startup, profile).unwrap();
    let view = opened.editor.view();
    assert_eq!(view.source, SOURCE);
    assert!(view.checked.diagnostics.is_empty());
    assert_eq!(view.checked.forms[0].items.len(), 3);
    assert_eq!(
        opened.lifecycle,
        CandidateLifecycle::AwaitingExplicitValidationPlanAndPlay
    );
    assert_eq!(opened.provenance, provenance);

    let edited = SOURCE.replace("bird-observations", "edited-bird-observations");
    opened.editor.replace_source(edited).unwrap();
    opened.editor.recheck().unwrap();
    let expanded = opened
        .editor
        .expand_form("edited-bird-observations")
        .unwrap();
    assert_ne!(expanded.source_document_id, original_source_id);
    assert_eq!(opened.provenance, provenance);
}

#[test]
fn malformed_invented_kind_and_invented_port_refuse_with_exact_diagnostics() {
    let (startup, profile) = catalogs();
    for (source, expected) in [
        ("form broken {", "closing"),
        (
            "form invented {\n bird: vision/magical-bird-counter\n}\n",
            "no startup signature is available for 'vision/magical-bird-counter'",
        ),
        (
            "form bad-port {\n camera: vision/deterministic-image\n recognize: vision/deterministic-detector\n camera.invented > recognize.image\n}\n",
            "gear 'camera' has no runtime port 'invented'",
        ),
    ] {
        let refusal = admit_candidate_form(
            &request(),
            result(source),
            None,
            &startup,
            &profile,
        )
        .unwrap_err();
        let CandidateFormRefusal::InvalidForm { message, .. } = refusal else {
            panic!("unexpected refusal: {refusal:?}")
        };
        assert!(message.contains(expected), "{message:?} did not contain {expected:?}");
    }
}

#[test]
fn generated_source_grants_no_authority_and_planning_tracks_current_offers() {
    let admitted = candidate(SOURCE);
    assert!(admitted.expanded.gears.iter().all(|gear| gear
        .configuration
        .iter()
        .all(|entry| !entry.key.contains("authority"))));

    let no_hosts = conduit_planner::default_expanded_placements(&admitted.expanded, &[]);
    assert!(no_hosts.is_err());
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/vision"),
        boot_id: BootId::from("boot/vision"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("profile/vision"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: conduit_std_offers::vision_std_offers(),
    };
    let placements = conduit_planner::default_expanded_placements(
        &admitted.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &admitted.expanded,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert!(plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .all(|placement| placement.authority.is_empty()));
}
