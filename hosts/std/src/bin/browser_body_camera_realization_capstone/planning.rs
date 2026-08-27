use std::collections::BTreeMap;

use conduit_body::MembershipCredential;
use conduit_core::{
    process_owned_line_offer_with_limits, resource_offer, AcquiredMediaResource,
    AuthorityContractId, AuthorityGrant, BaseImplementationId, HostAdvertisement,
    HostOperationContractId, KindId, LinkLimits, Plan, PortId,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ExpandedCanonicalForm,
    ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use conduit_std_catalog::{
    acquired_camera_source_offer, install_human_media_catalogs, CAMERA_FRAME_KIND,
    CAMERA_SOURCE_KIND, MEDIA_USE_OPERATION,
};
use conduit_std_host::browser_admission::browser_webrtc_line_contract;
use conduit_wire::SessionBinding;

const SOURCE: &str = include_str!("../../../../../examples/camera-summary.conduit");

pub(super) struct CameraRealization {
    pub(super) plan: Plan,
    pub(super) binding: SessionBinding,
    pub(super) output_port: PortId,
}

fn expanded() -> Result<ExpandedCanonicalForm, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_human_media_catalogs(&mut startup, &mut profile).map_err(debug("catalog"))?;
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(debug("check form"))?;
    expand_canonical_form(&checked, "camera-summary", &profile).map_err(debug("expand form"))
}

fn placements(
    expanded: &ExpandedCanonicalForm,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> Result<PlacementChoices, String> {
    let mut by_gear = BTreeMap::new();
    for gear in &expanded.gears {
        let host = if gear.kind_id.as_str() == CAMERA_SOURCE_KIND {
            source
        } else {
            sink
        };
        let capability = host
            .capabilities
            .iter()
            .find(|offer| offer.kind_id == gear.kind_id)
            .ok_or_else(|| format!("Host lacks exact capability {}", gear.kind_id.as_str()))?;
        by_gear.insert(
            gear.gear_id.clone(),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: capability.capability_id.clone(),
            },
        );
    }
    Ok(PlacementChoices { by_gear })
}

pub(super) fn realize(
    source_credential: &MembershipCredential,
    source_advertisement: &HostAdvertisement,
    sink_advertisement: &HostAdvertisement,
    resource: &AcquiredMediaResource,
) -> Result<CameraRealization, String> {
    if resource.host_id != source_credential.host_id
        || resource.boot_id != source_credential.boot_id
    {
        return Err("resource truth is not owned by the selected source Host/Boot".into());
    }
    let expanded = expanded()?;
    let mut source = source_advertisement.clone();
    let camera = acquired_camera_source_offer();
    let camera_capability = camera.capability_id.clone();
    source.capabilities.push(camera);
    source.resources.push(resource_offer(
        resource.handle_id.as_str(),
        resource.class_id.as_str(),
        1,
    ));
    let sink = sink_advertisement.clone();
    let placements = placements(&expanded, &source, &sink)?;
    let authority = AuthorityGrant {
        grant_id: resource.use_authority_grant.clone(),
        contract_id: AuthorityContractId::from(resource.use_authority_contract.as_str()),
        host_operation_contract_id: HostOperationContractId::from(MEDIA_USE_OPERATION),
        subject_kind: KindId::from(CAMERA_FRAME_KIND),
        host_id: source.host_id.clone(),
        boot_id: source.boot_id.clone(),
        capability_id: camera_capability,
    };
    let mut line = process_owned_line_offer_with_limits(
        "browser/body-camera-realization/camera-line",
        "browser/body-camera-realization/camera-binding",
        BaseImplementationId::from("conduit.base/webrtc-data-channel@1"),
        "browser/body-camera-realization/camera-base",
        &source,
        &sink,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 64 * 1024,
            maximum_buffered_bytes: 64 * 1024,
            maximum_frame_bytes: 128 * 1024,
        },
    );
    line.contract = browser_webrtc_line_contract();
    let connection = expanded
        .connections
        .first()
        .ok_or("camera form has no exact Cord")?;
    let line_candidates = BTreeMap::from([(
        (
            connection.source_gear_id.clone(),
            connection.sink_gear_id.clone(),
        ),
        vec![line.line_id.clone()],
    )]);
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &[source, sink],
        &placements,
        &[BaseImplementationId::from(
            "conduit.base/webrtc-data-channel@1",
        )],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 64 * 1024,
            authority_grants: &[authority],
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .map_err(debug("plan camera form"))?;
    let source_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == source_credential.host_id)
        .ok_or("source fragment absent")?;
    let planned = source_fragment
        .connections
        .iter()
        .find(|planned| planned.value_kind.as_str() == CAMERA_FRAME_KIND)
        .ok_or("planned camera Cord absent")?;
    let sink_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id != source_credential.host_id)
        .ok_or("sink fragment absent")?;
    let binding = SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source_fragment.fragment_id.clone(),
        sink_fragment.fragment_id.clone(),
        planned,
    )
    .map_err(debug("bind planned camera Cord"))?;
    Ok(CameraRealization {
        plan,
        binding,
        output_port: PortId::from("frame"),
    })
}

fn debug<T: core::fmt::Debug>(label: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{label}: {error:?}")
}
