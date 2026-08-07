use alloc::collections::BTreeMap;
use alloc::vec;

use super::{
    build_report, render_text_report, unsupported_state, CapabilityAvailability, CapabilitySupport,
    OfferFreshness, OperationalState, PlanLifecycle,
};
use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
use conduit_core::{
    authority_grant, present_authority_requirement, process_owned_link_binding, BootId,
    CapabilityId, ConnectionProvider, HostCommand, HostId, ObservationKind, OfferGeneration,
    OperationId, TerminalDisposition,
};
use conduit_form::parse;
use conduit_pico_sim::{pico_advertisement, PicoSimConfig};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
use conduit_realm::{AdmissionRequest, LinkId, Realm, RealmId};
use conduit_signal::signal_profile_catalog;
use conduit_std_host::{LegacyStdFixtureHost, StdHostConfig};

#[test]
fn report_separates_identity_capability_plan_connection_and_evidence_tables() {
    let mut std_host = LegacyStdFixtureHost::new_with_config(StdHostConfig {
        host_id: HostId::from("std-host-triple"),
        boot_id: BootId::from("std-boot-triple"),
        offer_generation: OfferGeneration(1),
    });
    let page = BrowserSimPage::with_hosts([BrowserSimConfig {
        host_id: HostId::from("browser-sim-triple"),
        boot_id: BootId::from("browser-sim-boot-triple"),
        offer_generation: OfferGeneration(1),
    }]);
    let pico_ad = pico_advertisement(PicoSimConfig {
        host_id: HostId::from("pico-sim-triple"),
        boot_id: BootId::from("pico-sim-boot-triple"),
        offer_generation: OfferGeneration(1),
    });
    let mut browser_ad = page
        .advertisements()
        .into_iter()
        .next()
        .expect("browser advertisement exists");
    let browser_host_id = browser_ad.host_id.clone();
    let browser_boot_id = browser_ad.boot_id.clone();
    let browser_capability = browser_ad
        .capabilities
        .iter_mut()
        .find(|capability| capability.capability_id == CapabilityId::from("dom-show"))
        .expect("browser presentation capability exists");
    let presentation_subject = browser_capability
        .host_operations
        .iter()
        .find_map(|requirement| requirement.target_kind.clone())
        .expect("browser presentation declares a target subject");
    let authority_requirement = present_authority_requirement(presentation_subject);
    browser_capability
        .authority_requirements
        .push(authority_requirement.clone());
    let browser_authority_grant = authority_grant(
        "grant/browser-presentation",
        &authority_requirement,
        browser_host_id,
        browser_boot_id,
        browser_capability.capability_id.clone(),
    );
    let advertisements = vec![
        std_host.advertisement().clone(),
        browser_ad.clone(),
        pico_ad.clone(),
    ];

    let mut realm = Realm::found(
        RealmId::from("realm-m3"),
        advertisements[0].clone(),
        LinkId::from("link-std"),
        16,
    );
    realm
        .admit(AdmissionRequest {
            advertisement: browser_ad.clone(),
            link_id: LinkId::from("link-browser"),
            allow: true,
        })
        .expect("browser joins realm");
    realm
        .admit(AdmissionRequest {
            advertisement: pico_ad.clone(),
            link_id: LinkId::from("link-pico"),
            allow: true,
        })
        .expect("pico joins realm");
    let realm_view = realm
        .view_for(&HostId::from("std-host-triple"))
        .expect("std host observes realm");

    let form = parse(
        include_str!("../../../examples/triple-signal.form"),
        &signal_profile_catalog(),
    )
    .expect("triple form parses");
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("pulse"),
                PlacementChoice {
                    host_id: HostId::from("std-host-triple"),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                OperationId::from("local"),
                PlacementChoice {
                    host_id: HostId::from("std-host-triple"),
                    capability_id: CapabilityId::from("stdout-show-1"),
                },
            ),
            (
                OperationId::from("web"),
                PlacementChoice {
                    host_id: HostId::from("browser-sim-triple"),
                    capability_id: CapabilityId::from("dom-show"),
                },
            ),
            (
                OperationId::from("light"),
                PlacementChoice {
                    host_id: HostId::from("pico-sim-triple"),
                    capability_id: CapabilityId::from("onboard-led"),
                },
            ),
        ]),
    };
    let connection_providers = BTreeMap::from([
        (
            (OperationId::from("pulse"), OperationId::from("local")),
            ConnectionProvider::Local,
        ),
        (
            (OperationId::from("pulse"), OperationId::from("web")),
            ConnectionProvider::FixtureFrame,
        ),
        (
            (OperationId::from("pulse"), OperationId::from("light")),
            ConnectionProvider::FixtureDatagram,
        ),
    ]);
    let link_bindings = vec![
        process_owned_link_binding(
            "link/std-browser",
            ConnectionProvider::FixtureFrame,
            "fixture/frame/std-browser",
            &advertisements[0],
            &advertisements[1],
            4,
            64,
        ),
        process_owned_link_binding(
            "link/std-pico",
            ConnectionProvider::FixtureDatagram,
            "fixture/datagram/std-pico",
            &advertisements[0],
            &advertisements[2],
            4,
            64,
        ),
    ];
    let plan = plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[
            ConnectionProvider::Local,
            ConnectionProvider::FixtureFrame,
            ConnectionProvider::FixtureDatagram,
        ],
        PlanningOptions {
            connection_providers: &connection_providers,
            connection_item_capacity: 4,
            connection_byte_capacity: 64,
            authority_grants: core::slice::from_ref(&browser_authority_grant),
            link_bindings: &link_bindings,
        },
    )
    .expect("M1 triple-simulation plan resolves");
    std_host.replace_link_bindings(link_bindings);
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == HostId::from("std-host-triple"))
        .expect("std fragment exists")
        .clone();
    let _ = std_host.handle(HostCommand::Prepare(fragment.clone()));
    let _ = std_host.handle(HostCommand::Activate(fragment.plan_id.clone()));
    let observations = std_host
        .handle(HostCommand::Inspect)
        .events
        .into_iter()
        .find_map(|event| match event {
            conduit_core::HostEvent::Observations { items } => Some(items),
            _ => None,
        })
        .expect("inspect returns observations");

    let report = build_report(&advertisements, Some(&realm_view), &[plan], &observations);
    assert_eq!(report.hosts.len(), 3);
    assert_eq!(report.links.len(), 3);
    assert!(report
        .hosts
        .iter()
        .all(|host| host.realm_id == Some(RealmId::from("realm-m3"))));
    assert!(report.hosts.iter().all(|host| host.boot_id.is_some()));
    assert!(report.capabilities.iter().all(|capability| {
        capability.support == CapabilitySupport::Supported
            && capability.availability == CapabilityAvailability::Available
            && capability.freshness == OfferFreshness::Fresh
    }));
    assert_eq!(report.plans.len(), 1);
    assert_eq!(report.plans[0].placement_count, 4);
    assert_eq!(report.plans[0].connection_count, 3);
    assert_eq!(report.placements.len(), 4);
    assert!(report.capabilities.iter().any(|capability| {
        capability.capability_id == CapabilityId::from("dom-show")
            && capability.authority_requirements == vec![authority_requirement.clone()]
    }));
    assert!(report.placements.iter().any(|placement| {
        placement.capability_id == CapabilityId::from("dom-show")
            && placement.authority.len() == 1
            && placement.authority[0].grant_id == browser_authority_grant.grant_id
    }));
    assert_eq!(report.connections.len(), 3);
    assert!(report
        .connections
        .iter()
        .any(|connection| connection.provider == ConnectionProvider::FixtureFrame));
    assert!(report
        .connections
        .iter()
        .any(|connection| connection.provider == ConnectionProvider::FixtureDatagram));
    assert!(report.connections.iter().all(|connection| {
        (connection.provider == ConnectionProvider::Local && connection.link_binding.is_none())
            || (connection.provider != ConnectionProvider::Local
                && connection.link_binding.is_some())
    }));
    assert!(report
        .evidence
        .iter()
        .all(|row| !row.evidence_id.as_str().is_empty()));
    assert!(report.evidence.iter().any(|row| {
        row.plan_id == Some(report.plans[0].plan_id.clone())
            && row.active_play_id.is_some()
            && matches!(row.kind, ObservationKind::PlanActivated)
    }));
    assert!(report.retention.bounded);

    let rendered = render_text_report(&report);
    assert!(rendered.contains("host observatory report"));
    assert!(rendered.contains("host id=std-host-triple boot=std-boot-triple"));
    assert!(rendered.contains("capability host=browser-sim-triple"));
    assert!(rendered.contains(conduit_core::PRESENT_HOST_OPERATION_CONTRACT));
    assert!(rendered.contains("presentation/signal"));
    assert!(rendered.contains(conduit_core::PRESENTATION_RESOURCE_CLASS));
    assert!(rendered.contains("browser/presentation"));
    assert!(rendered.contains(conduit_core::PRESENT_AUTHORITY_CONTRACT));
    assert!(rendered.contains("grant/browser-presentation"));
    assert!(rendered.contains("provider=FixtureFrame"));
    assert!(rendered.contains("provider=FixtureDatagram"));
    assert!(rendered.contains("link/std-browser"));
    assert!(rendered.contains("fixture/datagram/std-pico"));
    assert!(rendered.contains("authority: ProcessOwned"));
    assert!(rendered.contains("active_play="));
    assert!(!rendered.contains("evidence id=evidence/"));
}

#[test]
fn status_vocabulary_keeps_failure_modes_distinct() {
    assert_ne!(OperationalState::Stale, OperationalState::Unreachable);
    assert_ne!(OperationalState::Failed, OperationalState::Denied);
    assert_ne!(OperationalState::Unknown, unsupported_state());
    assert_ne!(PlanLifecycle::Failed, PlanLifecycle::Cancelled);
    assert_ne!(
        TerminalDisposition::Completed,
        TerminalDisposition::Failed {
            reason: conduit_core::FailureReason::UnsupportedKind,
        }
    );
}
