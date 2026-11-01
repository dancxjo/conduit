//! CPU measurement of the real environmental Source/native renderer descriptor.
//! Provider readiness is a fake-pin fixture; no sensing, display or MCU claim.
use super::*;
#[allow(
    dead_code,
    reason = "measurement borrows renderer metadata without issuing effects"
)]
#[path = "../support/native_mask.rs"]
mod native;
use conduit_planner::{
    indexed_source as indexed, ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions,
};
use std::{collections::BTreeMap, rc::Rc};
#[test]
fn actual_native_verified_context_uses_bounded_workspace_and_preserves_full_wire() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let material = conduit_environment_make::SevenSegmentMaskMaterial::prepare(&source).unwrap();
    // SAFETY: exact image emitted immediately from the checked Source above.
    // This CPU fixture uses its existing JSON decoder, not the MCU's linked
    // callback. Both comparison runs use the same configuration supplier.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            material.planning_image(),
        )
    }
    .unwrap();
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let mut host = fixture::host();
    host.capabilities
        .retain(|offer| offer.kind_id.as_str() != f::RENDERER_KIND);
    host.resources.clear();
    ready.append_to_advertisement(&mut host).unwrap();
    let grants = [native::grant(&ready)];
    let boundary = material.planning_boundary().unwrap();
    let choices = indexed::default_placements(&image, std::slice::from_ref(&host)).unwrap();
    let fore: BTreeMap<_, _> = boundary
        .bindings()
        .map(|(direction, port, track)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port.clone(),
                    track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 8192,
                },
            )
        })
        .collect();
    let empty = BTreeMap::new();
    let bases = ["conduit.base/local@1".into()];
    let compact = indexed::plan_compact_authoring_consuming_with_connection_limits(
        &image,
        &boundary,
        vec![host],
        &choices,
        &bases,
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8192,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &BTreeMap::new(),
        &fore,
    )
    .unwrap();
    // Rich materialization is a test oracle outside the measured preparation.
    let planned =
        f::PlannedMaskPlot::admit(material.role(), &compact.materialize().unwrap()).unwrap();
    let expected = serde_json::to_vec(&planned).unwrap();
    let verified = compact.into_verified_metadata().unwrap();
    let mut first = [0xa5; 8192 + 16];
    let mut second = [0x5a; 8192 + 16];
    let (context, memory) = {
        let mut workspace = [&mut first[8..8200], &mut second[8..8200]];
        allocation_probe::observe(|| {
            f::PreparedMaskShowContext::prepare_verified_with_workspace(
                material.role(),
                &verified,
                |gear| {
                    for index in 0..image.gear_count() {
                        let configuration = image.gear_configuration(index).map_err(|_| ())?;
                        if configuration.gear_id == gear.gear_id {
                            return Ok(configuration.configuration);
                        }
                    }
                    Err(())
                },
                4096,
                &mut workspace,
            )
            .unwrap()
        })
    };
    assert_eq!(context.plan_wire(), expected);
    assert!(context.plan_wire().len() <= 4096);
    assert!(first[..8].iter().chain(&first[8200..]).all(|b| *b == 0xa5));
    assert!(second[..8]
        .iter()
        .chain(&second[8200..])
        .all(|b| *b == 0x5a));
    first[8..8200].fill(0);
    second[8..8200].fill(0);
    assert_eq!(
        pins.edges.get(),
        0,
        "context preparation cannot touch renderer pins"
    );
    eprintln!("actual_native_mask_context allocations={} reallocations={} requested={} peak={} retained={} wire={} plan={}", memory.allocations, memory.reallocations, memory.requested_bytes, memory.peak_bytes, memory.retained_bytes(), context.plan_wire().len(), verified.plan_id().as_str());
}
