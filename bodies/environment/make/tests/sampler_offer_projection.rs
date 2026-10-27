//! Exact native sampler projection against initialized inert CPU owners. No
//! provider effect, physical startup, Body admission, or grant issuance occurs.
extern crate alloc;
use conduit_core as c;
use conduit_monotonic_wait::*;
use conduit_net::{NetworkJoinRequest, StationAttemptFailure, StationFailure};
use conduit_network_join_call::{credential_installation, installation::*, *};
use conduit_pulse_capture::installation::ReadyPulseBase;
use std::collections::BTreeMap;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod plan {
    pub const PREFIX: &str = "fixture/sampler-projection";
}
#[path = "display_support/fixture.rs"]
#[allow(
    dead_code,
    reason = "only exclusively retained inert electrical pins are reused"
)]
mod display_fixture;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_display_native_offer.rs"]
mod display_projection;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_sampler_offer.rs"]
mod projection;
struct DisplayOwner {
    ready: conduit_tm1637_wire::installation::ReadyWriteBase<display_fixture::NoPins>,
}
impl DisplayOwner {
    fn append_to_advertisement(&self, host: &mut c::HostAdvertisement) -> Result<(), ()> {
        self.ready
            .append_to_advertisement(
                host,
                &conduit_tm1637_wire::WriteContract::prepare().map_err(|_| ())?,
            )
            .map_err(|_| ())
    }
}
fn renderer(
    root: &NativeRoot,
) -> conduit_environment_runtime::mask_renderer::ReadyRenderer<display_fixture::NoPins> {
    use conduit_environment_runtime::{display_mask::PreparedDisplayMask, mask_renderer::*};
    use conduit_seven_segment_mask::{DigitWiring, SevenSegmentLook, Tm1637Profile};
    let identity = RendererIdentity {
        host_id: root.host.clone(),
        boot_id: root.boot.clone(),
        base_id: "fixture/display-base".into(),
        provider_instance_id: "fixture/display-instance".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/display-pool".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/display-generation".into()),
        envelope_id: "fixture/display-envelope".into(),
        artifact_id: "fixture/display-artifact".into(),
        implementation_id: "fixture/display-implementation".into(),
        base_implementation_id: "fixture/display-base-implementation".into(),
    };
    let mask = PreparedDisplayMask::prepare(
        Tm1637Profile {
            digits: core::array::from_fn(|i| DigitWiring {
                address: i as u8,
                segment_bits: [0, 1, 2, 3, 4, 5, 6],
            }),
            decimal_points: [None; 4],
            colon: None,
        },
        SevenSegmentLook::fahrenheit(),
        3000,
        120000,
        3,
    )
    .unwrap();
    // SAFETY: exclusive inert pins, finite immutable Look/profile, no effects.
    unsafe {
        ReadyRenderer::new(
            identity,
            Bounds {
                face_bytes: 8192,
                show_bytes: 8192,
                resource_bytes: 4096,
            },
            display_fixture::NoPins,
            mask,
        )
        .unwrap()
    }
}

