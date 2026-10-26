//! Exact display planning from initialized software and inert owned pins. The
//! indexed representation must preserve Fore contracts and canonical Plan IDs.
#[path = "display_support/fixture.rs"]
mod fixture;
use conduit_core as c;
use conduit_environment_make::{DisplayOutputSource, DisplaySourceMaterial};
use conduit_planner::{
    indexed_source as indexed, ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions,
};
use std::collections::BTreeMap;
mod plan {
    pub const PREFIX: &str = "fixture/display";
}
#[path = "../../runtime/tests/appliance_support/mod.rs"]
mod appliance_support;
#[allow(
    dead_code,
    reason = "shared inert provider supports other execution proofs"
)]
#[path = "../../runtime/tests/support/pulse/provider.rs"]
mod pulse;
#[path = "../../runtime/tests/support/verified_metadata.rs"]
mod verified_metadata;

#[test]
fn indexed_display_preserves_the_rich_plan_and_external_fore_refusals() {
    let source = DisplayOutputSource::compile().unwrap();
    source.plot.validate_boundary_provenance().unwrap();
    let material = DisplaySourceMaterial::prepare(&source).unwrap();
    let fixture = fixture::fixture(&source);
    let mut host = fixture.host.clone();
    let mut software = material
        .prepare_compact(host.host_id.clone(), host.boot_id.clone())
        .unwrap();
    host.capabilities
        .retain(|o| o.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND);
    host.capabilities.extend(
        software
            .current_offers(&host.host_id, &host.boot_id, host.offer_generation)
            .unwrap()
            .iter()
            .map(|bytes| serde_json::from_slice::<c::CapabilityOffer>(bytes).unwrap()),
    );
    // SAFETY: private material retains this exact validated graph and Fore.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            material.planning_image(),
        )
    }
    .unwrap();
    let boundary = material.planning_boundary().unwrap();
    let placements = indexed::default_placements(&image, core::slice::from_ref(&host)).unwrap();
    assert_eq!(
        placements,
        conduit_planner::default_expanded_placements(
            &source.plot.expanded,
            core::slice::from_ref(&host)
        )
        .unwrap()
    );
    let queues = source
        .plot
        .expanded
        .connections
        .iter()
        .map(|cord| {
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: conduit_tm1637_wire::MAXIMUM_BYTES,
                },
            )
        })
        .collect();
    let mut limits = BTreeMap::new();
    for (direction, bindings, bytes) in [
        (
            c::PortDirection::Input,
            &source.plot.input_bindings,
            conduit_tm1637_wire::MAXIMUM_BYTES,
        ),
        (c::PortDirection::Output, &source.plot.output_bindings, 64),
    ] {
        for binding in bindings {
            limits.insert(
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let grants = [fixture.grant];
    let bases = ["conduit.base/local@1".into(), "fixture/base".into()];
    let mut display_plan = None;
    for change in 0..4 {
        let mut boundary_limits = limits.clone();
        match change {
            1 => {
                boundary_limits.pop_first();
            }
            2 => boundary_limits.values_mut().next().unwrap().byte_capacity = 0,
            3 => boundary_limits.values_mut().next().unwrap().byte_capacity = u32::MAX,
            _ => (),
        }
        let options = || PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tm1637_wire::MAXIMUM_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        };
        let rich = conduit_planner::plan_expanded_authoring_with_connection_limits(
            &source.plot,
            core::slice::from_ref(&host),
            &placements,
            &bases,
            options(),
            &queues,
            &boundary_limits,
        );
        let compact = indexed::plan_compact_authoring_with_connection_limits(
            &image,
            &boundary,
            core::slice::from_ref(&host),
            &placements,
            &bases,
            options(),
            &queues,
            &boundary_limits,
        );
        match (rich, compact) {
            (Ok(rich), Ok(compact)) => {
                assert_eq!(change, 0);
                assert_eq!(compact.materialize().unwrap(), rich);
                let verified = compact.into_verified_metadata().unwrap();
                assert_eq!(verified.plan_id(), &rich.plan_id);
                assert_eq!(
                    verified.preparation_metadata().fragments[0].fore_ports,
                    rich.fragments[0].fore_ports
                );
                assert_eq!(rich.fragments[0].fore_ports.len(), 2);
                display_plan = Some(verified);
            }
            (Err(rich), Err(compact)) => assert_eq!(rich, compact),
            _ => panic!("rich/indexed Fore admission differs for case {change}"),
        }
    }
    let display_plan = display_plan.unwrap();
    let sampling = conduit_environment_runtime::construction::compile_default().unwrap();
    let mut pulse_identity = pulse::identity();
    pulse_identity.boot_id = host.boot_id.clone();
    let (pulse, effects) = pulse::ready_with_identity(0, pulse_identity);
    let mut sampler =
        conduit_environment_runtime::appliance::software::ReadySoftwareBacks::prepare(
            &sampling,
            host.host_id.clone(),
            host.boot_id.clone(),
        )
        .unwrap();
    let sample_host = appliance_support::advertisement(&sampling, &mut sampler, &pulse);
    let mut pulse_grant = pulse::grant();
    pulse_grant.boot_id = host.boot_id.clone();
    let sample = sampling
        .plan_local(
            &sample_host,
            &["conduit.base/local@1".into()],
            &[pulse_grant],
        )
        .unwrap();
    let image = conduit_environment_make::EnvironmentBodyImage::prepare(
        "01234567-89ab-4cde-8fab-0123456789ab".into(),
        1,
        "birth/combined".into(),
    )
    .unwrap();
    let workset = image.body.evidence().body.workset.clone();
    assert_eq!(workset.len(), 2);
    let wake = image
        .body
        .evidence()
        .body
        .wake(2, "wake/combined".into())
        .unwrap()
        .1;
    let sample = verified_metadata::from_plan(&sample);
    let resident = |plan: &c::VerifiedPlanMetadata| {
        conduit_body::ResidentPlot::new(
            plan.preparation_metadata().source_document_id.clone(),
            plan.preparation_metadata().checked_plot_id.clone(),
        )
    };
    let seal = conduit_body::BodyPlanSeal::capture_verified(
        &wake,
        &[
            conduit_body::VerifiedBodyPlotPlan {
                plot: resident(&sample),
                plan: &sample,
            },
            conduit_body::VerifiedBodyPlotPlan {
                plot: resident(&display_plan),
                plan: &display_plan,
            },
        ],
        &[],
        None,
    )
    .unwrap();
    assert_eq!(seal.workset(), &workset);
    assert_eq!(seal.plots().len(), 2);
    assert!(seal
        .plots()
        .iter()
        .flat_map(|p| p.fragments())
        .all(|f| f.host_id() == &host.host_id && f.boot_id() == &host.boot_id));
    assert_eq!(effects.get(), 0);
}
