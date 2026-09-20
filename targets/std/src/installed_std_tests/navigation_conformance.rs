use super::{installed_std, RecordingTimer};
use crate::{StdHost, StdHostConfig};
use conduit_core::{
    BaseImplementationId, BootId, HostId, ObservationKind, OfferGeneration, PortTemporal,
    TerminalDisposition,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog, StartupCatalog,
};
use std::collections::BTreeMap;

const FORM: &str = r#"form navigation_play {
 pose: conduit-test/navigation-pose-source
 goal: conduit-test/navigation-goal-source
 grid: conduit-test/navigation-grid-source
 time: conduit-test/navigation-time-source
 route: navigation/route-grid4
 timing: navigation/time-parameterize
 controller: navigation/local-control
 sink: conduit-test/local-model-result
 pose.value > route.pose
 goal.value > route.goal
 grid.value > route.traversability
 time.value > route.time
 route.decision > select(NavigationRouteDecision.route, unmatched=drop) > timing.route
 time.value > timing.time
 timing.trajectory > controller.trajectory
 pose.value > controller.pose
 time.value > controller.time
 controller.control > select(NavigationControl.motion, unmatched=drop) > sink.value
}
"#;

#[test]
fn portable_navigation_executes_as_one_bounded_production_play() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_robotics_structured_catalogs(&mut startup, &mut profiles)
        .unwrap();
    conduit_semantic_catalog::install_navigation_catalogs(&mut startup, &mut profiles).unwrap();
    installed_std::test_local_model_io::install_navigation_catalog(&mut startup, &mut profiles);
    let checked = check_syntax_document(&parse_syntax_document(FORM), &startup).unwrap();
    let selector_specs = checked
        .forms
        .iter()
        .flat_map(|form| &form.cords)
        .flat_map(|cord| &cord.stages)
        .filter_map(|stage| match stage {
            CheckedCordStage::StructuredSelector { selector, .. } => Some(selector),
            _ => None,
        })
        .cloned()
        .collect::<Vec<_>>();
    for selector in &selector_specs {
        profiles
            .insert(structured_selector_definition(
                selector,
                PortTemporal::Value,
            ))
            .unwrap();
    }
    let expanded = expand_canonical_form(&checked, "navigation_play", &profiles).unwrap();

    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/navigation-kernel"),
        boot_id: BootId::from("boot/navigation-kernel/1"),
        offer_generation: OfferGeneration(1),
    });
    host.advertisement
        .capabilities
        .extend(installed_std::test_local_model_io::navigation_source_offers());
    host.advertisement
        .capabilities
        .extend(selector_specs.iter().map(|selector| {
            conduit_std_offers::structured_selector_std_offer(selector, PortTemporal::Value)
        }));
    let motion = conduit_robotics::robotics_motion_request_type();
    host.advertisement
        .capabilities
        .push(installed_std::test_local_model_io::sink_offer(
            motion.profile().unwrap().value_kind().as_str(),
        ));
    host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4_096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    for implementation in [
        conduit_std_offers::NAVIGATION_ROUTE_GRID4_IMPLEMENTATION,
        conduit_std_offers::NAVIGATION_TIME_PARAMETERIZE_IMPLEMENTATION,
        conduit_std_offers::NAVIGATION_LOCAL_CONTROL_IMPLEMENTATION,
    ] {
        assert!(plan.fragments[0]
            .placements
            .iter()
            .any(|placement| placement.implementation_id.as_str() == implementation));
    }

    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect("portable navigation runs through the production kernel");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostOperationCompleted)
            .count(),
        11
    );
    assert!(timer.waits.is_empty());
}