#[path = "../../runtime/tests/support/pulse/provider.rs"]
mod pulse;
struct Inert;
impl MonotonicWaitProvider for Inert {
    async fn wait(&mut self, _: u64) -> Result<(), WaitProviderFailure> {
        panic!("planning has no timer effect")
    }
}
impl NetworkJoinProvider for Inert {
    async fn join(
        &mut self,
        _: NetworkJoinRequest<'_>,
    ) -> Result<AttachmentRecord, StationAttemptFailure> {
        panic!("planning has no radio effect")
    }
    async fn cleanup(&mut self) -> Result<(), StationFailure> {
        panic!("planning has no cleanup effect")
    }
}
impl CredentialSource for Inert {
    fn take_acknowledged(&mut self, _: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        panic!("planning has no credential effect")
    }
}
struct NativeRoot {
    host: c::HostId,
    boot: c::BootId,
    offer_generation: u64,
    pulse: Option<ReadyPulseBase<pulse::Provider>>,
    wait: Option<ReadyMonotonicWaitBase<Inert>>,
    display: DisplayOwner,
}
fn root() -> (NativeRoot, std::rc::Rc<std::cell::Cell<usize>>) {
    let (pulse, effects) = pulse::ready(0);
    let id = pulse.native_identity();
    let native = WaitNativeIdentity {
        host_id: id.host_id.clone(),
        boot_id: id.boot_id.clone(),
        base_id: "fixture/wait-base".into(),
        provider_instance_id: "fixture/wait-provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/wait-pool".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/wait/1".into()),
        envelope_id: "fixture/wait-envelope".into(),
        artifact_id: "fixture/wait-artifact".into(),
        implementation_id: "fixture/wait@1".into(),
        base_implementation_id: "fixture/wait-base@1".into(),
    };
    // SAFETY: actual exclusive CPU fixture owns finite inert timer only.
    let wait = unsafe {
        ReadyMonotonicWaitBase::new(
            native,
            WaitAttachment {
                generation: 1,
                resource_bytes: 64,
                maximum_duration_ms: 60_000,
            },
            Inert,
        )
        .unwrap()
    };
    (
        NativeRoot {
            display: DisplayOwner {
                ready: display_fixture::ready(&id.host_id, &id.boot_id),
            },
            host: id.host_id.clone(),
            boot: id.boot_id.clone(),
            offer_generation: 7,
            pulse: Some(pulse),
            wait: Some(wait),
        },
        effects,
    )
}
fn network_id(root: &NativeRoot, role: &str) -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: root.host.clone(),
        boot: root.boot.clone(),
        base: format!("fixture/{role}/base").into(),
        instance: format!("fixture/{role}/instance").into(),
        provider_generation: 1,
        resource: format!("fixture/{role}/pool").into(),
        resource_generation: c::ResourceGenerationId(format!("fixture/{role}/generation")),
        envelope: format!("fixture/{role}/envelope").into(),
        artifact: format!("fixture/{role}/artifact").into(),
        implementation: format!("fixture/{role}/implementation").into(),
        base_implementation: format!("fixture/{role}/base-implementation").into(),
    }
}
fn grants(root: &NativeRoot) -> [c::AuthorityGrant; 2] {
    [
        pulse::grant(),
        c::AuthorityGrant {
            grant_id: "fixture/wait-grant".into(),
            contract_id: WAIT_AUTHORITY.into(),
            host_call_contract_id: conduit_core::WAIT_HOST_CALL_CONTRACT.into(),
            subject_kind: conduit_time::TICK_KIND.into(),
            host_id: root.host.clone(),
            boot_id: root.boot.clone(),
            capability_id: WAIT_CAPABILITY.into(),
        },
    ]
}
fn plan(
    source: &conduit_environment_runtime::construction::ApplianceSource,
    host: c::HostAdvertisement,
    grants: &[c::AuthorityGrant],
) -> Result<c::VerifiedPlanMetadata, conduit_planner::PlannerError> {
    let bytes = conduit_plot::flat_image::indexed::encode(&source.expanded).unwrap();
    // SAFETY: trusted CPU Make fixture encoded this complete checked expansion.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let choices =
        conduit_planner::indexed_source::default_placements(&image, std::slice::from_ref(&host))?;
    let mut queues = BTreeMap::new();
    for cord in &source.expanded.connections {
        let mut capacity = 1024;
        for gear in [&cord.source_gear_id, &cord.sink_gear_id] {
            let selected = &choices.by_gear[gear];
            let offer = host
                .capabilities
                .iter()
                .find(|o| o.capability_id == selected.capability_id)
                .unwrap();
            if offer.kind_id.as_str()
                == conduit_environment_runtime::publication::PublicationChannel::Reading.kind_id()
            {
                capacity = conduit_environment_runtime::publication::PublicationChannel::Reading
                    .maximum_bytes() as u32;
            }
        }
        for gear in [&cord.source_gear_id, &cord.sink_gear_id] {
            let selected = &choices.by_gear[gear];
            let offer = host
                .capabilities
                .iter()
                .find(|o| o.capability_id == selected.capability_id)
                .unwrap();
            capacity = capacity.min(offer.limits.max_queue_bytes);
        }
        if capacity != 1024 {
            queues.insert(
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: capacity,
                },
            );
        }
    }
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let compact = conduit_planner::indexed_source::plan_compact_consuming_with_connection_limits(
        &image,
        vec![host],
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 1024,
            authority_grants: grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &queues,
    )?;
    compact.into_verified_metadata()
}
#[test]
fn sampler_projection_preserves_exact_plan_and_missing_authority_resource_refusals() {
    use conduit_environment_runtime::appliance::software::ReadySoftwareBacks;
    let (mut root, effects) = root();
    let source = conduit_environment_runtime::construction::compile_default().unwrap();
    let mut software =
        ReadySoftwareBacks::prepare(&source, root.host.clone(), root.boot.clone()).unwrap();
    let (mut narrow, narrow_memory) =
        allocation_probe::observe(|| root.sampler_native_advertisement().unwrap());
    software.append_to_advertisement(&mut narrow).unwrap();
    // Full current Root also owns initialized unused network/credential slots.
    // SAFETY: these exclusively owned inert fixtures cannot perform any effect.
    let (credentials, join) = unsafe {
        (
            credential_installation::ReadyCredentialBase::new(
                network_id(&root, "credentials"),
                Inert,
            )
            .unwrap(),
            ReadyNetworkJoinBase::new(network_id(&root, "network"), 1, Inert).unwrap(),
        )
    };
    let renderer = renderer(&root);
    let (mut full, full_memory) = allocation_probe::observe(|| {
        let mut host = root.sampler_native_advertisement().unwrap();
        credentials.append_to_advertisement(&mut host).unwrap();
        renderer.append_to_advertisement(&mut host).unwrap();
        join.append_to_advertisement(&mut host).unwrap();
        host.capabilities
            .push(conduit_net::network_attachment_sign_local_offer(
                "fixture/network-sign".into(),
                "fixture/network-sign-implementation".into(),
                "fixture/network-sign-artifact".into(),
            ));
        host.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
        host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        host
    });
    software.append_to_advertisement(&mut full).unwrap();
    assert!(full_memory.retained_bytes() > narrow_memory.retained_bytes());
    println!(
        "native sampler scoped retained={} full+network+renderer retained={} (CPU only)",
        narrow_memory.retained_bytes(),
        full_memory.retained_bytes()
    );
    assert_eq!(narrow.host_id, full.host_id);
    assert_eq!(narrow.boot_id, full.boot_id);
    assert_eq!(narrow.offer_generation, full.offer_generation);
    let authority = grants(&root);
    let a = plan(&source, narrow.clone(), &authority).unwrap();
    let b = plan(&source, full.clone(), &authority).unwrap();
    assert_eq!(a.preparation_metadata(), b.preparation_metadata());
    for variant in 0..4 {
        let mut a = narrow.clone();
        let mut b = full.clone();
        for host in [&mut a, &mut b] {
            if variant == 0 {
                host.capabilities
                    .retain(|c| c.capability_id.as_str() != WAIT_CAPABILITY);
            }
            if variant == 1 {
                host.resources
                    .retain(|r| r.pool_id.as_str() != "fixture/wait-pool");
            }
            if variant == 2 {
                host.resources
                    .iter_mut()
                    .find(|r| r.pool_id.as_str() == "fixture/wait-pool")
                    .unwrap()
                    .capacity_units = 0;
            }
        }
        let grants = if variant == 3 {
            &[][..]
        } else {
            &authority[..]
        };
        assert_eq!(
            format!("{:?}", plan(&source, a, grants).err().unwrap()),
            format!("{:?}", plan(&source, b, grants).err().unwrap())
        );
    }
    root.boot = "stale/boot".into();
    assert!(root.sampler_native_advertisement().is_err());
    root.boot = narrow.boot_id.clone();
    let retained = root.wait.take();
    let missing = root.sampler_native_advertisement().unwrap();
    assert!(missing
        .capabilities
        .iter()
        .all(|c| c.capability_id.as_str() != WAIT_CAPABILITY));
    root.wait = retained;
    assert_eq!(
        root.pulse.as_ref().unwrap().native_identity(),
        &pulse::identity()
    );
    assert_eq!(effects.get(), 0);
}

