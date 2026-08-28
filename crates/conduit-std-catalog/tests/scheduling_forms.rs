use conduit_core::{
    kind_id, ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, ImplementationOffer, OfferGeneration, Quantity, QuantityUnit,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    assess_schedule_values, assess_workflow_timing, deterministic_schedule_fixture,
    install_recurrence_catalogs, install_schedule_catalogs, recurrence_occurrence_type,
    scheduled_intent_type, ScheduleRefusal, ScheduleWindowPosition, WorkflowLifecycle,
    WorkflowTimingOutcome, SCHEDULE_ASSESS_KIND, SCHEDULE_FIXTURE_KIND,
};

const SOURCE: &str = include_str!("../../../examples/scheduling-workflow.conduit");

#[test]
fn canonical_forms_reuse_bounded_recurrence_and_consume_workflow_info() {
    let (startup, profile) = catalogs();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();

    let bounded =
        expand_canonical_form_for_authoring(&checked, "bounded-schedule", &profile).unwrap();
    assert_eq!(bounded.expanded.gears.len(), 1);
    assert_eq!(
        bounded.expanded.gears[0].kind_id.as_str(),
        conduit_std_catalog::RECURRENCE_KIND
    );
    let recurrence_host = host(vec![common::recurrence_proof_offer()]);
    let recurrence_placements = conduit_planner::default_expanded_placements(
        &bounded.expanded,
        core::slice::from_ref(&recurrence_host),
    )
    .unwrap();
    conduit_planner::plan_expanded_canonical(
        &bounded.expanded,
        &[recurrence_host],
        &recurrence_placements,
        &[],
    )
    .unwrap();

    let workflow =
        expand_canonical_form_for_authoring(&checked, "workflow-state", &profile).unwrap();
    assert_eq!(workflow.expanded.gears.len(), 2);
    assert_eq!(workflow.output_bindings.len(), 1);
    let workflow_host = host(schedule_conformance_offers(&profile));
    let placements = conduit_planner::default_expanded_placements(
        &workflow.expanded,
        core::slice::from_ref(&workflow_host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &workflow.expanded,
        &[workflow_host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    for kind in [SCHEDULE_FIXTURE_KIND, SCHEDULE_ASSESS_KIND] {
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .unwrap();
        assert!(placement.host_operations.is_empty());
        assert!(placement.resources.is_empty());
        assert!(placement.authority.is_empty());
    }
}

#[test]
fn deterministic_fixture_produces_exact_late_assessment_without_executing() {
    let fixture = deterministic_schedule_fixture().unwrap();
    let assessment =
        assess_schedule_values(&fixture.intent, &fixture.lifecycle, &fixture.observation).unwrap();
    assert_eq!(
        leaf_text(record_field(&assessment, "intent_identity")),
        "schedule/report#0"
    );
    let outcome = record_field(&assessment, "outcome");
    assert_eq!(variant_tag(outcome), "late");
    let StructuredInfoValueShape::Variant { payload, .. } = outcome.shape() else {
        panic!("assessment outcome must be a variant")
    };
    let StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
        panic!("late outcome must retain exact quantity")
    };
    assert_eq!(
        Quantity::decode(bytes).unwrap(),
        Quantity::new(2, QuantityUnit::Second)
    );
}

#[test]
fn lifecycle_timing_outcomes_remain_finite_and_distinct() {
    let zero = Quantity::new(0, QuantityUnit::Millisecond);
    let two = Quantity::new(2, QuantityUnit::Second);
    assert_eq!(
        assess_workflow_timing(
            WorkflowLifecycle::Pending,
            ScheduleWindowPosition::After,
            two,
            zero,
        ),
        Ok(WorkflowTimingOutcome::MissedWindow)
    );
    assert_eq!(
        assess_workflow_timing(
            WorkflowLifecycle::Completed,
            ScheduleWindowPosition::After,
            two,
            zero,
        ),
        Ok(WorkflowTimingOutcome::Late { lateness: two })
    );
    assert_eq!(
        assess_workflow_timing(
            WorkflowLifecycle::Cancelled,
            ScheduleWindowPosition::Within,
            zero,
            zero,
        ),
        Ok(WorkflowTimingOutcome::Cancelled)
    );
    assert_eq!(
        assess_workflow_timing(
            WorkflowLifecycle::Failed,
            ScheduleWindowPosition::Within,
            zero,
            zero,
        ),
        Ok(WorkflowTimingOutcome::Failed)
    );
    assert_eq!(
        assess_workflow_timing(
            WorkflowLifecycle::Expired,
            ScheduleWindowPosition::After,
            two,
            zero,
        ),
        Ok(WorkflowTimingOutcome::Expired)
    );
    assert_eq!(
        assess_workflow_timing(
            WorkflowLifecycle::Running,
            ScheduleWindowPosition::Indeterminate,
            zero,
            two,
        ),
        Ok(WorkflowTimingOutcome::ClockUncertain { uncertainty: two })
    );
    assert_eq!(
        assess_workflow_timing(
            WorkflowLifecycle::Completed,
            ScheduleWindowPosition::Before,
            zero,
            zero,
        ),
        Err(ScheduleRefusal::InconsistentLifecycle)
    );
    assert_eq!(
        assess_workflow_timing(
            WorkflowLifecycle::Pending,
            ScheduleWindowPosition::Within,
            Quantity::new(1, QuantityUnit::Meter),
            zero,
        ),
        Err(ScheduleRefusal::NonTemporalQuantity)
    );
}

#[test]
fn scheduled_intent_reuses_t1_occurrence_and_contains_no_authority_identity() {
    let intent = scheduled_intent_type();
    let StructuredInfoTypeShape::Record { fields, .. } = intent.shape() else {
        panic!("scheduled intent must be a record")
    };
    assert_eq!(
        fields
            .iter()
            .find(|field| field.name() == "occurrence")
            .unwrap()
            .value_type(),
        &recurrence_occurrence_type()
    );
    let rendered = format!("{intent:?}").to_ascii_lowercase();
    for forbidden in [
        "authority",
        "resource",
        "host/",
        "boot/",
        "socket",
        "execute-now",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "scheduled observation leaked {forbidden}"
        );
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_recurrence_catalogs(&mut startup, &mut profile).unwrap();
    install_schedule_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn schedule_conformance_offers(profile: &ProfileCatalog) -> Vec<CapabilityOffer> {
    [SCHEDULE_FIXTURE_KIND, SCHEDULE_ASSESS_KIND]
        .into_iter()
        .map(|kind| {
            let definition = profile.get(&kind_id(kind)).unwrap();
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from(format!("test/schedule-contract/{kind}@1")),
                kind_id: definition.kind_id.clone(),
                kind_contract_revision: definition.kind_contract_revision.clone(),
                implementation: ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from(
                        "test/schedule-contract-fixture@1",
                    ),
                    implementation_id: ImplementationId::from(format!("test/{kind}@1")),
                    artifact_id: ArtifactId::from("conduit-std-catalog/test-schedule-contract@1"),
                },
                inputs: definition.inputs.clone(),
                outputs: definition.outputs.clone(),
                host_operations: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 4,
                    max_queue_items: 4,
                    max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
                },
            }
        })
        .collect()
}

fn host(capabilities: Vec<conduit_core::CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/schedule-proof"),
        boot_id: BootId::from("boot/schedule-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/schedule-contract-fixture@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}

fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn variant_tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("expected variant")
    };
    tag
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected leaf")
    };
    core::str::from_utf8(bytes).unwrap()
}
mod common;
