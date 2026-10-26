//! Native binder and real wire engine with fake electrical pins. No HIL claim.
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
            let mut closed_wire = vec![0; serde_json::to_vec(&closed_probe).unwrap().len()];
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
