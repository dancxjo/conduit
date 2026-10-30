// Current Host/resource/authority facts are explicit CPU fixtures.

use super::super::*;
use conduit_environment_make::RecordingPublicationSource;
use std::collections::BTreeMap;
pub(crate) fn plan(
    reference: &c::BoundedResourceRef,
) -> (
    c::Plan,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
) {
    planned(reference, false)
}
pub(crate) fn foreign_plan(
    reference: &c::BoundedResourceRef,
) -> (
    c::Plan,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
) {
    planned(reference, true)
}
fn planned(
    reference: &c::BoundedResourceRef,
    foreign: bool,
) -> (
    c::Plan,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
) {
    let source = RecordingPublicationSource::compile(reference).unwrap();
    let (mut host, mut grant) = fixture_store(reference);
    let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(true);
    let content = host.resources[0].content.as_mut().unwrap();
    content.contract.access = c::ResourceAccessMode::WriteCandidatePublish;
    content.contract.publication_slots = 1;
    let base_id = content.base_id.clone();
    let mut offer =
        conduit_environment_linux::recording_publication_offer(&host.resources[0]).unwrap();
    if foreign {
        offer.implementation.execution_profile_id = "fixture/foreign-publisher@1".into();
        offer.implementation.implementation_id = "fixture/foreign-publisher@1".into();
        offer.implementation.artifact_id = "fixture/foreign-publisher@1".into();
    }
    grant.capability_id = offer.capability_id.clone();
    grant.subject_kind = kind.kind_id;
    grant.host_call_contract_id = conduit_linux_snapshot_store::PUBLISH_OPERATION.into();
    host.bases.push(c::BaseProviderAdvertisement {
        base_id,
        provider_instance_id: "fixture/file-provider".into(),
        provider_generation: 1,
        implementation_id: "conduit.base/local@1".into(),
        mechanism_family: "fixture/files".into(),
        enforcement_class: c::BaseEnforcementClass::Cooperative,
        lifecycle: c::BaseLifecycle::Ready,
        capability_ids: vec![offer.capability_id.clone()],
        resource_pool_ids: vec![host.resources[0].pool_id.clone()],
    });
    host.capabilities = vec![offer];
    let hosts = [host];
    let choices =
        conduit_planner::default_expanded_placements(&source.plot.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (direction, bindings, byte_capacity) in [
        (c::PortDirection::Input, &source.plot.input_bindings, 4096),
        (c::PortDirection::Output, &source.plot.output_bindings, 512),
    ] {
        for binding in bindings {
            boundaries.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &source.plot,
        &hosts,
        &choices,
        &bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    (plan, lowered)
}
