use std::collections::BTreeMap;

use conduit_core::{
    resource_offer, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BootId, CapabilityId,
    ConnectionBase, HostAdvertisement, HostId, HostOperationContractId, HostProfileId, KindId,
    LinkLimits, OfferGeneration, PROTOCOL_VERSION,
};
use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use conduit_std_catalog::{
    acquired_camera_source_offer, browser_camera_frame_sink_offer, install_human_media_catalogs,
    CAMERA_FRAME_KIND, CAMERA_RESOURCE_CLASS, CAMERA_SOURCE_KIND, MEDIA_USE_AUTHORITY,
    MEDIA_USE_OPERATION,
};

const SOURCE: &str = include_str!("../../../examples/camera-summary.conduit");

fn advertisement(host: &str, boot: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/human-body@1"),
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_human_media_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    expand_canonical_form(&checked, "camera-summary", &profile).unwrap()
}

fn choices(
    expanded: &conduit_form::ExpandedCanonicalForm,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> PlacementChoices {
    PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == CAMERA_SOURCE_KIND {
                    source
                } else {
                    sink
                };
                let capability = host
                    .capabilities
                    .iter()
                    .find(|offer| offer.kind_id == gear.kind_id)
                    .map(|offer| offer.capability_id.clone())
                    .unwrap_or_else(|| CapabilityId::from("absent/pre-acquisition"));
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: capability,
                    },
                )
            })
            .collect(),
    }
}

#[test]
fn body_plan_requires_new_resource_truth_and_seals_exact_camera_cord() {
    let expanded = expanded();
    let mut source = advertisement("browser/source", "browser-boot/source-1");
    let mut sink = advertisement("browser/sink", "browser-boot/sink-1");
    sink.capabilities.push(browser_camera_frame_sink_offer());
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();

    let before = choices(&expanded, &source, &sink);
    assert!(plan_expanded_canonical_with_options(
        &expanded,
        &[source.clone(), sink.clone()],
        &before,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &empty_bases,
            line_candidates: &empty_lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 64 * 1024,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .is_err());

    let camera = acquired_camera_source_offer();
    let camera_capability = camera.capability_id.clone();
    source.capabilities.push(camera);
    source.resources.push(resource_offer(
        "browser/source/opaque-track-7",
        CAMERA_RESOURCE_CLASS,
        1,
    ));
    let use_grant = AuthorityGrant {
        grant_id: AuthorityGrantId::from("browser/source/use-opaque-track-7"),
        contract_id: AuthorityContractId::from(MEDIA_USE_AUTHORITY),
        host_operation_contract_id: HostOperationContractId::from(MEDIA_USE_OPERATION),
        subject_kind: KindId::from(CAMERA_FRAME_KIND),
        host_id: source.host_id.clone(),
        boot_id: source.boot_id.clone(),
        capability_id: camera_capability,
    };
    let placements = choices(&expanded, &source, &sink);
    assert!(plan_expanded_canonical_with_options(
        &expanded,
        &[source.clone(), sink.clone()],
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &empty_bases,
            line_candidates: &empty_lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 64 * 1024,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .is_err());

    let line = conduit_core::process_owned_line_offer_with_limits(
        "browser/human-body/camera-line",
        "browser/human-body/camera-binding",
        ConnectionBase::WebRtcDataChannel,
        "browser/human-body/camera-base",
        &source,
        &sink,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 64 * 1024,
            maximum_buffered_bytes: 64 * 1024,
            maximum_frame_bytes: 128 * 1024,
        },
    );
    let connection = &expanded.connections[0];
    let line_candidates = BTreeMap::from([(
        (
            connection.source_gear_id.clone(),
            connection.sink_gear_id.clone(),
        ),
        vec![line.line_id.clone()],
    )]);
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &[source.clone(), sink.clone()],
        &placements,
        &[ConnectionBase::WebRtcDataChannel],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 64 * 1024,
            authority_grants: std::slice::from_ref(&use_grant),
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .unwrap();

    let source_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == source.host_id)
        .unwrap();
    let source_gear = source_fragment
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == CAMERA_SOURCE_KIND)
        .unwrap();
    assert_eq!(source_gear.resources.len(), 1);
    assert_eq!(
        source_gear.resources[0].pool_id.as_str(),
        "browser/source/opaque-track-7"
    );
    assert_eq!(source_gear.authority[0].grant_id, use_grant.grant_id);
    let cord = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| connection.value_kind.as_str() == CAMERA_FRAME_KIND)
        .unwrap();
    assert_eq!(cord.source_port_id.as_str(), "frame");
    assert_eq!(cord.sink_port_id.as_str(), "frame");
    assert_eq!(cord.item_capacity, 1);
    assert_eq!(cord.byte_capacity, 64 * 1024);
    assert_eq!(
        cord.selected_line.as_ref().unwrap().line_id.as_str(),
        "browser/human-body/camera-line"
    );
}
