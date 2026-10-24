//! Ordinary Body admission with CPU membership, pins and Mask offer fixtures.
use super::*;
use conduit_body::{BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology};
use conduit_environment_make::DisplayMaskSource;
use std::collections::BTreeMap;
#[path = "mask_offers.rs"]
mod mask_offers;

#[test]
fn sampler_and_selected_mask_share_one_exact_body_proposal() {
    let source = construction::compile_default().unwrap();
    let (ready, effects) = pulse::ready(0);
    let (_software, mut host) = retained_advertisement(&source, &ready);
    let sampler = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &[c::LOCAL_BASE_IMPLEMENTATION_ID.into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let mask_source = DisplayMaskSource::compile().unwrap();
    host.capabilities.extend(mask_offers::offers(&mask_source));
    let boundaries = mask_source
        .plot()
        .front
        .inputs()
        .iter()
        .chain(mask_source.plot().front.outputs())
        .map(|port| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction: port.direction,
                    front_port_id: port.port_id.clone(),
                    track: c::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: conduit_face::MAX_FACE_TOTAL_BYTES as u32,
                },
            )
        })
        .collect();
    let planned = mask_source
        .plan(
            core::slice::from_ref(&host),
            &[c::LOCAL_BASE_IMPLEMENTATION_ID.into()],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_face::MAX_FACE_TOTAL_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
    let admission = conduit_face::MaskTopologyAdmission::from_plan(&planned.plan).unwrap();
    assert_eq!(admission.chains.len(), 1);
    let topology = BodyMaskTopology {
        face: BodyFaceSelector {
            plot: Some(sampler.resident_plot()),
            // The retained producer owner composes this Face; it does not
            // claim an executable Face-output port on a sampling gear.
            source_placement_id: None,
        },
        chains: admission
            .chains
            .iter()
            .map(|chain| BodyMaskChainPlan {
                plan: planned.plan.clone(),
                stage_placement_ids: chain
                    .stages
                    .iter()
                    .map(|s| s.placement_id.clone())
                    .collect(),
            })
            .collect(),
    };
    for invalid in [0, 1] {
        let mut owner = body_owner(&sampler, &host.host_id, &host.boot_id, false);
        let before = owner.evidence().clone();
        let mut wrong = topology.clone();
        if invalid == 0 {
            wrong.face.plot.as_mut().unwrap().source_document_id = "fixture/foreign-source".into();
        } else {
            wrong.face.source_placement_id = Some("fixture/unplanned-placement".into());
        }
        assert!(sampler.propose_to_body(&mut owner, vec![wrong]).is_err());
        assert_eq!(owner.evidence(), &before);
        assert!(owner.realization().is_none());
    }
    let mut owner = body_owner(&sampler, &host.host_id, &host.boot_id, false);
    let proposal = sampler
        .propose_to_body(&mut owner, vec![topology.clone()])
        .unwrap();
    assert_eq!(proposal.plan.plots.len(), 1);
    assert_eq!(proposal.plan.mask_topologies, [topology]);
    assert_eq!(proposal.plan.plots[0].plan.plan_id, sampler.plan().plan_id);
    assert_eq!(effects.get(), 0);
}