#[test]
fn display_projection_preserves_exact_offers_and_ordinary_plan_without_unused_metadata() {
    use conduit_environment_make::{DisplayOutputSource, DisplaySourceMaterial};
    let (mut root, effects) = root();
    // These real initialized inert owners model the unrelated providers discarded
    // by the former full-Root bootstrap. No claim or provider operation occurs.
    let (credentials, join) = unsafe {
        (
            credential_installation::ReadyCredentialBase::new(
                network_id(&root, "credentials"),
                Inert,
            )
            .unwrap(),
            ReadyNetworkJoinBase::new(network_id(&root, "network"), 1, Inert).unwrap(),
        )
    };
    let renderer = renderer(&root);
    let (projected, scoped_memory) =
        allocation_probe::observe(|| root.display_native_advertisement().unwrap());
    let (baseline, old_memory) = allocation_probe::observe(|| {
        let mut full = root.sampler_native_advertisement().unwrap();
        credentials.append_to_advertisement(&mut full).unwrap();
        renderer.append_to_advertisement(&mut full).unwrap();
        join.append_to_advertisement(&mut full).unwrap();
        full.capabilities
            .push(conduit_net::network_attachment_sign_local_offer(
                "fixture/network-sign".into(),
                "fixture/network-sign-implementation".into(),
                "fixture/network-sign-artifact".into(),
            ));
        full.bases = vec![];
        full.resources = vec![];
        full.capabilities = vec![];
        root.display.append_to_advertisement(&mut full).unwrap();
        full.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
        full.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        full
    });
    assert_eq!(projected, baseline);
    assert!(old_memory.peak_bytes > scoped_memory.peak_bytes);
    println!(
        "display bootstrap CPU scoped peak={} retained={}, old full/discard peak={} retained={}",
        scoped_memory.peak_bytes,
        scoped_memory.retained_bytes(),
        old_memory.peak_bytes,
        old_memory.retained_bytes()
    );
    let source = DisplayOutputSource::compile().unwrap();
    let material = DisplaySourceMaterial::prepare(&source).unwrap();
    let mut software = material
        .prepare_compact(root.host.clone(), root.boot.clone())
        .unwrap();
    let mut hosts = [projected, baseline];
    for host in &mut hosts {
        host.capabilities.extend(
            software
                .current_offers(&root.host, &root.boot, host.offer_generation)
                .unwrap()
                .iter()
                .map(|b| serde_json::from_slice::<c::CapabilityOffer>(b).unwrap()),
        );
    }
    let grant = c::AuthorityGrant {
        grant_id: "fixture/display".into(),
        contract_id: conduit_tm1637_wire::installation::WRITE_AUTHORITY.into(),
        host_call_contract_id: conduit_tm1637_wire::WRITE_CALL.into(),
        subject_kind: c::kind_id(conduit_tm1637_wire::WRITE_KIND),
        host_id: root.host.clone(),
        boot_id: root.boot.clone(),
        capability_id: conduit_tm1637_wire::installation::WRITE_CAPABILITY.into(),
    };
    let run = |host: &c::HostAdvertisement, grants: &[c::AuthorityGrant]| {
        let choices = conduit_planner::default_expanded_placements(
            &source.plot.expanded,
            std::slice::from_ref(host),
        )?;
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
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: conduit_tm1637_wire::MAXIMUM_BYTES,
                    },
                )
            })
            .collect();
        let mut fore = BTreeMap::new();
        for (direction, bindings, bytes) in [
            (
                c::PortDirection::Input,
                &source.plot.input_bindings,
                conduit_tm1637_wire::MAXIMUM_BYTES,
            ),
            (c::PortDirection::Output, &source.plot.output_bindings, 64),
        ] {
            for binding in bindings {
                fore.insert(
                    conduit_planner::ForeBoundaryKey {
                        direction,
                        front_port_id: binding.front_port_id.clone(),
                        track: binding.track,
                    },
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: bytes,
                    },
                );
            }
        }
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        conduit_planner::plan_expanded_authoring_with_connection_limits(
            &source.plot,
            std::slice::from_ref(host),
            &choices,
            &["conduit.base/local@1".into(), "fixture/base".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &empty,
                line_candidates: &lines,
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_tm1637_wire::MAXIMUM_BYTES,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &queues,
            &fore,
        )
    };
    let grants = [grant];
    assert_eq!(
        run(&hosts[0], &grants).unwrap(),
        run(&hosts[1], &grants).unwrap()
    );
    for variant in 0..3 {
        let mut changed = hosts.clone();
        for host in &mut changed {
            match variant {
                0 => host.resources.clear(),
                1 => host
                    .capabilities
                    .retain(|o| o.kind_id.as_str() != conduit_tm1637_wire::WRITE_KIND),
                _ => (),
            }
        }
        let authority = if variant == 2 { &[][..] } else { &grants[..] };
        assert_eq!(
            format!("{:?}", run(&changed[0], authority).unwrap_err()),
            format!("{:?}", run(&changed[1], authority).unwrap_err())
        );
    }
    root.boot = "stale/boot".into();
    assert!(root.display_native_advertisement().is_err());
    assert_eq!(effects.get(), 0);
}
