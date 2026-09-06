use super::*;
use crate::current_device_for_capability;
use conduit_core::{
    BaseImplementationId, BootId, DeviceAssociation, DeviceId, DeviceIdentityEvidence,
    DeviceIdentityFact, DeviceIdentityStrength, DeviceTruthDisposition, HostId, LineAvailability,
    Observation, ObservationKind, SignId, PROTOCOL_VERSION,
};
use conduit_observatory::{
    CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport, LineReport,
    ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport, SNAPSHOT_SCHEMA,
};

fn host_report(
    advertisement: conduit_core::HostAdvertisement,
    state: OperationalState,
) -> HostReport {
    let capabilities = advertisement
        .capabilities
        .iter()
        .map(|offer| CapabilityStatusReport {
            capability_id: offer.capability_id.clone(),
            freshness: if state == OperationalState::Stale {
                OfferFreshness::Stale
            } else {
                OfferFreshness::Fresh
            },
            support: CapabilitySupport::Supported,
            availability: if state == OperationalState::Available {
                CapabilityAvailability::Available
            } else {
                CapabilityAvailability::Unavailable
            },
        })
        .collect();
    HostReport {
        advertisement,
        state,
        capabilities,
        devices: Vec::new(),
    }
}

fn fleet_snapshot(link_available: bool) -> ObservatorySnapshot {
    let exact = conduit_signal_conformance::triple::exact_plan().unwrap();
    let laptop = exact.source_advertisement;
    let browser_new = exact.browser_advertisement;
    let mut browser_old = browser_new.clone();
    browser_old.boot_id = BootId::from("s4/triple-browser-old-boot");
    let pico = exact.pico_advertisement;
    let websocket = exact.browser_line;
    let mut usb = exact.pico_line;
    if !link_available {
        usb.availability.availability = LineAvailability::Unavailable;
    }
    let observations = vec![Observation {
        sign_id: SignId::from("fleet/sign-1"),
        active_play_id: None,
        presentation_id: None,
        host_id: laptop.host_id.clone(),
        boot_id: laptop.boot_id.clone(),
        plan_id: None,
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::AdvertisementPublished,
    }];
    ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![
            host_report(laptop, OperationalState::Available),
            host_report(browser_old, OperationalState::Stale),
            host_report(browser_new, OperationalState::Available),
            host_report(pico, OperationalState::Available),
        ],
        bases: Vec::new(),
        lines: vec![
            LineReport {
                offer: websocket,
                state: OperationalState::Available,
            },
            LineReport {
                offer: usb,
                state: if link_available {
                    OperationalState::Available
                } else {
                    OperationalState::Unreachable
                },
            },
        ],
        plans: vec![exact.plan],
        plays: Vec::new(),
        retention: RetentionReport {
            item_capacity: 2,
            retained_items: 1,
            dropped_items: 3,
        },
        observations,
        historical_observations: Vec::new(),
        sealed_boot_provenance: Vec::new(),
    }
}

#[test]
fn bounded_fleet_view_keeps_exact_boot_capability_resource_line_and_gap_facts() {
    let mut topology = PatchbayTopology::new(2).unwrap();
    topology.ingest(&fleet_snapshot(true)).unwrap();
    let document = topology.document(None).unwrap().lines().join("\n");

    assert!(document.contains("BOOT s4/triple-browser-old-boot state=Stale"));
    assert!(document.contains("BOOT s4/triple-browser-boot state=Available"));
    assert!(document.contains("OFFER"));
    assert!(document.contains("RESOURCE"));
    assert!(document.contains("base=conduit.base/websocket-rfc6455@1"));
    assert!(document.contains("base=conduit.base/usb-cdc-acm@1"));
    assert!(document.contains("visible_gaps=3"));
}

#[test]
fn capability_inspection_descends_to_optional_device_provenance() {
    let mut snapshot = fleet_snapshot(true);
    let advertisement = &snapshot.hosts[0].advertisement;
    let capability_id = advertisement.capabilities[0].capability_id.clone();
    let host_id = advertisement.host_id.clone();
    let boot_id = advertisement.boot_id.clone();
    let offer_generation = advertisement.offer_generation;
    snapshot.hosts[0].devices = vec![DeviceAssociation {
        protocol_version: PROTOCOL_VERSION,
        device_id: DeviceId::from("device/provider-one"),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        offer_generation,
        disposition: DeviceTruthDisposition::Current,
        capability_ids: vec![capability_id.clone()],
        resources: vec![],
        identity_evidence: DeviceIdentityEvidence {
            strength: DeviceIdentityStrength::ProviderAsserted,
            provider: BaseImplementationId::from("fixture/provider@1"),
            facts: vec![DeviceIdentityFact {
                name: "provider-subject".into(),
                value: "one".into(),
            }],
        },
    }];

    let mut topology = PatchbayTopology::new(1).unwrap();
    topology.ingest(&snapshot).unwrap();
    let report = topology.current_report().unwrap();
    let association =
        current_device_for_capability(report, &host_id, &boot_id, &capability_id).unwrap();
    assert_eq!(association.device_id.as_str(), "device/provider-one");
    let lines = topology.document(None).unwrap().lines().join("\n");
    assert!(lines.contains("DEVICE device/provider-one"));
    assert!(lines.contains("DEVICE not identified"));
    let placement = report
        .placements
        .iter()
        .find(|placement| {
            placement.host_id == host_id
                && placement.boot_id == boot_id
                && placement.capability_id == capability_id
        })
        .unwrap();
    assert_eq!(placement.offer_generation, offer_generation);
    assert!(crate::topology_hosts::current_device_for_placement(report, placement).is_some());
    assert!(lines.contains("SELECTED DEVICE device/provider-one"));
    let mut changed = report.clone();
    changed.devices[0].association.offer_generation =
        conduit_core::OfferGeneration(offer_generation.0 + 1);
    assert!(crate::topology_hosts::current_device_for_placement(&changed, placement).is_none());
    changed.devices[0].association.offer_generation = offer_generation;
    changed.devices[0].association.disposition = DeviceTruthDisposition::HistoricalLost {
        terminal_sign_id: None,
    };
    assert!(crate::topology_hosts::current_device_for_placement(&changed, placement).is_none());
    changed.devices[0].association.disposition = DeviceTruthDisposition::Current;
    changed.devices[0].association.boot_id = BootId::from("different-boot");
    assert!(crate::topology_hosts::current_device_for_placement(&changed, placement).is_none());
    let sealed_plans = snapshot.plans.clone();
    let newer = conduit_core::OfferGeneration(offer_generation.0 + 1);
    snapshot.hosts[0].advertisement.offer_generation = newer;
    snapshot.hosts[0].devices[0].offer_generation = newer;
    topology.ingest(&snapshot).unwrap();
    let updated = topology.document(None).unwrap().lines().join("\n");
    assert!(updated.contains("DEVICE device/provider-one"));
    assert!(!updated.contains("SELECTED DEVICE device/provider-one"));
    assert_eq!(snapshot.plans, sealed_plans);
}

