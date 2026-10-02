use super::*;
use crate::usb_line_offer::*;
use alloc::{collections::BTreeMap, vec};
use conduit_core::*;
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions, plan_with_options};

fn fixture() -> (
    Plan,
    [HostAdvertisement; 2],
    SessionBinding,
    AdmittedUsbLineBasis,
    UsbLineObservation,
) {
    let host = |name: &str| {
        conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
            host_id: HostId::from(name),
            boot_id: BootId::from(name),
            offer_generation: OfferGeneration(1),
        })
        .advertisement()
        .clone()
    };
    let hosts = [host("actual/source"), host("actual/sink")];
    let observation = UsbLineObservation {
        realization: UsbLineRealization {
            controller_id: [1; 32],
            device_id: [2; 32],
            interface_id: [3; 32],
            input_endpoint_id: [4; 32],
            output_endpoint_id: [5; 32],
            attachment_epoch: 1,
            input_dci: 3,
            output_dci: 4,
            packet_bytes: 64,
            payload_bytes: 62,
            transfer_trbs_per_direction: 128,
        },
        base_instance_id: "base/current".into(),
        state: UsbLineState::Current,
        state_sign_id: "sign/current".into(),
    };
    let offer = offer_usb_ftdi_line(
        UsbLineIdentity {
            line_id: "line/current".into(),
            binding_id: "binding/current".into(),
            base_instance_id: observation.base_instance_id.clone(),
            source_host_id: hosts[0].host_id.clone(),
            source_boot_id: hosts[0].boot_id.clone(),
            source_endpoint_id: "endpoint/source".into(),
            sink_host_id: hosts[1].host_id.clone(),
            sink_boot_id: hosts[1].boot_id.clone(),
            sink_endpoint_id: "endpoint/sink".into(),
        },
        &observation,
    )
    .unwrap();
    let plot = conduit_plot::parse(
        "plot linked {\n pulse: logic/not\n show: logic/not\n pulse.out >> show.in\n}\n",
        &conduit_semantic_catalog::standard_profile_catalog(),
    )
    .unwrap();
    let capability = |host: &HostAdvertisement, kind: &str| {
        host.capabilities
            .iter()
            .find(|o| o.kind_id.as_str() == kind)
            .unwrap()
            .capability_id
            .clone()
    };
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("linked/pulse"),
                PlacementChoice {
                    host_id: hosts[0].host_id.clone(),
                    capability_id: capability(&hosts[0], "logic/not"),
                },
            ),
            (
                GearId::from("linked/show"),
                PlacementChoice {
                    host_id: hosts[1].host_id.clone(),
                    capability_id: capability(&hosts[1], "logic/not"),
                },
            ),
        ]),
    };
    let plan = plan_with_options(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from(USB_FTDI_BASE)],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::from([(
                (GearId::from("linked/pulse"), GearId::from("linked/show")),
                vec![offer.line_id.clone()],
            )]),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: core::slice::from_ref(&offer),
        },
    )
    .unwrap();
    let source = plan
        .fragments
        .iter()
        .find(|p| p.host_id == hosts[0].host_id)
        .unwrap();
    let sink = plan
        .fragments
        .iter()
        .find(|p| p.host_id == hosts[1].host_id)
        .unwrap();
    let binding = SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        &source.connections[0],
    )
    .unwrap();
    let basis = AdmittedUsbLineBasis::from_offer(&offer, &observation).unwrap();
    (plan, hosts, binding, basis, observation)
}
#[test]
fn exact_planner_output_accepts_both_roles_without_harness_names() {
    let (plan, hosts, binding, basis, observation) = fixture();
    for (host, role) in [
        (&hosts[0], SessionRole::Source),
        (&hosts[1], SessionRole::Sink),
    ] {
        let admitted =
            PlannedUsbLine::admit(&plan, host, &binding, &basis, &observation, role).unwrap();
        assert_eq!(admitted.binding(), &binding);
        assert!(!admitted.session.is_active());
    }
}
#[test]
fn stale_host_generation_role_and_binding_refuse() {
    let (plan, hosts, binding, basis, observation) = fixture();
    let check = |host: &HostAdvertisement, binding: &SessionBinding| {
        PlannedUsbLine::admit(
            &plan,
            host,
            binding,
            &basis,
            &observation,
            SessionRole::Source,
        )
        .err()
    };
    let mut stale = hosts[0].clone();
    stale.offer_generation.0 += 1;
    assert_eq!(check(&stale, &binding), Some(PlannedLineError::StaleHost));
    stale = hosts[0].clone();
    stale.boot_id = "boot/stale".into();
    assert_eq!(check(&stale, &binding), Some(PlannedLineError::StaleHost));
    assert_eq!(
        check(&hosts[1], &binding),
        Some(PlannedLineError::StaleHost)
    );
    let mut changed = binding.clone();
    changed.sink_active_play_id = "play/invented".into();
    assert_eq!(check(&hosts[0], &changed), Some(PlannedLineError::Binding));
    changed = binding.clone();
    changed.attachment.sink_endpoint_id = "endpoint/substituted".into();
    assert_eq!(check(&hosts[0], &changed), Some(PlannedLineError::Binding));
}
#[test]
fn seal_substitution_loss_and_physical_replacement_refuse() {
    let (mut plan, hosts, binding, basis, observation) = fixture();
    let mut lost = observation.clone();
    lost.state = UsbLineState::Lost;
    assert_eq!(
        PlannedUsbLine::admit(
            &plan,
            &hosts[0],
            &binding,
            &basis,
            &lost,
            SessionRole::Source
        )
        .err(),
        Some(PlannedLineError::Observation(UsbLineOfferError::Lost))
    );
    let mut replaced = observation.clone();
    replaced.realization.attachment_epoch += 1;
    assert_eq!(
        PlannedUsbLine::admit(
            &plan,
            &hosts[0],
            &binding,
            &basis,
            &replaced,
            SessionRole::Source
        )
        .err(),
        Some(PlannedLineError::Observation(
            UsbLineOfferError::RealizationMismatch
        ))
    );
    plan.fragments[0].offer_generation.0 += 1;
    assert_eq!(
        PlannedUsbLine::admit(
            &plan,
            &hosts[0],
            &binding,
            &basis,
            &observation,
            SessionRole::Source
        )
        .err(),
        Some(PlannedLineError::Plan)
    );
}
