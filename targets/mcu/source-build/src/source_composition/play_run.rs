// Actual Started kernels, exact local Fore transfer, and independent actors.
use crate::{
    equipment::{MonotonicTimer, WAIT_SLOTS},
    play_service::Scopes,
};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_kernel::{
    self as k,
    scheduler::{
        prepared_session::{OptionalOutputStorage, OwnedValueBoundary},
        RemoteIngressOutcome, SchedulerStatus,
    },
    SignQuery,
};
use conduit_mcu_host_runtime::{
    execution_factory::Calls,
    execution_inspection::{Observation, State},
    execution_mask::{FrameTicket, RendererService},
    execution_workset::StartedEvidence,
    image_memory::ImageMemoryLease,
    kernel_sign,
    mask_session::{Boundaries, MaskSession},
    native_dispatch::Dispatch,
    native_runner::{CooperativeYield, Observer, Runner},
};
use core::cell::RefCell;
type NativeCalls<const W: usize> =
    Calls<crate::equipment::Capture, crate::equipment::Wire, MonotonicTimer, W>;
const FRAME_BYTES: usize = conduit_environment_runtime::display_packets::DISPLAY_FRAME_BYTES;
struct Yield;
impl CooperativeYield for Yield {
    async fn pause(&self) {
        embassy_time::Timer::after_millis(1).await;
    }
}
mod observers {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/source_play_observers.rs"));
}
use observers::{Delivery, OutputExit, OutputObserver, SampleObserver};
mod inspection_encoder {
    include!(concat!(env!("OUT_DIR"), "/source_play_inspection.rs"));
}
mod control {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/source_play_control.rs"));
}
pub(super) use control::ReturnControl;
fn boundary(fore: c::AssignedForeRequirement) -> OwnedValueBoundary {
    OwnedValueBoundary {
        cord: k::CordId(fore.cord()),
        endpoint: k::RemoteEndpointId(fore.endpoint()),
    }
}
async fn remain_retained() -> ! {
    loop {
        embassy_time::Timer::after_secs(60).await;
    }
}

// Keep one-time session construction scratch off every actor's live poll chain.
#[inline(never)]
fn prepare_mask<'a>(
    mask: conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel,
    renderer: RendererService<'a, crate::renderer::Clock>,
    optional: OptionalOutputStorage<6>,
) -> MaskSession<'a, crate::renderer::Clock> {
    MaskSession::bind_started(
        mask.kernel,
        renderer,
        Boundaries {
            face: boundary(mask.face),
            show: boundary(mask.show),
            interaction: boundary(mask.interaction),
        },
        16,
        optional,
    )
    .expect("exact prepared Mask session")
}

// Return directly into the retained future field; no temporary zeroed array
// or archive needs to remain on the sampling actor's synchronous call chain.
#[inline(never)]
fn inspection_buffer() -> [u8; crate::workset::renderer::RENDERER_FACE_BYTES as usize] {
    [0; crate::workset::renderer::RENDERER_FACE_BYTES as usize]
}
#[inline(never)]
fn invocation_archive() -> k::invocation_archive::InvocationArchive<64, 4, 3> {
    k::invocation_archive::InvocationArchive::new()
}
#[inline(never)]
fn inspection_state(evidence: &StartedEvidence) -> State<3> {
    State::new(evidence)
}

