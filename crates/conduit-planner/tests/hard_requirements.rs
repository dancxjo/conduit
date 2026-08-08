use conduit_core::{kind_id, OperationId, ResourceClassId, TIMER_RESOURCE_CLASS};
use conduit_form::parse;
use conduit_planner::{
    default_placements, plan_with_hard_requirements, HardRealizationRequirements, PlannerError,
};
use conduit_signal::{pico_local_advertisement, signal_profile_catalog};
use std::collections::{BTreeMap, BTreeSet};

fn pulse_form() -> conduit_form::CheckedForm {
    parse(
        "form 0\n\nrequirements {\n    pulse: flow/pulse\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n}\n",
        &signal_profile_catalog(),
    )
    .expect("pulse form checks")
}

fn planning_inputs() -> (
    conduit_form::CheckedForm,
    conduit_core::HostAdvertisement,
    conduit_planner::PlacementChoices,
) {
    let form = pulse_form();
    let host = pico_local_advertisement();
    let placements = default_placements(&form, std::slice::from_ref(&host))
        .expect("pulse realization is face-compatible");
    (form, host, placements)
}

#[test]
fn hard_bounds_reject_before_plan_construction_and_pass_when_satisfied() {
    let (form, host, placements) = planning_inputs();
    let operation_id = form.operations[0].operation_id.clone();
    let selected = &host.capabilities[0];
    let requirements = BTreeMap::from([(
        operation_id.clone(),
        HardRealizationRequirements {
            minimum_queue_items: selected.limits.max_queue_items + 1,
            ..HardRealizationRequirements::default()
        },
    )]);
    assert!(matches!(
        plan_with_hard_requirements(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[],
            &requirements,
        ),
        Err(PlannerError::HardRealizationRequirementUnsatisfied(_))
    ));

    let satisfied = BTreeMap::from([(
        operation_id,
        HardRealizationRequirements {
            minimum_queue_items: selected.limits.max_queue_items,
            minimum_queue_bytes: selected.limits.max_queue_bytes,
            ..HardRealizationRequirements::default()
        },
    )]);
    plan_with_hard_requirements(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[],
        &satisfied,
    )
    .expect("satisfied hard bounds admit the selected realization");
}

#[test]
fn resource_and_effect_allowlists_are_hard_gates_not_rankings() {
    let (form, host, placements) = planning_inputs();
    let operation_id = form.operations[0].operation_id.clone();
    let forbidden_timer = BTreeMap::from([(
        operation_id.clone(),
        HardRealizationRequirements {
            maximum_resource_units: BTreeMap::from([(
                ResourceClassId::from(TIMER_RESOURCE_CLASS),
                0,
            )]),
            ..HardRealizationRequirements::default()
        },
    )]);
    assert!(matches!(
        plan_with_hard_requirements(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[],
            &forbidden_timer,
        ),
        Err(PlannerError::HardRealizationRequirementUnsatisfied(_))
    ));

    let no_host_effects = BTreeMap::from([(
        operation_id,
        HardRealizationRequirements {
            permitted_host_operations: Some(BTreeSet::new()),
            ..HardRealizationRequirements::default()
        },
    )]);
    assert!(matches!(
        plan_with_hard_requirements(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[],
            &no_host_effects,
        ),
        Err(PlannerError::HardRealizationRequirementUnsatisfied(_))
    ));
}

#[test]
fn checked_face_compatibility_is_evaluated_before_hard_requirements() {
    let (form, mut host, placements) = planning_inputs();
    host.capabilities[0].outputs[0].value_kind = kind_id("test/different-value");
    let requirements = BTreeMap::from([(
        form.operations[0].operation_id.clone(),
        HardRealizationRequirements {
            minimum_queue_items: u16::MAX,
            ..HardRealizationRequirements::default()
        },
    )]);
    assert!(matches!(
        plan_with_hard_requirements(&form, &[host], &placements, &[], &requirements),
        Err(PlannerError::IncompatibleCheckedFace(_))
    ));
}

#[test]
fn requirements_for_an_unknown_operation_fail_closed() {
    let (form, host, placements) = planning_inputs();
    let requirements = BTreeMap::from([(
        OperationId::from("absent"),
        HardRealizationRequirements::default(),
    )]);
    assert!(matches!(
        plan_with_hard_requirements(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[],
            &requirements,
        ),
        Err(PlannerError::UnknownOperation(operation)) if operation == "absent"
    ));
}
