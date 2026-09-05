//! Cross-Host proof that browser button transitions feed the portable timing Forms.

use super::factory;
use std::collections::BTreeMap;

use conduit_core::{
    process_owned_line_offer_with_limits, resource_offer, BaseImplementationId, BootId,
    HostAdvertisement, HostId, HostProfileId, LinkLimits, OfferGeneration, PROTOCOL_VERSION,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

const SOURCE: &str = r#"form browser-knock-trigger {
    button: input/button
    attempt: time/pressed-button-attempt(maximum-presses = 3, maximum-transitions = 4, timeout-ms = 1000ms)
    intervals: time/ordered-event-intervals
    normalize: sequence/normalize-relative-duration

    button.transition > attempt.transition
    attempt.events > intervals.events
    intervals.intervals > normalize.intervals
}
"#;

#[test]
fn real_browser_button_offer_feeds_the_same_portable_timing_forms() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_timed_pattern_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_timed_button_attempt_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)
        .unwrap();
    let syntax = conduit_form::parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "browser-knock-trigger", &profile).unwrap();

    let browser = factory::advertisement(
        HostId::from("browser/secret-knock-trigger"),
        BootId::from("browser/secret-knock-trigger-boot"),
    );
    let std = timing_host();
    let hosts = [browser.clone(), std.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == conduit_semantic_catalog::BUTTON_SOURCE_KIND
                {
                    &browser
                } else {
                    &std
                };
                let capability_id = host
                    .capabilities
                    .iter()
                    .find(|offer| offer.kind_id == gear.kind_id)
                    .unwrap()
                    .capability_id
                    .clone();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id,
                    },
                )
            })
            .collect(),
    };
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        maximum_buffered_bytes: (super::MAXIMUM_BROWSER_VALUE_BYTES * 4) as u32,
        maximum_frame_bytes: (super::MAXIMUM_BROWSER_VALUE_BYTES * 2) as u32,
    };
    let line = process_owned_line_offer_with_limits(
        "line/browser-secret-knock-trigger",
        "binding/browser-secret-knock-trigger",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "base/browser-secret-knock-trigger",
        &browser,
        &std,
        limits,
    );
    let crossing = expanded
        .connections
        .iter()
        .find(|connection| connection.source_gear_id.as_str() == "browser-knock-trigger/button")
        .unwrap();
    let line_candidates = BTreeMap::from([(
        (
            crossing.source_gear_id.clone(),
            crossing.sink_gear_id.clone(),
        ),
        vec![line.line_id.clone()],
    )]);
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: core::slice::from_ref(&line),
        },
    )
    .unwrap();

    let button = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::BUTTON_SOURCE_KIND)
        .unwrap();
    assert_eq!(button.host_id, browser.host_id);
    assert_eq!(
        button.implementation_id.as_str(),
        super::input::BUTTON_IMPLEMENTATION
    );
    for kind in [
        conduit_semantic_catalog::TIMED_BUTTON_ATTEMPT_KIND,
        conduit_semantic_catalog::ORDERED_EVENT_INTERVALS_KIND,
        conduit_semantic_catalog::NORMALIZE_SEQUENCE_KIND,
    ] {
        assert!(plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .any(|gear| gear.kind_id.as_str() == kind && gear.host_id == std.host_id));
    }
    let planned = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|cord| cord.selected_line.is_some())
        .unwrap();
    assert_eq!(planned.value_kind, crossing.value_kind);
    assert_eq!(
        planned.selected_line.as_ref().unwrap().line_id,
        line.line_id
    );
}

fn timing_host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("std/secret-knock-timing"),
        boot_id: BootId::from("std/secret-knock-timing-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/secret-knock-timing@1"),
        resources: vec![
            resource_offer("std/clock", conduit_core::TIMER_RESOURCE_CLASS, 2),
            resource_offer(
                "std/deadline",
                conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
                1,
            ),
        ],
        capabilities: vec![
            conduit_std_offers::timed_button_attempt_std_offer(),
            conduit_std_offers::ordered_event_intervals_std_offer(),
            conduit_std_offers::normalize_sequence_std_offer(),
        ],
        planner_capabilities: vec![],
    }
}