// Cold normal-drain transition. Keep both complete fixed Sign owners and
// begin_next's affine return scratch off every ordinary rendering poll frame.
// The session and exact finite archive remain in their original actor fields.
#[inline(never)]
fn rearm_archived(
    mask: &mut MaskSession<'_, crate::renderer::Clock>,
    archive: &mut k::invocation_archive::InvocationArchive<64, 4, 3>,
) -> Result<bool, conduit_mcu_host_runtime::mask_session::Refusal> {
    let previous = mask.drained_evidence().expect("normally drained Mask");
    if archive
        .can_append(
            previous.signs,
            previous.decisions,
            previous.logical_invocations,
        )
        .is_err()
    {
        return Ok(false);
    }
    let signs = conduit_seven_segment_mask::native_kernel::prepare_signs()
        .expect("pre-admitted fixed Sign capacity");
    let previous = mask.rearm(signs)?;
    archive
        .append(
            &previous.signs,
            previous.decisions,
            previous.logical_invocations,
        )
        .expect("preflighted exact invocation evidence retention");
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub async fn run<'r, 'f>(
    mut sampling: Box<crate::play::SamplingKernel<'r, 'f>>,
    mut sampling_calls: conduit_mcu_host_runtime::execution_factory::CallsWithOptionalAppender<
        NativeCalls<WAIT_SLOTS>,
        crate::equipment::RecordingStorage,
    >,
    mask: conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel,
    mut output: Box<crate::play::OutputKernel<'r, 'f>>,
    mut output_calls: NativeCalls<0>,
    renderer: RendererService<'_, crate::renderer::Clock>,
    optional: OptionalOutputStorage<6>,
    scopes: Scopes,
    evidence: StartedEvidence,
    arena: ImageMemoryLease<'_>,
    face: &conduit_environment_runtime::publication_face_custody::LocalPublicationFace,
    link: &mut crate::transport::Link,
    activity: &conduit_mcu_host_runtime::native_runner::NativeActivity,
    append_output: &mut [u8],
    append_status: conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus,
    mut control: Option<ReturnControl<'_, '_>>,
) -> ! {
    crate::runtime::stack_checkpoint(9);
    let inspection = inspection_state(&evidence);
    let delivery = RefCell::new(Delivery::Empty);
    let mut sample_observer = SampleObserver {
        inspection: &inspection,
        status: append_status,
    };
    let fore = crate::workset::generated::output_layout::fore();
    // The independently checked output wrapper has exactly one input. Its
    // complete Frame ABI and local endpoint are validated before Start.
    let mut inputs = fore
        .iter()
        .filter(|f| f.direction() == c::AssignedForeDirection::Input);
    let frame = *inputs.next().expect("checked output Fore");
    assert!(inputs.next().is_none(), "checked single Frame input");
    let write_node = output_calls
        .write
        .as_ref()
        .expect("Started output writer")
        .node();
    let mut output_observer = OutputObserver {
        delivery: &delivery,
        inspection: &inspection,
        scopes: &scopes,
        evidence: &evidence,
        boundary: boundary(frame),
        sequence: 0,
        write_node,
        last_outcome: None,
    };
    let mut mask = prepare_mask(mask, renderer, optional);
    let start_sign = kernel_sign::authenticated_start(&evidence, scopes.fragment(1), 64)
        .expect("actual authenticated Start causal identity");
    let cancelled = || false;
    let yield_to_host = Yield;
    let sample = async {
        let runner = Runner::new(
            &mut *sampling,
            &mut sample_observer,
            &yield_to_host,
            &cancelled,
        )
        .with_activity(activity);
        let result = if let Some(appender) = sampling_calls.appender.as_mut() {
            let mut service = conduit_mcu_host_runtime::native_runner::synchronous::Service::new(
                appender,
                append_output,
            )
            .expect("pre-Start funded append output");
            runner
                .run_with_sync_calls(&mut sampling_calls.base, &mut service)
                .await
        } else {
            runner.run(&mut sampling_calls.base).await
        };
        if let Some(event) = sampling.signs().events().last() {
            let _ = inspection.record(0, event);
        }
        if let Err(reason) = &result {
            let _ = inspection.observe_refusal(0, "runner", reason);
        }
        let _ = inspection.observe(
            0,
            if result.is_ok() {
                Observation::Drained
            } else {
                Observation::Refused
            },
        );
        remain_retained().await
    };
    let display_output = async {
        let result = Runner::new(
            &mut *output,
            &mut output_observer,
            &yield_to_host,
            &cancelled,
        )
        .with_activity(activity)
        .run(&mut output_calls)
        .await;
        if let Some(event) = output.signs().events().last() {
            let _ = inspection.record(2, event);
        }
        if let Err(reason) = &result {
            let _ = inspection.observe_refusal(
                2,
                "runner",
                &OutputExit {
                    outcome: output_observer.last_outcome,
                    refusal: reason,
                },
            );
        }
        let _ = inspection.observe(
            2,
            if result.is_ok() {
                Observation::Drained
            } else {
                Observation::Refused
            },
        );
        if result.is_err() {
            *delivery.borrow_mut() = Delivery::Failed;
        }
        remain_retained().await
    };
    let display_mask = async {
        // Full original traces remain owned. Repeated structure is stored once;
        // every request generation, remote identity and gap is reconstructible.
        let mut archive = invocation_archive();
        const _: () = assert!(
            core::mem::size_of::<k::invocation_archive::InvocationArchive<64, 4, 3>>()
                <= core::mem::size_of::<heapless::Vec<(u32, k::KernelEvent), 128>>()
                    + core::mem::size_of::<
                        heapless::Vec<
                            (u32, Option<k::SignRetentionGap>, u32, u64, usize, usize),
                            16,
                        >,
                    >()
        );
        let mut active = false;
        let mut drained = false;
        loop {
            let completed = {
                let mut state = delivery.borrow_mut();
                if matches!(*state, Delivery::Completed { .. } | Delivery::Failed) {
                    Some(core::mem::replace(&mut *state, Delivery::Empty))
                } else {
                    None
                }
            };
            match completed {
                Some(Delivery::Completed { ticket, sign }) => {
                    // SAFETY: the exact single in-flight Fore ticket has just
                    // completed its actual output-kernel electrical Host Call.
                    if let Err(reason) = unsafe { mask.electrical_completion(ticket, sign) } {
                        let _ = inspection.observe_refusal(1, "electrical-completion", &reason);
                        break;
                    }
                }
                Some(Delivery::Failed) => {
                    let _ =
                        inspection.observe_refusal(1, "output-delivery", &"output runner refused");
                    break;
                }
                _ => {}
            }
            if drained {
                match rearm_archived(&mut mask, &mut archive) {
                    Ok(true) => {
                        drained = false;
                        active = false;
                    }
                    Ok(false) => {
                        let _ = inspection.observe(1, Observation::EvidenceCapacity);
                    }
                    Err(reason) => {
                        let _ = inspection.observe_refusal(1, "rearm", &reason);
                        break;
                    }
                }
            }
            if !active && !drained && mask.face_changed().unwrap_or(false) {
                match mask.begin(start_sign.clone()) {
                    Ok(()) => active = true,
                    Err(conduit_mcu_host_runtime::mask_session::Refusal::Renderer(
                        conduit_mcu_host_runtime::execution_mask::Refusal::Face,
                    )) => {}
                    Err(reason) => {
                        let _ = inspection.observe_refusal(1, "begin", &reason);
                        break;
                    }
                }
            }
            if active && !drained {
                match mask.step(&start_sign, |_| Ok(())) {
                    Ok(SchedulerStatus::Drained) => {
                        drained = true;
                        let _ = inspection.observe(1, Observation::Drained);
                    }
                    Ok(_) => {}
                    Err(reason) => {
                        let _ = inspection.observe_refusal(1, "step", &reason);
                        break;
                    }
                }
                if let Some(event) = mask.last_event() {
                    let _ = inspection.record(1, event);
                }
            }
            if matches!(*delivery.borrow(), Delivery::Empty) {
                match mask.frame() {
                    Ok(Some(frame)) => {
                        if frame.canonical.len() > FRAME_BYTES {
                            let _ = inspection.observe(1, Observation::FrameCapacity);
                            break;
                        }
                        let mut bytes = [0; FRAME_BYTES];
                        bytes[..frame.canonical.len()].copy_from_slice(frame.canonical);
                        *delivery.borrow_mut() = Delivery::Queued {
                            ticket: frame.ticket,
                            bytes,
                            length: frame.canonical.len(),
                        };
                    }
                    Ok(None) => {}
                    Err(reason) => {
                        let _ = inspection.observe_refusal(1, "frame", &reason);
                        break;
                    }
                }
            }
            embassy_time::Timer::after_millis(10).await;
        }
        let _ = mask.cancel();
        // Keep original evidence and native ownership retained; another fragment
        // failing does not cancel the sampler or invent resource retirement.
        let _ = inspection.observe(1, Observation::Refused);
        loop {
            // The fenced Root continues owning the evidence after a local
            // refusal. Keep its bytes live across every suspension.
            core::hint::black_box(&archive);
            core::hint::black_box(&mask);
            embassy_time::Timer::after_secs(1).await;
        }
    };
    let inspect = async {
        let mut response = inspection_buffer();
        loop {
            // The original complete arena remains reserved throughout local
            // execution, including carrier interruptions and native failures.
            core::hint::black_box(&arena);
            link.connected().await;
            let _ = control::serve(
                &inspection,
                face,
                link,
                &mut response,
                &mut control,
                activity,
            )
            .await;
            link.reset();
            embassy_time::Timer::after_millis(1).await;
        }
    };
    // Fairly poll the four independent actors. A carrier failure is handled
    // inside inspection; a provider fault stays inside its own runner.
    use core::{
        future::{poll_fn, Future},
        pin::pin,
        task::Poll,
    };
    let (mut sample, mut output, mut mask, mut inspect) = (
        pin!(sample),
        pin!(display_output),
        pin!(display_mask),
        pin!(inspect),
    );
    // Separate actor polls so display/archive and carrier cold initialization
    // scratch is retired before the sampler's scalar evaluator runs.
    let (mut sample, mut output, mut mask, mut inspect) = (
        pin!(conduit_mcu_host_runtime::poll_boundary::separate(
            sample.as_mut()
        )),
        pin!(conduit_mcu_host_runtime::poll_boundary::separate(
            output.as_mut()
        )),
        pin!(conduit_mcu_host_runtime::poll_boundary::separate(
            mask.as_mut()
        )),
        pin!(conduit_mcu_host_runtime::poll_boundary::separate(
            inspect.as_mut()
        )),
    );
    crate::runtime::stack_checkpoint(10);
    poll_fn(|cx| {
        crate::runtime::stack_checkpoint(11);
        let _ = sample.as_mut().poll(cx);
        crate::runtime::stack_checkpoint(12);
        crate::runtime::stack_checkpoint(13);
        let _ = output.as_mut().poll(cx);
        crate::runtime::stack_checkpoint(14);
        crate::runtime::stack_checkpoint(15);
        let _ = mask.as_mut().poll(cx);
        crate::runtime::stack_checkpoint(16);
        crate::runtime::stack_checkpoint(17);
        let _ = inspect.as_mut().poll(cx);
        crate::runtime::stack_checkpoint(18);
        Poll::<()>::Pending
    })
    .await;
    unreachable!()
}
