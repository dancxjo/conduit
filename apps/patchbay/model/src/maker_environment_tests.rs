use conduit_core::{
    BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration, PROTOCOL_VERSION,
};
use conduit_observatory::{
    HostReport, ObservatorySnapshot, OperationalState, RetentionReport, SNAPSHOT_SCHEMA,
};

use crate::{
    AuthoredEnvironment, AuthoredEnvironmentError, AuthoredLink, AuthoredPart,
    EnvironmentComparisonRow, EnvironmentLinkKind, MachineProfile, ObservedPartBinding,
};

fn pete_environment() -> AuthoredEnvironment {
    let mut environment = AuthoredEnvironment::new("pete-workbench").unwrap();
    for (id, name, profile, x) in [
        ("pico", "Pico W", MachineProfile::PicoW, -300),
        ("motherbrain", "RPi 5", MachineProfile::RaspberryPi5, 0),
        ("forebrain", "Laptop", MachineProfile::LaptopLinux, 300),
    ] {
        let mut part = AuthoredPart::reviewed(id, name, profile);
        part.x = x;
        environment.add_part(part).unwrap();
    }
    environment
        .add_link(AuthoredLink {
            link_id: "pico-wifi-motherbrain".into(),
            left_part_id: "pico".into(),
            right_part_id: "motherbrain".into(),
            kind: EnvironmentLinkKind::Wifi,
        })
        .unwrap();
    environment
}

#[test]
fn authored_environment_round_trips_and_projects_only_simulation_candidates() {
    let mut environment = pete_environment();
    environment
        .rename_part("forebrain", "Workshop laptop".into())
        .unwrap();
    environment.move_part("pico", -240, 20).unwrap();
    environment.validate().unwrap();

    let encoded = serde_json::to_vec(&environment).unwrap();
    let reopened: AuthoredEnvironment = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(environment, reopened);
    assert_eq!(encoded, serde_json::to_vec(&reopened).unwrap());

    let projection = reopened.simulation_projection().unwrap();
    assert_eq!(projection.hosts.len(), 3);
    assert_eq!(
        projection.provenance.proof_class,
        "authored-environment-simulation"
    );
    assert!(!projection.provenance.observed_live_truth);
    assert!(!projection.provenance.physical_evidence);
    assert!(!projection.provenance.authority_granted);
    assert!(projection.hosts.iter().all(|host| {
        host.host_id
            .starts_with("simulation/environment/pete-workbench/")
            && host
                .boot_id
                .contains(&format!("/revision/{}/", reopened.revision))
    }));

    environment.remove_part("pico").unwrap();
    assert_eq!(environment.parts.len(), 2);
    assert!(environment.links.is_empty());
}

#[test]
fn malformed_duplicate_overflow_and_unsupported_environment_facts_refuse() {
    let mut environment = pete_environment();
    let duplicate = AuthoredPart::reviewed("pico", "Other", MachineProfile::PicoW);
    assert_eq!(
        environment.add_part(duplicate),
        Err(AuthoredEnvironmentError::DuplicatePart)
    );

    environment.parts[0].resources.memory_bytes = u64::MAX;
    assert_eq!(
        environment.validate(),
        Err(AuthoredEnvironmentError::InvalidResources)
    );
    environment.parts[0].resources = MachineProfile::PicoW.reviewed_resources();
    environment.version = 2;
    assert_eq!(
        environment.validate(),
        Err(AuthoredEnvironmentError::WrongVersion)
    );
    environment.version = 1;
    environment.parts[0].x = i32::MAX;
    assert_eq!(
        environment.validate(),
        Err(AuthoredEnvironmentError::CoordinateOutOfBounds)
    );
    environment.parts[0].x = 0;
    environment.parts[0].x = i32::MIN;
    assert_eq!(
        environment.validate(),
        Err(AuthoredEnvironmentError::CoordinateOutOfBounds)
    );
    environment.parts[0].x = 0;

    let mut too_many_parts = AuthoredEnvironment::new("overflow-parts").unwrap();
    for index in 0..crate::MAX_AUTHORED_PARTS {
        too_many_parts
            .add_part(AuthoredPart::reviewed(
                format!("part-{index}"),
                format!("Part {index}"),
                MachineProfile::LaptopLinux,
            ))
            .unwrap();
    }
    assert_eq!(
        too_many_parts.add_part(AuthoredPart::reviewed(
            "one-too-many",
            "One too many",
            MachineProfile::LaptopLinux,
        )),
        Err(AuthoredEnvironmentError::TooManyParts)
    );

    let unsupported = AuthoredLink {
        link_id: "pico-ethernet-laptop".into(),
        left_part_id: "pico".into(),
        right_part_id: "forebrain".into(),
        kind: EnvironmentLinkKind::Ethernet,
    };
    assert_eq!(
        environment.add_link(unsupported),
        Err(AuthoredEnvironmentError::UnsupportedLink)
    );
}

#[test]
fn observed_comparison_retains_modeled_observed_matching_and_discrepant_truth() {
    let environment = pete_environment();
    let snapshot = snapshot(&[
        ("live-pico", "boot-pico", "conduit-pico-w"),
        ("live-rpi", "boot-rpi", "unexpected-rpi-profile"),
        ("unmodeled", "boot-extra", "other-host"),
    ]);
    let comparison = environment
        .compare_observed(
            &snapshot,
            &[
                ObservedPartBinding {
                    part_id: "pico".into(),
                    host_id: HostId::from("live-pico"),
                },
                ObservedPartBinding {
                    part_id: "motherbrain".into(),
                    host_id: HostId::from("live-rpi"),
                },
            ],
        )
        .unwrap();
    assert!(comparison.rows.iter().any(
        |row| matches!(row, EnvironmentComparisonRow::Matching { part_id, .. } if part_id == "pico")
    ));
    assert!(comparison.rows.iter().any(|row| matches!(row, EnvironmentComparisonRow::Discrepant { part_id, .. } if part_id == "motherbrain")));
    assert!(comparison.rows.iter().any(|row| matches!(row, EnvironmentComparisonRow::ModeledOnly { part_id, .. } if part_id == "forebrain")));
    assert!(comparison.rows.iter().any(|row| matches!(row, EnvironmentComparisonRow::ObservedOnly { host_id, .. } if host_id.as_str() == "unmodeled")));
    assert_eq!(environment, pete_environment());
}

fn snapshot(hosts: &[(&str, &str, &str)]) -> ObservatorySnapshot {
    ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: hosts
            .iter()
            .map(|(host, boot, profile)| HostReport {
                advertisement: HostAdvertisement {
                    protocol_version: PROTOCOL_VERSION,
                    host_id: HostId::from(*host),
                    boot_id: BootId::from(*boot),
                    profile: HostProfileId::from(*profile),
                    offer_generation: OfferGeneration(1),
                    capabilities: Vec::new(),
                    planner_capabilities: Vec::new(),
                    resources: Vec::new(),
                },
                state: OperationalState::Available,
                capabilities: Vec::new(),
            })
            .collect(),
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
    }
}
