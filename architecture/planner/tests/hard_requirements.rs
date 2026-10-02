use conduit_core::{kind_id, GearId, ResourceClassId, TIMER_RESOURCE_CLASS};
use conduit_planner::{
    default_placements, plan_with_hard_requirements, HardRealizationRequirements, PlannerError,
};
use conduit_plot::parse_with_startup;
use conduit_signal::signal_profile_catalog;
use conduit_signal_conformance::pico_local_advertisement;
use std::collections::{BTreeMap, BTreeSet};

fn pulse_plot() -> conduit_plot::CheckedPlot {
    parse_with_startup(
        "plot requirements {\n    pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
    .expect("pulse plot checks")
}

fn planning_inputs() -> (
    conduit_plot::CheckedPlot,
    conduit_core::HostAdvertisement,
    conduit_planner::PlacementChoices,
) {
    let plot = pulse_plot();
    let host = pico_local_advertisement();
    let placements = default_placements(&plot, std::slice::from_ref(&host))
        .expect("pulse realization is front-compatible");
    (plot, host, placements)
}

#[test]
fn hard_bounds_reject_before_plan_construction_and_pass_when_satisfied() {
    let (plot, host, placements) = planning_inputs();
    let gear_id = plot.gears[0].gear_id.clone();
    let selected = &host.capabilities[0];
    let requirements = BTreeMap::from([(
        gear_id.clone(),
        HardRealizationRequirements {
            minimum_queue_items: selected.limits.max_queue_items + 1,
            ..HardRealizationRequirements::default()
        },
    )]);
    assert!(matches!(
        plan_with_hard_requirements(
            &plot,
            std::slice::from_ref(&host),
            &placements,
            &[],
            &requirements,
        ),
        Err(PlannerError::HardRealizationRequirementUnsatisfied(_))
    ));

    let satisfied = BTreeMap::from([(
        gear_id,
        HardRealizationRequirements {
            minimum_queue_items: selected.limits.max_queue_items,
            minimum_queue_bytes: selected.limits.max_queue_bytes,
            ..HardRealizationRequirements::default()
        },
    )]);
    plan_with_hard_requirements(
        &plot,
        std::slice::from_ref(&host),
        &placements,
        &[],
        &satisfied,
    )
    .expect("satisfied hard bounds admit the selected realization");
}

#[test]
fn resource_and_effect_allowlists_are_hard_gates_not_rankings() {
    let (plot, host, placements) = planning_inputs();
    let gear_id = plot.gears[0].gear_id.clone();
    let forbidden_timer = BTreeMap::from([(
        gear_id.clone(),
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
            &plot,
            std::slice::from_ref(&host),
            &placements,
            &[],
            &forbidden_timer,
        ),
        Err(PlannerError::HardRealizationRequirementUnsatisfied(_))
    ));

    let no_host_effects = BTreeMap::from([(
        gear_id,
        HardRealizationRequirements {
            permitted_host_calls: Some(BTreeSet::new()),
            ..HardRealizationRequirements::default()
        },
    )]);
    assert!(matches!(
        plan_with_hard_requirements(
            &plot,
            std::slice::from_ref(&host),
            &placements,
            &[],
            &no_host_effects,
        ),
        Err(PlannerError::HardRealizationRequirementUnsatisfied(_))
    ));
}

#[test]
fn checked_front_compatibility_is_evaluated_before_hard_requirements() {
    let (plot, mut host, placements) = planning_inputs();
    host.capabilities[0].outputs[0].value_kind = kind_id("test/different-value");
    let requirements = BTreeMap::from([(
        plot.gears[0].gear_id.clone(),
        HardRealizationRequirements {
            minimum_queue_items: u16::MAX,
            ..HardRealizationRequirements::default()
        },
    )]);
    assert!(matches!(
        plan_with_hard_requirements(&plot, &[host], &placements, &[], &requirements),
        Err(PlannerError::IncompatibleCheckedFront(_))
    ));
}

#[test]
fn requirements_for_an_unknown_operation_fail_closed() {
    let (plot, host, placements) = planning_inputs();
    let requirements = BTreeMap::from([(
        GearId::from("absent"),
        HardRealizationRequirements::default(),
    )]);
    assert!(matches!(
        plan_with_hard_requirements(
            &plot,
            std::slice::from_ref(&host),
            &placements,
            &[],
            &requirements,
        ),
        Err(PlannerError::UnknownGear(gear)) if gear == "absent"
    ));
}
