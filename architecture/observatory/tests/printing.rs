use conduit_core::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_plot::{ProfileCatalog, StartupCatalog};
use conduit_semantic_catalog::*;
use std::collections::BTreeMap;

use conduit_observatory::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn plan() -> Plan {
    arrangement().0
}
fn arrangement() -> (Plan, HostAdvertisement) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_print_catalogs(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../plots/print-contract/main.conduit");
    assert!(!source.contains("ipp://"));
    assert!(!source.contains("/dev/"));
    let plot = conduit_plot::parse_with_startup(source, &startup, &profile).unwrap();
    assert_eq!(plot.gears.len(), 3);
    let kind = print_submit_semantic_contract();
    assert!(kind
        .semantic_laws
        .contains(&KindSemanticLaw::Replay(ReplayBehavior::Ineligible)));
    assert!(kind
        .semantic_laws
        .contains(&KindSemanticLaw::ExternalEffects(
            ExternalEffectBehavior::Observable
        )));
    let authority = AuthorityRequirement {
        contract_id: PRINT_DOCUMENT_AUTHORITY.into(),
        host_call_contract_id: PRINT_SUBMIT_HOST_CALL.into(),
        subject_kind: kind.kind_id.clone(),
    };
    let mut offer = capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters:kind.startup_parameters.clone(),shorthand:kind.shorthand.clone(),
        capability_id:"fixture/print-provider".into(),kind_id:kind.kind_id.clone(),kind_contract_revision:kind.kind_contract_revision.clone(),
        implementation:ImplementationOffer {execution_profile_id:"fixture/print-simulator@1".into(),implementation_id:"fixture/print-protocol@1".into(),artifact_id:"fixture/print-protocol-artifact@1".into()},
        inputs:kind.inputs.clone(),outputs:kind.outputs.clone(),
        host_calls:vec![HostCallRequirement {contract_id:PRINT_SUBMIT_HOST_CALL.into(),target_kind:Some(kind.kind_id.clone()),maximum_in_flight:1,maximum_input_bytes:8192,maximum_output_bytes:8192}],
        resource_requirements:vec![resource_requirement("fixture/print-slot",1)],
        authority_requirements:vec![authority.clone()],
        limits:CapabilityLimits {max_active_instances:3,max_queue_items:1,max_queue_bytes:8192},
    };
    let support = PrintSupport::new(
        PrintColorSupport::new(
            BoundedSequence::try_from_iter([PrintColor::Monochrome, PrintColor::Color]).unwrap(),
        )
        .unwrap(),
        10,
        PrintMediaSupport::new(
            BoundedSequence::try_from_iter([PrintMedia::A4, PrintMedia::Letter]).unwrap(),
        )
        .unwrap(),
        PrintSidesSupport::new(
            BoundedSequence::try_from_iter([PrintSides::OneSided, PrintSides::DuplexLongEdge])
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    offer.realization_properties = vec![support.property().unwrap()];
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/printer-owner".into(),
        boot_id: "fixture/printer-owner/boot-1".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/print-protocol-only@1".into(),
        bases: vec![],
        resources: vec![resource_offer(
            "fixture/print-slot",
            "fixture/print-slot",
            3,
        )],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let placements =
        conduit_planner::default_placements(&plot, core::slice::from_ref(&host)).unwrap();
    let grant = authority_grant(
        "grant/print",
        &authority,
        host.host_id.clone(),
        host.boot_id.clone(),
        host.capabilities[0].capability_id.clone(),
    );
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let prepare = |grants: &[AuthorityGrant]| {
        conduit_planner::plan_with_options(
            &plot,
            core::slice::from_ref(&host),
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &empty,
                line_candidates: &lines,
                connection_item_capacity: 1,
                connection_byte_capacity: 8192,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
    };
    assert!(prepare(&[]).is_err());
    let plan = prepare(core::slice::from_ref(&grant)).unwrap();
    (plan, host)
}

fn identity(text: &str) -> conduit_process::PrintIdentity {
    conduit_process::PrintIdentity::new(text.into()).unwrap()
}
fn evidence(event: conduit_process::PrintLifecycle) -> SignArtifact {
    let plan = plan();
    let provider = &plan.fragments[0].placements[0];
    let identity = bind_sign(&provider.host_id, &provider.boot_id, None, 1);
    let sign = Observation {
        sign_id: identity.sign_id.clone(),
        active_play_id: None,
        face_id: None,
        host_id: provider.host_id.clone(),
        boot_id: provider.boot_id.clone(),
        plan_id: Some(plan.plan_id.clone()),
        placement_id: Some(provider.placement_id.clone()),
        connection_id: None,
        kind: ObservationKind::ValueProduced {
            value: ValuePayload {
                value_kind: conduit_process::PrintLifecycle::semantic_type()
                    .unwrap()
                    .profile()
                    .unwrap()
                    .value_kind()
                    .clone(),
                encoded: event.encode().unwrap(),
            },
        },
    };
    SignArtifact {
        schema: SIGN_ARTIFACT_SCHEMA.into(),
        identity,
        plan: Some(plan),
        play: None,
        sign,
    }
}

#[test]
fn lost_ack_is_uncertain_and_preserves_exact_inspectable_evidence() {
    use conduit_process::*;
    let event = PrintLifecycle::uncertain(
        PrintOperation::new(identity("attempt/one"), identity("job/one")).unwrap(),
        None,
        PrintUncertainty::AcknowledgementUnknown,
    )
    .unwrap();
    let artifact = evidence(event.clone());
    let before = serde_json::to_vec(&artifact).unwrap();
    let inspection = inspect_print_sign(&artifact).unwrap();
    assert_eq!(inspection.lifecycle(), &event);
    let text = inspection.render_text();
    assert!(text.contains("delivery uncertain; retry not authorized"));
    for value in [
        artifact.sign.sign_id.as_str(),
        artifact.plan.as_ref().unwrap().source_document_id.as_str(),
        inspection.provider().implementation_id.as_str(),
        "job/one",
        "attempt/one",
        "AcknowledgementUnknown",
    ] {
        assert!(text.contains(value), "missing {value}");
    }
    assert_eq!(serde_json::to_vec(&artifact).unwrap(), before);
}

#[test]
fn queued_ack_completed_cancelled_and_failed_remain_distinct() {
    use conduit_process::*;
    let op = PrintOperation::new(identity("attempt/one"), identity("job/one")).unwrap();
    for (event, label) in [
        (
            PrintLifecycle::queued(identity("attempt/one"), identity("job/one")).unwrap(),
            "queued",
        ),
        (
            PrintLifecycle::dispatched(identity("attempt/one"), identity("job/one")).unwrap(),
            "dispatched; ACK not observed",
        ),
        (
            PrintLifecycle::acknowledged(op.clone(), identity("provider/job")).unwrap(),
            "provider acknowledged",
        ),
        (
            PrintLifecycle::completed(op.clone(), identity("provider/job")).unwrap(),
            "completed",
        ),
        (
            PrintLifecycle::cancelled(identity("attempt/one"), identity("job/one")).unwrap(),
            "cancelled before dispatch",
        ),
        (
            PrintLifecycle::failed(op, PrintFailure::RejectedBeforeDispatch).unwrap(),
            "failed (provider evidence)",
        ),
    ] {
        let artifact = evidence(event);
        assert!(inspect_print_sign(&artifact)
            .unwrap()
            .render_text()
            .starts_with(&format!("Print: {label}\n")));
    }
}

#[test]
fn mismatched_provider_plan_type_and_nonprovider_events_are_refused() {
    use conduit_process::*;
    let artifact =
        evidence(PrintLifecycle::queued(identity("attempt/one"), identity("job/one")).unwrap());
    let mut wrong = artifact.clone();
    wrong.sign.plan_id = Some("wrong-plan".into());
    assert!(inspect_print_sign(&wrong).is_err());
    let mut wrong = artifact.clone();
    wrong.sign.placement_id = Some("wrong-placement".into());
    assert!(inspect_print_sign(&wrong).is_err());
    let mut wrong = artifact.clone();
    wrong.sign.kind = ObservationKind::PlanCompleted;
    assert!(inspect_print_sign(&wrong).is_err());
    let mut wrong = artifact.clone();
    if let ObservationKind::ValueProduced { value } = &mut wrong.sign.kind {
        value.value_kind = kind_id("value/text");
    }
    assert!(inspect_print_sign(&wrong).is_err());
    let mut wrong = artifact.clone();
    if let ObservationKind::ValueProduced { value } = &mut wrong.sign.kind {
        value.encoded = vec![0, 1, 2];
    }
    assert!(inspect_print_sign(&wrong).is_err());
    let mut wrong = artifact.clone();
    wrong.sign.host_id = "foreign/provider".into();
    wrong.identity = bind_sign(&wrong.sign.host_id, &wrong.sign.boot_id, None, 1);
    wrong.sign.sign_id = wrong.identity.sign_id.clone();
    assert!(inspect_print_sign(&wrong).is_err());
    let mut wrong = artifact;
    if let ObservationKind::ValueProduced { value } = &mut wrong.sign.kind {
        value.encoded = vec![0; PRINT_MAXIMUM_WIRE_BYTES + 1];
    }
    assert!(inspect_print_sign(&wrong).is_err());
}

#[test]
fn ordinary_snapshot_text_report_presents_typed_print_delivery_uncertainty() {
    use conduit_process::*;
    let event = PrintLifecycle::uncertain(
        PrintOperation::new(identity("attempt/one"), identity("job/one")).unwrap(),
        None,
        PrintUncertainty::AcknowledgementUnknown,
    )
    .unwrap();
    let artifact = evidence(event);
    let (plan, advertisement) = arrangement();
    assert_eq!(Some(&plan), artifact.plan.as_ref());
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![HostReport {
            advertisement,
            state: OperationalState::Available,
            capabilities: vec![],
            devices: vec![],
        }],
        bases: vec![],
        lines: vec![],
        plans: vec![plan],
        plays: vec![],
        observations: vec![artifact.sign],
        historical_observations: vec![],
        sealed_boot_provenance: vec![],
        retention: RetentionReport {
            item_capacity: 1,
            retained_items: 1,
            dropped_items: 0,
        },
    };
    let before = serde_json::to_vec(&snapshot).unwrap();
    let report = build_report(&snapshot).unwrap();
    assert_eq!(report.print_events.len(), 1);
    let text = render_text_report(&report);
    assert!(text.contains("Print: delivery uncertain; retry not authorized"));
    assert!(text.contains("AcknowledgementUnknown"));
    assert!(text.contains("fixture/print-protocol@1"));
    assert_eq!(serde_json::to_vec(&snapshot).unwrap(), before);
}
