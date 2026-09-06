use super::*;
use crate::{RunControl, RunControlRequestId};

const CLEAR_FORM: &str = "form prewake_drive {\n intent: robotics/velocity-intent(linear-microunits = 750000, angular-microunits = -250000)\n drive: robotics/drive-differential(ttl-ms = 1000)\n intent.linear > drive.linear\n intent.angular > drive.angular\n}\n";

fn plan(source: &str, id: &str) -> (StdHost, conduit_core::PlanFragment) {
    let host = host(id);
    let form = parse(source, &installed_std::test_catalog()).expect("robot safety Form checks");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("robot placements resolve");
    let fragment = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_robotics::ROBOTICS_ODOMETRY_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("robot safety Form plans with capacity-one Cords")
    .fragments
    .remove(0);
    (host, fragment)
}

fn run(source: &str, id: &str) -> (crate::StdRunReport, String, conduit_core::PlanFragment) {
    let (mut host, fragment) = plan(source, id);
    let mut output = Vec::with_capacity(4_096);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(1),
    };
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("robot safety Form executes through the production kernel");
    assert!(timer.waits.is_empty());
    (
        report,
        String::from_utf8(output).expect("robot proof output is UTF-8"),
        fragment,
    )
}

#[test]
fn portable_drive_meaning_has_one_effect_free_prewake_projection() {
    let (clear, clear_output, clear_fragment) = run(CLEAR_FORM, "robot-clear");
    assert_eq!(clear_fragment.placements.len(), 2);
    assert_eq!(clear_fragment.connections.len(), 2);
    assert!(clear_fragment
        .connections
        .iter()
        .all(|cord| cord.item_capacity == 1 && cord.byte_capacity == 12));
    assert!(clear_fragment
        .placements
        .iter()
        .filter(|placement| placement.kind_id.as_str().starts_with("robotics/"))
        .all(|placement| {
            placement.host_operations.is_empty()
                && placement.resources.is_empty()
                && placement.authority.is_empty()
                && placement.implementation_id.as_str().contains("prewake")
        }));
    assert!(clear_output.contains(
        "PREWAKE simulated drive projection linear-microunits=750000 angular-microunits=-250000 physical-effect=false authority-grant=false"
    ));

    assert!(matches!(
        clear
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = clear.kernel.expect("production kernel report exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn every_robotics_observation_contract_plans_with_distinct_exact_info() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_robotics_catalogs(&mut startup, &mut profile).unwrap();
    let infos = conduit_semantic_catalog::supported_nucleus_contracts()
        .into_iter()
        .filter(|contract| {
            matches!(
                contract.kind_id.as_str(),
                conduit_semantic_catalog::ROBOTICS_OBSERVE_BUMP_KIND
                    | conduit_semantic_catalog::ROBOTICS_OBSERVE_IMU_KIND
                    | conduit_semantic_catalog::ROBOTICS_OBSERVE_RANGE_KIND
                    | conduit_semantic_catalog::ROBOTICS_OBSERVE_ODOMETRY_KIND
                    | conduit_semantic_catalog::ROBOTICS_OBSERVE_BATTERY_KIND
            )
        })
        .map(|contract| contract.outputs[0].value_kind.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(infos.len(), 5);
}

#[test]
fn missing_stale_invalid_cancelled_pressure_and_unavailable_remain_distinct() {
    let missing_source = CLEAR_FORM.replace(
        "form prewake_drive {",
        "form prewake_drive {\n bump: robotics/observe-bump(availability = \"missing\")",
    );
    let stale_source = CLEAR_FORM.replace(
        "form prewake_drive {",
        "form prewake_drive {\n bump: robotics/observe-bump(availability = \"stale\")",
    );
    let missing = run_failure(&missing_source, "robot-missing");
    let stale = run_failure(&stale_source, "robot-stale");
    assert_ne!(missing, stale);
    assert!(missing.contains("OperationFailed(Failure { code: InvalidInput, detail: 40 })"));
    assert!(stale.contains("OperationFailed(Failure { code: InvalidInput, detail: 41 })"));

    for source in [
        "form invalid {\n range: robotics/observe-range(distance-mm = 1000001)\n}\n",
        "form invalid {\n battery: robotics/observe-battery(charge-permille = 1001)\n}\n",
        "form invalid {\n odometry: robotics/observe-odometry(yaw-microradians = 3141594)\n}\n",
        "form invalid {\n drive: robotics/drive-differential(minimum-clearance-mm = 250)\n}\n",
        "form invalid {\n drive: robotics/drive-differential(ttl-ms = 9)\n}\n",
    ] {
        assert!(parse(source, &installed_std::test_catalog()).is_err());
    }

    let (mut cancelled_host, fragment) = plan(CLEAR_FORM, "robot-cancelled");
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("robot-cancel").unwrap())
        .unwrap();
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    let cancelled = cancelled_host
        .run_fragment_controlled_to(fragment, &mut output, &mut timer, &control)
        .expect("admitted cancellation stays machine-readable");
    assert!(matches!(
        cancelled
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));

    let (mutated_host, mut fragment) = plan(CLEAR_FORM, "robot-mutated");
    fragment
        .placements
        .iter_mut()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND
        })
        .unwrap()
        .implementation_id = conduit_core::ImplementationId::from("std/missing-robotics@1");
    let mut mutated_host = mutated_host;
    let error = mutated_host
        .run_fragment_to(
            fragment,
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect_err("unavailable selected implementation refuses before Play");
    assert_eq!(
        error,
        "fragment does not match the installed std kernel profile"
    );

    let baseline = host("robot-pressure");
    let form = parse(CLEAR_FORM, &installed_std::test_catalog()).unwrap();
    let hosts = [baseline.advertisement().clone()];
    let placements = default_placements(&form, &hosts).unwrap();
    assert!(plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 0,
            connection_byte_capacity: 12,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .is_err());
}

fn run_failure(source: &str, id: &str) -> String {
    let (mut host, fragment) = plan(source, id);
    host.run_fragment_to(
        fragment,
        &mut Vec::new(),
        &mut RecordingTimer { waits: Vec::new() },
    )
    .expect_err("missing/stale simulated observation must fail distinctly")
}