#[test]
fn line_update_changes_only_reported_availability_and_retention_is_visible() {
    let mut topology = PatchbayTopology::new(2).unwrap();
    let available = fleet_snapshot(true);
    let unavailable = fleet_snapshot(false);
    let plan_before = available.plans[0].plan_id.clone();
    topology.ingest(&available).unwrap();
    topology.ingest(&unavailable).unwrap();
    topology.ingest(&available).unwrap();

    assert_eq!(topology.retained_reports(), 2);
    assert_eq!(topology.dropped_reports(), 1);
    let current = topology.current_report().unwrap();
    assert_eq!(current.plans[0].plan_id, plan_before);
    assert_eq!(current.hosts[0].host_id, HostId::from("s4/triple-std"));
    let document = topology.document(None).unwrap().lines().join("\n");
    assert!(document.contains("retained=2 capacity=2 dropped=1"));
    assert!(document.contains("availability=Ready"));
}

#[test]
fn portable_planner_offers_are_visible_only_when_reported() {
    let model = crate::PatchbayModel::with_identity(
        HostId::from("patchbay-planner-host"),
        BootId::from("patchbay-planner-boot"),
    );
    let mut topology = PatchbayTopology::new(1).unwrap();
    topology.ingest(&model.startup_snapshot()).unwrap();
    let document = topology.document(None).unwrap().lines().join("\n");

    assert!(document.contains("PLANNER conduit.planner/full@1"));
    assert_eq!(
        topology.current_report().unwrap().hosts[0]
            .planner_capabilities
            .len(),
        1
    );
}

#[test]
fn absent_reports_and_presentation_controls_cannot_invent_facts() {
    let topology = PatchbayTopology::new(1).unwrap();
    let empty = topology.document(Some("invented")).unwrap();
    assert_eq!(
        empty.lines(),
        &[
            "CURRENT REPORTS retained=0 capacity=1 dropped=0".to_string(),
            "HOSTS none reported".to_string(),
            "LINES none reported".to_string(),
        ]
    );

    let mut topology = PatchbayTopology::new(1).unwrap();
    topology.ingest(&fleet_snapshot(true)).unwrap();
    let before = topology.current_report().unwrap().clone();
    let filtered = topology.document(Some("pico")).unwrap();
    assert!(filtered
        .lines()
        .iter()
        .all(|line| line.starts_with("CURRENT REPORTS") || line.contains("pico")));
    assert_eq!(&before, topology.current_report().unwrap());
    assert!(!filtered
        .lines()
        .iter()
        .any(|line| line.contains("invented")));
}

#[test]
fn invalid_input_does_not_replace_last_valid_report() {
    let mut topology = PatchbayTopology::new(1).unwrap();
    topology.ingest(&fleet_snapshot(true)).unwrap();
    let before = topology.current_report().unwrap().clone();
    let mut invalid = fleet_snapshot(true);
    invalid.schema = "unknown".into();

    assert!(matches!(
        topology.ingest(&invalid),
        Err(TopologyViewError::InvalidReport(_))
    ));
    assert_eq!(&before, topology.current_report().unwrap());
}

#[test]
fn oversized_report_is_rejected_before_retained_history() {
    let model = crate::PatchbayModel::with_identity(
        HostId::from("bounded-host"),
        BootId::from("bounded-boot"),
    );
    let mut hosts = Vec::new();
    for index in 0..130 {
        let mut advertisement = model.advertisement().clone();
        advertisement.host_id = HostId::from(format!("host-{index}"));
        advertisement.boot_id = BootId::from(format!("boot-{index}"));
        hosts.push(host_report(advertisement, OperationalState::Available));
    }
    let oversized = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts,
        bases: Vec::new(),
        lines: Vec::new(),
        plans: Vec::new(),
        plays: Vec::new(),
        observations: Vec::new(),
        historical_observations: Vec::new(),
        sealed_boot_provenance: Vec::new(),
        retention: RetentionReport {
            item_capacity: 0,
            retained_items: 0,
            dropped_items: 0,
        },
    };
    let mut topology = PatchbayTopology::new(1).unwrap();

    assert_eq!(
        topology.ingest(&oversized),
        Err(TopologyViewError::ReportTooLarge)
    );
    assert_eq!(topology.retained_reports(), 0);
}
