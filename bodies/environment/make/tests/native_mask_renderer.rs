//! Native binder and real wire engine with fake electrical pins. No HIL claim.
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/mask.rs"]
mod fixture;
#[path = "support/native_mask.rs"]
mod native;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_environment_runtime::mask_renderer::*;
use conduit_face as f;
use conduit_kernel::{HostCallId, RequestId};
use std::rc::Rc;
fn planned(ready: &ReadyRenderer<native::Provider>) -> f::PlannedMaskPlot {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let mut host = fixture::host();
    host.capabilities
        .retain(|o| o.kind_id.as_str() != f::RENDERER_KIND);
    host.resources.clear();
    ready.append_to_advertisement(&mut host).unwrap();
    source.plan(&host, 8192, &[native::grant(ready)]).unwrap()
}
fn encode(part: f::MaskJsonPart<'_>, out: &mut [u8]) -> Result<usize, f::MaskJsonRefusal> {
    serde_json_core::to_slice(&part, out).map_err(|_| f::MaskJsonRefusal::Capacity)
}
#[test]
fn real_renderer_binding_acknowledges_show_then_explicitly_blanks_before_retirement() {
    for failure in [
        None,
        Some("nack"),
        Some("cancel"),
        Some("withdraw-nack"),
        Some("withdraw-cancel"),
    ] {
        let pins = Rc::new(native::Pins::default());
        let (ready, preparation) = allocation_probe::observe(|| native::ready(pins.clone()));
        let planned = planned(&ready);
        let active = c::bind_active_play(
            &planned.plan.plan_id,
            &ready.identity().host_id,
            &ready.identity().boot_id,
            1,
        );
        let mut owner = Some(ready);
        assert!(native::bind(&planned, &mut owner, &active, true).is_err());
        assert!(owner.is_some());
        assert_eq!(pins.edges.get(), 0);
        let (mut call, node) = native::bind(&planned, &mut owner, &active, false).unwrap();
        assert!(owner.is_none());
        assert_eq!(pins.edges.get(), 0);
        let plan_wire = serde_json::to_vec(&planned).unwrap();
        let (context, context_memory) = allocation_probe::observe(|| {
            f::PreparedMaskShowContext::from_wire(plan_wire.clone().into_boxed_slice(), 8192, 65536)
                .unwrap()
        });
        let face = fixture::face();
        let face_wire = serde_json::to_vec(&face).unwrap();
        let (mut presentation, occurrence_memory) = allocation_probe::observe(|| {
            call.prepare_presentation(PresentationBasis {
                face: &face,
                face_wire: &face_wire,
                context: &context,
                elapsed_ms: 0,
                front: "environment".into(),
                target: "fixture/display".into(),
                prepared_sign: "fixture/prepared".into(),
                available_sign: "fixture/ack".into(),
            })
            .unwrap()
        });
        if failure.is_none() {
            let prepared = f::MaskShow::prepared(
                &planned,
                &face,
                active.clone(),
                "environment".into(),
                "fixture/display".into(),
                "fixture/prepared".into(),
            )
            .unwrap();
            let available = prepared
                .transition(f::ManifestationLifecycle::Available, "fixture/ack".into())
                .unwrap();
            let closed = available
                .transition(f::ManifestationLifecycle::Closed, "fixture/closed".into())
                .unwrap();
            let failed = available
                .fail(
                    f::ManifestationFailure::DeliveryFailed,
                    "fixture/failed".into(),
                )
                .unwrap();
            for show in [&prepared, &available, &closed, &failed] {
                let bytes = serde_json::to_vec(show).unwrap();
                assert!(
                    bytes.len() <= 8192,
                    "native {:?} Show exceeds8192: {}",
                    show.show.lifecycle,
                    bytes.len()
                );
                assert_eq!(
                    serde_json::from_slice::<f::MaskShow>(&bytes).unwrap(),
                    *show
                );
            }
        }
        drop(planned);
        drop(face);
        let mut out = vec![0; 8192];
        assert!(call
            .present_encoded(
                node,
                HostCallId(0),
                RequestId(0),
                b"wrong face",
                &presentation,
                &mut out,
                encode,
                || false
            )
            .is_err());
        assert!(call
            .present_encoded(
                node,
                HostCallId(0),
                RequestId(0),
                &face_wire,
                &presentation,
                &mut out[..8],
                encode,
                || false
            )
            .is_err());
        assert_eq!(pins.edges.get(), 0);
        assert_eq!(call.state(), RendererState::Ready);
        pins.nack.set(failure == Some("nack"));
        let (result, memory) = allocation_probe::observe(|| {
            call.present_encoded(
                node,
                HostCallId(0),
                RequestId(0),
                &face_wire,
                &presentation,
                &mut out,
                encode,
                || failure == Some("cancel"),
            )
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(pins.released.get(), [true; 2]);
        if matches!(failure, Some("nack" | "cancel")) {
            assert!(result.is_err());
            assert_eq!(call.state(), RendererState::Failed);
            assert_eq!(call.inspection().lifecycle, c::CapabilityLifecycle::Revoked);
            let edges = pins.edges.get();
            assert!(call
                .present_encoded(
                    node,
                    HostCallId(0),
                    RequestId(0),
                    &face_wire,
                    &presentation,
                    &mut out,
                    encode,
                    || false
                )
                .is_err());
            assert_eq!(pins.edges.get(), edges);
            assert!(RendererCall::retire_retained(&mut Some(call)).is_err());
        } else {
            let length = result.unwrap();
            let show: f::MaskShow = serde_json::from_slice(&out[..length]).unwrap();
            assert_eq!(show.show.lifecycle, f::ManifestationLifecycle::Available);
            let closed_probe = show
                .transition(f::ManifestationLifecycle::Closed, "fixture/closed".into())
                .unwrap();
            println!(
                "native complete Show Available={} Closed={}",
                length,
                serde_json::to_vec(&closed_probe).unwrap().len()
            );
            assert_eq!(show.show.active_play_id, active.active_play_id);
            assert!(pins.edges.get() > 200);
            assert_eq!(call.state(), RendererState::Available);
            assert!(call
                .closed_encoded(&mut presentation, "fixture/closed".into(), &mut out, encode)
                .is_err());
            assert!(serde_json::to_vec(&closed_probe).unwrap().len() <= 8192);
            let mut closed_wire = vec![0; 8192];
            pins.nack.set(failure == Some("withdraw-nack"));
            let mut slot = Some(call);
            assert!(RendererCall::retire_retained(&mut slot).is_err());
            let (result, memory) = allocation_probe::observe(|| {
                slot.as_mut()
                    .unwrap()
                    .withdraw(|| failure == Some("withdraw-cancel"))
            });
            assert_eq!((memory.allocations, memory.reallocations), (0, 0));
            if matches!(failure, Some("withdraw-nack" | "withdraw-cancel")) {
                assert!(result.is_err());
                assert_eq!(slot.as_ref().unwrap().state(), RendererState::Failed);
                assert!(slot
                    .as_ref()
                    .unwrap()
                    .closed_encoded(
                        &mut presentation,
                        "fixture/closed".into(),
                        &mut closed_wire,
                        encode
                    )
                    .is_err());
                assert!(RendererCall::retire_retained(&mut slot).is_err());
                assert_eq!(pins.released.get(), [true; 2]);
                continue;
            }
            result.unwrap();
            assert_eq!(slot.as_ref().unwrap().state(), RendererState::Withdrawn);
            let length = slot
                .as_ref()
                .unwrap()
                .closed_encoded(
                    &mut presentation,
                    "fixture/closed".into(),
                    &mut closed_wire,
                    encode,
                )
                .unwrap();
            let closed: f::MaskShow = serde_json::from_slice(&closed_wire[..length]).unwrap();
            assert_eq!(closed.show.lifecycle, f::ManifestationLifecycle::Closed);
            assert_eq!(closed.show.manifestation_id, show.show.manifestation_id);
            let _ready = RendererCall::retire_retained(&mut slot).unwrap();
            assert!(slot.is_none());
        }
        println!(
            "renderer preparation retained={} peak={}; occurrence retained={} peak={}; native context retained={} peak={}",
            preparation.retained_bytes(),
            preparation.peak_bytes,
            occurrence_memory.retained_bytes(),
            occurrence_memory.peak_bytes,
            context_memory.retained_bytes(), context_memory.peak_bytes
        );
    }
}

#[test]
fn ordinary_mask_kernel_publishes_only_acknowledged_native_show_under_pressure() {
    use conduit_environment_runtime::mask_kernel::PreparedMaskKernel;
    use conduit_kernel::{self as k, scheduler::SchedulerStatus, ValueStorage};
    for nack in [false, true] {
        let pins = Rc::new(native::Pins::default());
        let mut owner = Some(native::ready(pins.clone()));
        let planned = planned(owner.as_ref().unwrap());
        let id = owner.as_ref().unwrap().identity();
        let active = c::bind_active_play(&planned.plan.plan_id, &id.host_id, &id.boot_id, 1);
        let (mut call, _) = native::bind(&planned, &mut owner, &active, false).unwrap();
        let context = f::PreparedMaskShowContext::from_wire(
            serde_json::to_vec(&planned).unwrap().into_boxed_slice(),
            8192,
            65536,
        )
        .unwrap();
        let face = fixture::face();
        let face_wire = serde_json::to_vec(&face).unwrap();
        let presentation = call
            .prepare_presentation(PresentationBasis {
                face: &face,
                face_wire: &face_wire,
                context: &context,
                elapsed_ms: 3000,
                front: "environment".into(),
                target: "fixture/display".into(),
                prepared_sign: "fixture/prepared".into(),
                available_sign: "fixture/ack".into(),
            })
            .unwrap();
        let mut prepared = PreparedMaskKernel::prepare(&planned).unwrap();
        drop(face);
        drop(planned);
        pins.nack.set(nack);
        let mut calls = 0;
        let mut drained = false;
        let mut egress = None;
        let mut output = Vec::with_capacity(8192);
        let (_, memory) = allocation_probe::observe(|| {
            prepared
                .kernel
                .admit_remote_input(prepared.face.endpoint, prepared.face.cord, 0, &face_wire)
                .unwrap();
            prepared
                .kernel
                .close_remote_input(prepared.face.endpoint, prepared.face.cord)
                .unwrap();
            for step in 0..96 {
                let status = prepared.kernel.step();
                if status.is_err() {
                    assert!(nack);
                    break;
                }
                let status = status.unwrap();
                if let Some(request) = prepared.kernel.next_host_request() {
                    calls += 1;
                    assert_eq!(calls, 1);
                    let verified = call
                        .verify_input(
                            request.node,
                            request.call,
                            request.request,
                            prepared.kernel.host_value(request.input.value).unwrap(),
                            &presentation,
                        )
                        .unwrap();
                    let encoded = prepared.kernel.store_host_value_encoded(8192, |out| {
                        call.present_verified(
                            request.node,
                            request.call,
                            request.request,
                            verified,
                            out,
                            encode,
                            || false,
                        )
                    });
                    let result = match encoded {
                        Ok(value) => k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output: Some(k::BoundedValueRef::new(value, 8192).unwrap()),
                            failure: None,
                        },
                        Err(_) => {
                            assert!(nack);
                            k::HostCallOutcome {
                                disposition: k::HostCallDisposition::Failed,
                                output: None,
                                failure: Some(k::Failure {
                                    code: k::FailureCode::HostCallFailed,
                                    detail: 1,
                                }),
                            }
                        }
                    };
                    prepared
                        .kernel
                        .complete_host_call(request.node, request.request, result)
                        .unwrap();
                }
                if let Some(offer) = prepared
                    .kernel
                    .remote_egress_offer(prepared.show.endpoint, prepared.show.cord)
                    .unwrap()
                {
                    assert!(!nack);
                    if let Some(old) = egress {
                        assert_eq!(offer, old);
                    }
                    egress = Some(offer);
                    if step < 32 {
                        assert_ne!(status, SchedulerStatus::Drained);
                    } else {
                        output.extend_from_slice(prepared.kernel.host_value(offer.value).unwrap());
                        prepared
                            .kernel
                            .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
                            .unwrap();
                        prepared
                            .kernel
                            .remote_egress_delivered(offer.endpoint, offer.cord, offer.sequence)
                            .unwrap();
                    }
                }
                if status == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(calls, 1);
        if nack {
            assert!(egress.is_none());
            assert!(output.is_empty());
            assert!(!drained);
            prepared.kernel.cancel().unwrap();
        } else {
            assert!(drained);
            let show: f::MaskShow = serde_json::from_slice(&output).unwrap();
            show.validate(&fixture::face()).unwrap();
            assert_eq!(show.show.lifecycle, f::ManifestationLifecycle::Available);
            call.withdraw(|| false).unwrap();
        }
        assert_eq!(prepared.kernel.values().used_items(), 0);
    }
}

#[test]
fn readiness_and_prepared_face_cannot_move_to_another_boot_or_semantic_value() {
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let mut foreign = fixture::host();
    foreign.boot_id = "foreign".into();
    let before = foreign.clone();
    assert!(ready.append_to_advertisement(&mut foreign).is_err());
    assert_eq!(foreign, before);
    let planned = planned(&ready);
    let active = c::bind_active_play(
        &planned.plan.plan_id,
        &ready.identity().host_id,
        &ready.identity().boot_id,
        1,
    );
    let (mut call, _) = native::bind(&planned, &mut Some(ready), &active, false).unwrap();
    let context = f::PreparedMaskShowContext::from_wire(
        serde_json::to_vec(&planned).unwrap().into_boxed_slice(),
        8192,
        65536,
    )
    .unwrap();
    let face = fixture::face();
    let mut changed = face.clone();
    changed.revision += 1;
    changed.reseal_prepared().unwrap();
    let wrong_wire = serde_json::to_vec(&changed).unwrap();
    assert!(call
        .prepare_presentation(PresentationBasis {
            face: &face,
            face_wire: &wrong_wire,
            context: &context,
            elapsed_ms: 0,
            front: "environment".into(),
            target: "fixture/display".into(),
            prepared_sign: "fixture/prepared".into(),
            available_sign: "fixture/ack".into(),
        })
        .is_err());
    let (_, memory) = allocation_probe::observe(|| call.cancel_before_delivery().unwrap());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(pins.edges.get(), 0);
    assert_eq!(call.state(), RendererState::Cancelled);
    assert!(RendererCall::retire_retained(&mut Some(call)).is_ok());
}

#[test]
fn local_verified_renderer_checks_fore_configuration_and_possession_before_transfer() {
    use conduit_plan_lowering::selected_operation::*;
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let planned = planned(&ready);
    let mut metadata = planned.plan.clone();
    let fragment_id = metadata.fragments[0].fragment_id.clone();
    let fore = metadata.fragments[0].fore_ports.clone();
    let configurations: std::collections::BTreeMap<_, _> = metadata
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .map(|gear| {
            (
                gear.placement_id.clone(),
                core::mem::take(&mut gear.configuration),
            )
        })
        .collect();
    let verified = c::verify_plan_metadata_with_configurations(metadata, |gear| {
        Ok::<_, ()>(configurations[&gear.placement_id].clone())
    })
    .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
        &verified,
        &fragment_id,
    )
    .unwrap();
    let gear = planned.show_placement();
    let active = c::bind_active_play(
        verified.plan_id(),
        &ready.identity().host_id,
        &ready.identity().boot_id,
        1,
    );
    let mut kind = f::renderer_kind();
    kind.limits = c::CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: 8192,
    };
    for failure in [
        "profile",
        "artifact",
        "base",
        "mechanism",
        "authority",
        "class",
        "limits",
    ] {
        let (table, _, claim) = native::issue(&ready, &active);
        let id = ready.identity();
        let mut expected_kind = kind.clone();
        if failure == "limits" {
            expected_kind.limits.max_queue_bytes -= 1;
        }
        let result = bind_selected_verified_configured_fore_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: &expected_kind,
                call: CALL,
                input_bytes: 8192,
                output_bytes: 8192,
                resource_bytes: 64,
            },
            &gear.configuration,
            conduit_plan_lowering::selected_assigned_operation::AssignedNativeIdentity {
                capability: CAPABILITY,
                execution_profile: if failure == "profile" {
                    "foreign/profile"
                } else {
                    PROFILE
                },
                artifact: if failure == "artifact" {
                    "foreign/artifact"
                } else {
                    id.artifact_id.as_str()
                },
                base: if failure == "base" {
                    "foreign/base"
                } else {
                    id.base_id.as_str()
                },
                base_implementation: id.base_implementation_id.as_str(),
                mechanism: if failure == "mechanism" {
                    "foreign/mechanism"
                } else {
                    ATTACHMENT
                },
                authority: if failure == "authority" {
                    "foreign/authority"
                } else {
                    AUTHORITY
                },
                resource_class: if failure == "class" {
                    "foreign/class"
                } else {
                    ATTACHMENT
                },
            },
            SelectedVerifiedForeOperationPlan {
                fore: &fore,
                operation: SelectedVerifiedOperationPlan {
                    plan: &verified,
                    fragment_id: &fragment_id,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &gear.placement_id,
                },
            },
        );
        assert!(result.is_err(), "{failure}");
        assert_eq!(pins.edges.get(), 0);
    }
    let mut owner = Some(ready);
    for failure in [
        "fore",
        "configuration",
        "lowering",
        "play",
        "resource",
        "boot",
        "envelope",
        "parameter-budget",
        "generation",
        "revoked",
        "",
    ] {
        let (mut table, handle, mut claim) = native::issue(owner.as_ref().unwrap(), &active);
        let mut wrong_fore = fore.clone();
        let mut wrong_lowered = lowered.clone();
        let mut wrong_active = active.clone();
        if failure == "fore" {
            wrong_fore[0].byte_capacity -= 1;
        }
        if failure == "lowering" {
            wrong_lowered.fore_ports[0].byte_capacity -= 1;
        }
        if failure == "play" {
            wrong_active.play_sequence += 1;
        }
        if failure == "resource" {
            claim.resource_pool_id = "foreign/pool".into();
        }
        if failure == "boot" {
            claim.boot_id = "foreign/boot".into();
        }
        if failure == "envelope" {
            claim.envelope_id = "foreign/envelope".into();
        }
        if failure == "parameter-budget" {
            claim.parameter_bytes -= 1;
        }
        if failure == "generation" {
            claim.base_provider_generation += 1;
        }
        if failure == "revoked" {
            table.revoke(&handle).unwrap();
        }
        let result = ReadyRenderer::bind_verified_retained(
            &mut owner,
            table,
            handle,
            claim,
            if failure == "configuration" {
                &[]
            } else {
                &gear.configuration
            },
            SelectedVerifiedForeOperationPlan {
                fore: &wrong_fore,
                operation: SelectedVerifiedOperationPlan {
                    plan: &verified,
                    fragment_id: &fragment_id,
                    lowered: &wrong_lowered,
                    active: &wrong_active,
                    placement_id: &gear.placement_id,
                },
            },
        );
        assert_eq!(pins.edges.get(), 0);
        if failure.is_empty() {
            assert!(result.is_ok());
            assert!(owner.is_none());
        } else {
            assert!(result.is_err(), "{failure}");
            assert!(owner.is_some(), "{failure}");
        }
    }
}

#[path = "native_mask_renderer/invocation.rs"]
mod invocation_tests;

#[path = "native_mask_renderer/leases.rs"]
mod lease_tests;

#[path = "native_mask_renderer/direct_projection.rs"]
mod direct_projection_tests;
