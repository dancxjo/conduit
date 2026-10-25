//! CPU execution of actual Root policy; no MCU, entropy or Body start is claimed.
#![allow(
    unexpected_cfgs,
    dead_code,
    unused_imports,
    reason = "shared device policy has additional entry points"
)]
extern crate alloc;
use conduit_core as c;
use conduit_environment_runtime::{appliance::software::ReadySoftwareBacks, construction::compile};
use conduit_monotonic_wait::*;
use conduit_pulse_capture::PulseReadContract;
use std::{cell::Cell, collections::BTreeMap, rc::Rc};
#[path = "display_support/concurrent_root.rs"]
mod concurrent_display;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_authority.rs"]
mod native_authority;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_retirement.rs"]
mod native_retirement;
mod plan {
    pub const PREFIX: &str = "fixture/root-policy";
}
#[path = "../../runtime/tests/support/pulse/provider.rs"]
mod pulse;
struct Wait(Rc<Cell<usize>>);
impl MonotonicWaitProvider for Wait {
    async fn wait(&mut self, _: u64) -> Result<(), WaitProviderFailure> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
}
#[test]
fn root_checks_both_owners_before_spending_issuance_or_moving_either() {
    let source = compile(3, 250).unwrap();
    let (pulse, effects) = pulse::ready(0);
    let native = pulse::identity();
    let mut issuer =
        native_authority::RootIssuer::fixture(native.host_id.clone(), native.boot_id.clone());
    let mut foreign =
        native_authority::RootIssuer::fixture(native.host_id.clone(), "stale/boot".into());
    let calls = Rc::new(Cell::new(0));
    let wait_identity = WaitNativeIdentity {
        host_id: native.host_id.clone(),
        boot_id: native.boot_id.clone(),
        base_id: "fixture/wait-base".into(),
        provider_instance_id: "fixture/wait-provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/wait-resource".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/wait/1".into()),
        envelope_id: "fixture/wait-envelope".into(),
        artifact_id: "fixture/wait-artifact".into(),
        implementation_id: "fixture/wait@1".into(),
        base_implementation_id: "fixture/wait-base@1".into(),
    };
    // SAFETY: inert CPU timer retains only the counted state; it never touches hardware.
    let wait = unsafe {
        ReadyMonotonicWaitBase::new(
            wait_identity.clone(),
            WaitAttachment {
                generation: 1,
                resource_bytes: 64,
                maximum_duration_ms: 1000,
            },
            Wait(calls.clone()),
        )
    }
    .unwrap();
    let grants = [
        issuer.grant_pulse(pulse.native_identity()).unwrap(),
        issuer.grant_wait(wait.native_identity()).unwrap(),
    ];
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: native.host_id.clone(),
        boot_id: native.boot_id.clone(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/root-policy".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let contract = PulseReadContract::prepare().unwrap();
    pulse.append_to_advertisement(&mut host, &contract).unwrap();
    wait.append_to_advertisement(&mut host).unwrap();
    let mut software =
        ReadySoftwareBacks::prepare(&source, native.host_id, native.boot_id).unwrap();
    software.append_to_advertisement(&mut host).unwrap();
    let recipe = conduit_environment_make::NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    // SAFETY: these bytes were just checked and encoded by trusted Make.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            recipe.planning_image(),
        )
    }
    .unwrap();
    let hosts = [host];
    let choices = conduit_planner::indexed_source::default_placements(&image, &hosts).unwrap();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let queues = source
        .expanded
        .connections
        .iter()
        .map(|cord| {
            let reading = source.expanded.gears.iter().any(|g| {
                g.gear_id == cord.sink_gear_id
                    && g.kind_id.as_str()
                        == conduit_environment_runtime::publication::PublicationChannel::Reading
                            .kind_id()
            });
            let limit = [&cord.source_gear_id, &cord.sink_gear_id]
                .into_iter()
                .map(|gear| {
                    let choice = &choices.by_gear[gear];
                    hosts[0]
                        .capabilities
                        .iter()
                        .find(|offer| offer.capability_id == choice.capability_id)
                        .unwrap()
                        .limits
                        .max_queue_bytes
                })
                .min()
                .unwrap();
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: limit.min(if reading {
                        conduit_environment_runtime::publication::PublicationChannel::Reading
                            .maximum_bytes() as u32
                    } else {
                        1024
                    }),
                },
            )
        })
        .collect();
    let compact = conduit_planner::indexed_source::plan_compact_with_connection_limits(
        &image,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 1024,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &queues,
    )
    .unwrap();
    let metadata = compact.into_verified_metadata().unwrap();
    let fragment = &metadata.preparation_metadata().fragments[0];
    let fragment_id = fragment.fragment_id.clone();
    let placement = |kind: &str| {
        fragment
            .placements
            .iter()
            .find(|g| g.kind_id.as_str() == kind)
            .unwrap()
            .placement_id
            .clone()
    };
    let pulse_placement = placement(conduit_pulse_capture::PULSE_READ_KIND);
    let wait_placement = placement(conduit_time::TICK_KIND);
    let lowered = conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
        &metadata,
        &fragment_id,
    )
    .unwrap();
    type LocalTables<'a> =
        conduit_plan_lowering::local_tables::VerifiedLocalKernelTables<'a, 15, 14, 1, 15, 14, 15>;
    let local = LocalTables::prepare(&metadata, &fragment_id, &lowered).unwrap();
    assert_eq!(local.startup_order().len(), 15);
    assert_eq!(local.preparation_scope().0.plan_id(), metadata.plan_id());
    for (actual, expected) in local.cords.iter().zip(&lowered.cords) {
        assert_eq!(*actual, expected.spec);
    }
    for (actual, expected) in local.nodes.iter().zip(&lowered.node_specs) {
        assert_eq!(actual.maximum_step_fuel, expected.maximum_step_fuel);
        assert_eq!(actual.input_cords[0], expected.input_cords[0]);
    }
    for route in &lowered.routes {
        assert_eq!(
            local
                .routes
                .route(route.source_node, route.source_port)
                .unwrap()
                .collect::<Vec<_>>(),
            route.targets
        );
    }
    let mut substituted = lowered.clone();
    substituted.node_specs[0].maximum_step_fuel += 1;
    assert!(matches!(
        LocalTables::prepare(&metadata, &fragment_id, &substituted),
        Err(conduit_plan_lowering::local_tables::LocalTableRefusal::Lowering)
    ));
    assert!(conduit_plan_lowering::local_tables::VerifiedLocalKernelTables::<14, 14, 1, 15, 14, 15>::prepare(&metadata, &fragment_id, &lowered).is_err());
    assert!(foreign
        .reserve_verified_play(&metadata, &fragment_id)
        .is_err());
    assert_eq!(foreign.next_play_sequence(), Some(1));
    assert!(issuer
        .reserve_verified_play(&metadata, &"missing".into())
        .is_err());
    assert_eq!(issuer.next_play_sequence(), Some(1));
    let active = issuer
        .reserve_verified_play(&metadata, &fragment_id)
        .unwrap();
    assert_eq!(active.play_sequence, 1);
    let cadence = conduit_time::TickConfiguration {
        count: 3,
        period_ms: 250,
    };
    let wrong_cadence = conduit_time::TickConfiguration {
        count: 3,
        period_ms: 251,
    };
    let mut pulse = Some(pulse);
    let mut wait = Some(wait);
    let input = |cadence| native_authority::VerifiedNativeInput {
        plan: &metadata,
        fragment: &fragment_id,
        lowered: &lowered,
        active: &active,
        cadence,
        pulse: &pulse_placement,
        wait: &wait_placement,
        contract: &contract,
    };
    // Pulse preflight succeeds, then timer refusal must preserve both owners and
    // issuance. The subsequent correct request uses the same reserved Play.
    assert!(matches!(
        issuer.bind_verified_native(&mut pulse, &mut wait, input(&wrong_cadence)),
        Err("wait-selection")
    ));
    assert!(pulse.is_some() && wait.is_some());
    assert_eq!((effects.get(), calls.get()), (0, 0));
    let bound = issuer
        .bind_verified_native(&mut pulse, &mut wait, input(&cadence))
        .unwrap();
    assert!(pulse.is_none() && wait.is_none());
    assert_eq!((effects.get(), calls.get()), (0, 0));
    assert_eq!(bound.pulse.inspection().scope.maximum_operations, 3);
    assert_eq!(bound.wait.inspection().scope.maximum_operations, 3);
    assert_eq!(
        bound.pulse.inspection().scope.active_play_id,
        active.active_play_id
    );
    assert_eq!(
        bound.wait.inspection().scope.active_play_id,
        active.active_play_id
    );
    // Fresh providers cannot reuse spent issuance, even with the same generation.
    pulse = Some(pulse::ready(0).0);
    // SAFETY: separate inert CPU timer; no physical provider is duplicated.
    wait = Some(
        unsafe {
            ReadyMonotonicWaitBase::new(
                wait_identity,
                WaitAttachment {
                    generation: 1,
                    resource_bytes: 64,
                    maximum_duration_ms: 1000,
                },
                Wait(calls.clone()),
            )
        }
        .unwrap(),
    );
    assert!(matches!(
        issuer.bind_verified_native(&mut pulse, &mut wait, input(&cadence)),
        Err("pulse-possession")
    ));
    assert!(pulse.is_some() && wait.is_some());
    concurrent_display::check(&mut issuer, &active);
    assert_eq!(
        bound.pulse.inspection().scope.active_play_id,
        active.active_play_id
    );
    assert_eq!(
        bound.wait.inspection().scope.active_play_id,
        active.active_play_id
    );
    assert_eq!((effects.get(), calls.get()), (0, 0));
}
