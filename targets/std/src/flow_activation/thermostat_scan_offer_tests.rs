use super::*;
use conduit_core::{port_id, BaseImplementationId, HostAdvertisement, PortDirection};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_activations, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};

use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../../plots/thermostat/main.conduit");

fn authored_with_maximum(
    maximum_items: u16,
) -> (
    conduit_plot::CheckedSyntaxDocument,
    conduit_plot::ExpandedAuthoringPlot,
    ProfileCatalog,
) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_thermostat_plot::install_catalogs(&mut startup, &mut profile).unwrap();
    let source = SOURCE.replace(
        "maximum-items = 256",
        &format!("maximum-items = {maximum_items}"),
    );
    let document = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&document, "thermostat/main", &profile).unwrap();
    (document, authoring, profile)
}

pub(crate) fn authored_thermostat_plan_on_host(
    advertisement: &HostAdvertisement,
    maximum_items: u16,
) -> conduit_core::Plan {
    let (document, authoring, profile) = authored_with_maximum(maximum_items);
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty_bases,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 32,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let bounds = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 3,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("states"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 13,
            },
        ),
    ]);
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    plan_expanded_authoring_with_activations(
        &document,
        &authoring,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &bases,
        options,
        &bounds,
    )
    .expect("exact production Todo scan must plan")
}

#[test]
fn thermostat_scan_has_a_finite_domain_bound() {
    let initial = ThermostatState::default();
    assert!(thermostat_scan_offer(&initial, 256).is_ok());
    assert!(thermostat_scan_offer(&initial, 0).is_err());
    assert!(thermostat_scan_offer(&initial, 257).is_err());
}

fn prepared_reusable_scan() -> (
    conduit_core::Plan,
    conduit_core::PreparedPlan,
    super::super::StdActivationHost,
) {
    let host = crate::StdHost::new_for_thermostat_scan(
        crate::StdHostConfig {
            host_id: conduit_core::HostId::from("reuse-control"),
            boot_id: conduit_core::BootId::from("reuse-control/boot"),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &ThermostatState::default(),
        2,
    )
    .unwrap();
    let plan = authored_thermostat_plan_on_host(host.advertisement(), 2);
    let fragment = &plan.fragments[0];
    let identity = conduit_core::PreparationHostIdentity {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
    };
    let mut owner = super::super::StdActivationHost::new(
        identity,
        super::super::standard_child_registry().unwrap(),
    );
    let prepared = conduit_core::prepare_plan_on_hosts(&plan, &mut [&mut owner]).unwrap();
    (plan, prepared, owner)
}

#[test]
fn reusable_thermostat_handoff_refuses_stale_identity_and_plan_before_consuming() {
    use super::super::install_pure_thermostat_scan;
    let (plan, mut prepared, mut host) = prepared_reusable_scan();
    let conduit_core::PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
        panic!()
    };
    let id = &scan.activation_id;
    let original = host.identity.clone();
    host.identity.boot_id = conduit_core::BootId::from("replacement-boot");
    assert!(install_pure_thermostat_scan(&plan, &mut prepared, id, &mut host).is_err());
    host.identity = original;
    let mut foreign = plan.clone();
    foreign.plan_id = conduit_core::PlanId::from("substituted-plan");
    assert!(install_pure_thermostat_scan(&foreign, &mut prepared, id, &mut host).is_err());
    assert!(install_pure_thermostat_scan(&plan, &mut prepared, id, &mut host).is_ok());
    assert!(install_pure_thermostat_scan(&plan, &mut prepared, id, &mut host).is_err());
}

#[test]
fn reusable_scan_preserves_pending_output_until_acceptance_and_exhausts_exactly() {
    use conduit_composite::{BoundedScanAdmission as Admission, BoundedScanState as State};
    use conduit_thermostat_plot::{Command, Fan};
    let (plan, mut prepared, mut host) = prepared_reusable_scan();
    let conduit_core::PlannedActivationEntry::Scan(planned) = &plan.activations[0] else {
        panic!()
    };
    let mut scan = super::super::install_pure_thermostat_scan(
        &plan,
        &mut prepared,
        &planned.activation_id,
        &mut host,
    )
    .unwrap();
    scan.bind_parent_play(&conduit_core::ActivePlayId::from("parent/reuse-controls"))
        .unwrap();
    let command = |command: Command| conduit_core::ValuePayload {
        value_kind: conduit_core::kind_id(conduit_thermostat_plot::COMMAND_KIND),
        encoded: command.encode().to_vec(),
    };
    let first = command(Command::SetTarget(225));
    let second = command(Command::SetFan(Fan::On));
    let mut wrong = first.clone();
    wrong.value_kind = conduit_core::kind_id("foreign-command");
    assert!(scan.admit(&wrong).is_err());
    assert_eq!(scan.admit(&first).unwrap(), Admission::Accepted);
    assert_eq!(scan.admit(&second).unwrap(), Admission::Accepted);
    assert_eq!(scan.admit(&second).unwrap(), Admission::Full);
    let mut output = conduit_core::ValuePayload {
        value_kind: planned.output.value_kind.clone(),
        encoded: Vec::with_capacity(planned.retained_accumulator_bytes as usize),
    };
    for expected_revision in 1..=2 {
        for _ in 0..16 {
            if scan.step().unwrap() == &State::OutputReady {
                break;
            }
        }
        assert_eq!(scan.step().unwrap(), &State::OutputReady);
        assert!(scan.output_into(&mut output).unwrap());
        let state = ThermostatState::decode(&output.encoded).unwrap();
        assert_eq!(state.target, 225);
        assert_eq!(state.revision, expected_revision);
        assert_eq!(
            state.fan,
            if expected_revision == 1 {
                Fan::Auto
            } else {
                Fan::On
            }
        );
        // A rejected copy and repeated Steps cannot accept or advance this Value.
        assert!(scan.output_into(&mut wrong).is_err());
        assert_eq!(scan.step().unwrap(), &State::OutputReady);
        scan.complete_output().unwrap();
    }
    assert_eq!(
        scan.admit(&first).unwrap(),
        Admission::MaximumItemsExceeded { maximum_items: 2 }
    );
    scan.close_input().unwrap();
    assert_eq!(scan.step().unwrap(), &State::Complete);
    assert_eq!(scan.child_sign_receipts().unwrap().len(), 2);
}
