#[path = "../../../../semantics/catalog/tests/common/experience_evidence.rs"]
mod common;
use super::{host, installed_std, RecordingTimer};
use common::*;
use conduit_core::{
    ConfigurationValue, ObservationKind, PortDirection, PortTemporal, StructuredInfoValue,
    TerminalDisposition,
};
use conduit_human::{
    ExperienceAvailability, ExperienceEvidenceContext, ExperienceEvidenceRequest,
    ExperienceEvidenceSource, ExperienceOrigin, ExperienceTemporalRole,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document,
    rust_binding::NativeRustBinding, KindConfigurationField, KindConfigurationRule, KindProjection,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};

const SOURCE: &str = "conduit-test/evidence-request";
const SINK: &str = "conduit-test/evidence-context";

#[test]
fn ordinary_std_offer_plans_lowers_and_executes_explicit_evidence_without_play_growth() {
    let observed = with_sources(
        observation("observation"),
        vec![
            resource_source(),
            ExperienceEvidenceSource::implementation_run(
                identity("artifact/7"),
                identity("implementation/7"),
                identity("provider/7"),
                identity("run/7"),
            )
            .unwrap(),
            ExperienceEvidenceSource::human_statement(identity("statement/7")).unwrap(),
            ExperienceEvidenceSource::sign(identity("sign/7")).unwrap(),
        ],
    );
    let memory = item(
        "memory",
        ExperienceOrigin::Remembered,
        ExperienceTemporalRole::Historical,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"bounded selected memory",
    );
    let inference = item(
        "inference",
        ExperienceOrigin::ModelDerived,
        ExperienceTemporalRole::Stale,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"derived, not observed",
    );
    let ids = ["memory", "inference", "observation"];
    let selection = selection_with_relationships(&ids, &[("observation", "inference")]);
    let request = ExperienceEvidenceRequest::new(
        items(vec![observed.clone(), inference.clone(), memory.clone()]),
        selection.clone(),
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let expected =
        ExperienceEvidenceContext::new(items(vec![memory, inference, observed]), selection)
            .unwrap()
            .into_structured()
            .unwrap();
    let report = run(&request, &expected).expect("ordinary installed execution");
    assert!(matches!(
        report.observations.last().map(|event| &event.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("real installed kernel");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.identity.lengths(), (0, 0, 1));
}

#[test]
fn ordinary_kernel_refuses_stale_evidence_instead_of_emitting_context() {
    let stale = item(
        "a",
        ExperienceOrigin::Observation,
        ExperienceTemporalRole::Current,
        ExperienceAvailability::Present,
        6,
        "provenance/7",
        b"stale revision",
    );
    let request = request(&["a"], vec![stale.clone()])
        .into_structured()
        .unwrap();
    let expected = ExperienceEvidenceContext::new(items(vec![stale]), selection(&["a"]))
        .unwrap()
        .into_structured()
        .unwrap();
    let failure = run(&request, &expected).expect_err("stale input must fail the real kernel");
    assert!(
        failure.contains("BackFailed(Failure { code: InvalidInput, detail: 9 })"),
        "{failure}"
    );
    assert!(
        failure.contains("logical-gear=evidence-proof/project"),
        "{failure}"
    );
}

#[test]
fn projection_is_explicitly_composed_and_advertises_no_ambient_authority() {
    let offer = conduit_std_offers::experience_evidence_offer();
    let contract = conduit_semantic_catalog::experience_evidence_contract();
    assert_eq!(offer.inputs, contract.inputs);
    assert_eq!(offer.outputs, contract.outputs);
    assert_eq!(offer.semantic_contract, contract.semantic_contract());
    assert!(offer.host_calls.is_empty());
    assert!(offer.resource_requirements.is_empty());
    assert!(offer.authority_requirements.is_empty());
    assert!(!crate::StdHostComposition::minimal().experience_evidence);
    assert!(
        crate::StdHostComposition::minimal()
            .with_experience_evidence()
            .experience_evidence
    );
    assert!(host("evidence-reference")
        .advertisement()
        .capabilities
        .iter()
        .any(|offered| offered == &offer));
}

fn run(
    request: &StructuredInfoValue,
    expected: &StructuredInfoValue,
) -> Result<crate::StdRunReport, String> {
    let source_offer = fixture_offer(request, PortDirection::Output);
    let sink_offer = fixture_offer(expected, PortDirection::Input);
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_experience_evidence_catalogs(&mut startup, &mut profile)
        .unwrap();
    for (kind, value, offer) in [
        (SOURCE, request, &source_offer),
        (SINK, expected, &sink_offer),
    ] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![StartupParameterSignature {
                    name: "value".into(),
                    value_type: "Text".into(),
                    default: None,
                }],
            })
            .unwrap();
        profile
            .insert(KindProjection {
                kind_id: offer.kind_id.clone(),
                kind_contract_revision: offer.kind_contract_revision.clone(),
                inputs: offer.inputs.clone(),
                outputs: offer.outputs.clone(),
                configuration: vec![KindConfigurationField {
                    key: "value".into(),
                    default_value: ConfigurationValue::Text(hex(&value.canonical_bytes().unwrap())),
                    rule: KindConfigurationRule::TextBytes { maximum: 131_072 },
                }],
            })
            .unwrap();
    }
    let source = format!(
        "plot evidence-proof {{\n input: {SOURCE}(value = \"{}\")\n project: experience/project-evidence\n result: {SINK}(value = \"{}\")\n input.output >> project.request\n project.context >> result.input\n}}\n",
        hex(&request.canonical_bytes().unwrap()), hex(&expected.canonical_bytes().unwrap()),
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "evidence-proof", &profile).unwrap();
    let mut execution_host = host("evidence-host");
    let mut advertisement = execution_host.advertisement().clone();
    advertisement
        .capabilities
        .extend([source_offer, sink_offer]);
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        conduit_planner::PlanningOptions {
            connection_bases: &Default::default(),
            line_candidates: &Default::default(),
            connection_item_capacity: 1,
            connection_byte_capacity: 65_536,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 3);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    assert_eq!(lowered.nodes.len(), 3);
    assert!(lowered.host_calls.is_empty());
    execution_host.advertisement = advertisement;
    execution_host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&execution_host.advertisement)
            .unwrap();
    execution_host.run_fragment_to(
        fragment.clone(),
        &mut Vec::with_capacity(1024),
        &mut RecordingTimer { waits: Vec::new() },
    )
}

fn fixture_offer(
    value: &StructuredInfoValue,
    direction: PortDirection,
) -> conduit_core::CapabilityOffer {
    let mut offer = installed_std::test_structured_selector::offer_named(
        value.value_type(),
        direction,
        SOURCE,
        SINK,
    );
    let port = offer
        .outputs
        .first_mut()
        .or_else(|| offer.inputs.first_mut())
        .unwrap();
    port.temporal = PortTemporal::Value;
    offer.startup_parameters[0].has_default = false;
    installed_std::test_structured_selector::bind_text_configuration(
        &mut offer,
        "value",
        hex(&value.canonical_bytes().unwrap()),
        131_072,
    );
    offer
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
