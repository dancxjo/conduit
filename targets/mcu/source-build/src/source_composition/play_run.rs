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
enum Delivery {
    Empty,
    Queued {
        ticket: FrameTicket,
        bytes: [u8; FRAME_BYTES],
        length: usize,
    },
    Admitted {
        ticket: FrameTicket,
    },
    Completed {
        ticket: FrameTicket,
        sign: c::SignId,
    },
    Failed,
}
struct SampleObserver<'a> {
    inspection: &'a State<3>,
}
impl<'r, 'f> Observer<crate::play::SamplingKernel<'r, 'f>> for SampleObserver<'_> {
    type Error = ();
    fn before_step(&mut self, _: &mut crate::play::SamplingKernel<'r, 'f>) -> Result<(), ()> {
        Ok(())
    }
    fn completed(
        &mut self,
        kernel: &mut crate::play::SamplingKernel<'r, 'f>,
        _: Dispatch,
    ) -> Result<(), ()> {
        if let Some(event) = kernel.signs().events().last() {
            self.inspection.record(0, event).map_err(|_| ())?;
        }
        Ok(())
    }
}
struct OutputObserver<'a> {
    delivery: &'a RefCell<Delivery>,
    inspection: &'a State<3>,
    scopes: &'a Scopes,
    evidence: &'a StartedEvidence,
    boundary: OwnedValueBoundary,
    sequence: u64,
    write_node: k::NodeId,
}
impl<'r, 'f> Observer<crate::play::OutputKernel<'r, 'f>> for OutputObserver<'_> {
    type Error = ();
    fn before_step(&mut self, kernel: &mut crate::play::OutputKernel<'r, 'f>) -> Result<(), ()> {
        let mut state = self.delivery.borrow_mut();
        if let Delivery::Queued {
            ticket,
            bytes,
            length,
        } = &*state
        {
            let ticket = *ticket;
            match kernel
                .admit_remote_input(
                    self.boundary.endpoint,
                    self.boundary.cord,
                    self.sequence,
                    &bytes[..*length],
                )
                .map_err(|_| ())?
            {
                RemoteIngressOutcome::Accepted { .. } => {
                    self.sequence = self.sequence.checked_add(1).ok_or(())?;
                    *state = Delivery::Admitted { ticket };
                }
                RemoteIngressOutcome::Full { .. } => {}
            }
        }
        Ok(())
    }
    fn completed(
        &mut self,
        kernel: &mut crate::play::OutputKernel<'r, 'f>,
        dispatch: Dispatch,
    ) -> Result<(), ()> {
        let Dispatch::Completed { request, outcome } = dispatch else {
            return Ok(());
        };
        if let Some(event) = kernel.signs().events().last() {
            self.inspection.record(2, event).map_err(|_| ())?;
        }
        if request.node != self.write_node || request.call != k::HostCallId(0) {
            return Err(());
        }
        let mut state = self.delivery.borrow_mut();
        let Delivery::Admitted { ticket } = &*state else {
            return Err(());
        };
        let ticket = *ticket;
        if outcome.disposition != k::HostCallDisposition::Completed || outcome.failure.is_some() {
            *state = Delivery::Failed;
            return Err(());
        }
        // This independently checked output Source has one input Frame and one
        // write path. Only one Frame is admitted at a time, so this completion
        // belongs to this exact ticket, never to carrier delivery or another Play.
        let sign = kernel_sign::host_call(
            self.evidence,
            self.scopes.fragment(2),
            kernel.signs(),
            request.node,
            request.request,
            kernel_sign::Phase::Completed,
            64,
        )
        .map_err(|_| ())?;
        *state = Delivery::Completed { ticket, sign };
        Ok(())
    }
}
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

#[allow(clippy::too_many_arguments)]
pub async fn run<'r, 'f>(
    mut sampling: Box<crate::play::SamplingKernel<'r, 'f>>,
    mut sampling_calls: NativeCalls<WAIT_SLOTS>,
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
) -> ! {
    let inspection = State::<3>::new(&evidence);
    let delivery = RefCell::new(Delivery::Empty);
    let mut sample_observer = SampleObserver {
        inspection: &inspection,
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
    };
    let mut mask = MaskSession::bind_started(
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
    .expect("exact prepared Mask session");
    let start_sign = kernel_sign::authenticated_start(&evidence, scopes.fragment(1), 64)
        .expect("actual authenticated Start causal identity");
    let cancelled = || false;
    let yield_to_host = Yield;
    let sample = async {
        let result = Runner::new(
            &mut *sampling,
            &mut sample_observer,
            &yield_to_host,
            &cancelled,
        )
        .run(&mut sampling_calls)
        .await;
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
        .run(&mut output_calls)
        .await;
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
        // Finite real evidence retention; capacity pressure stops new Face
        // invocations, never silently discards mandatory prior Signs. Retained
        // timed pages and independent sampling continue under original ownership.
        let mut archive = heapless::Vec::<(u32, k::KernelEvent), 128>::new();
        // Invocation ordinal, exact retention gap, decisions, logical count,
        // and range in the flat event archive are retained together.
        let mut invocations =
            heapless::Vec::<(u32, Option<k::SignRetentionGap>, u32, u64, usize, usize), 16>::new();
        let mut invocation = 0u32;
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
                Some(Delivery::Failed) => break,
                _ => {}
            }
            if drained {
                let count = mask.drained_event_count().expect("normally drained Mask");
                if archive.capacity() - archive.len() >= count && !invocations.is_full() {
                    let signs = conduit_seven_segment_mask::native_kernel::prepare_signs()
                        .expect("pre-admitted fixed Sign capacity");
                    match mask.rearm(signs) {
                        Ok(previous) => {
                            invocations
                                .push((
                                    invocation,
                                    previous.signs.retention_gap(),
                                    previous.decisions,
                                    previous.logical_invocations,
                                    archive.len(),
                                    count,
                                ))
                                .expect("preflighted invocation evidence retention");
                            for event in previous.signs.events() {
                                archive
                                    .push((invocation, event))
                                    .expect("preflighted evidence retention");
                            }
                            invocation = invocation.checked_add(1).expect("finite archive ordinal");
                            drained = false;
                            active = false;
                        }
                        Err(reason) => {
                            let _ = inspection.observe_refusal(1, "rearm", &reason);
                            break;
                        }
                    }
                } else {
                    let _ = inspection.observe(1, Observation::EvidenceCapacity);
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
            core::hint::black_box(&invocations);
            core::hint::black_box(&mask);
            embassy_time::Timer::after_secs(1).await;
        }
    };
    let inspect = async {
        let mut response = [0u8; crate::workset::renderer::RENDERER_FACE_BYTES as usize];
        loop {
            // The original complete arena remains reserved throughout local
            // execution, including carrier interruptions and native failures.
            core::hint::black_box(&arena);
            link.connected().await;
            let _ = conduit_mcu_host_runtime::execution_inspection::serve_with(
                &inspection,
                link,
                &mut response,
                |request, output| {
                    #[cfg(target_arch = "xtensa")]
                    if request == b"P" {
                        use core::fmt::Write;
                        #[derive(serde::Serialize)]
                        struct CaptureReport<'a> {
                            schema: &'static str,
                            assurance: &'static str,
                            phase: &'a str,
                            completed_bits: u8,
                            completed_octets: u8,
                            elapsed_us: u32,
                        }
                        let Some(capture) = conduit_esp32_electrical::last_capture_observation()
                        else {
                            return Err(());
                        };
                        let mut phase = heapless::String::<24>::new();
                        write!(&mut phase, "{:?}", capture.phase).map_err(|_| ())?;
                        return serde_json_core::to_slice(
                            &CaptureReport {
                                schema: "conduit.host/pulse-capture-inspection@1",
                                assurance: "unsigned-inspection",
                                phase: phase.as_str(),
                                completed_bits: capture.completed_bits,
                                completed_octets: capture.completed_octets,
                                elapsed_us: capture.elapsed_us,
                            },
                            output,
                        )
                        .map(Some)
                        .map_err(|_| ());
                    }
                    if request != b"F" {
                        return Ok(None);
                    }
                    face.with_retained(|current, status| {
                        #[derive(serde::Serialize)]
                        struct Report<'a> {
                            schema: &'static str,
                            assurance: &'static str,
                            source_sequence: u64,
                            publisher_retained: bool,
                            refusal: Option<&'static str>,
                            face: &'a conduit_face::Face,
                        }
                        serde_json_core::to_slice(
                            &Report {
                                schema: "conduit.host/retained-face-inspection@1",
                                assurance: "unsigned-inspection",
                                source_sequence: status.source_sequence,
                                publisher_retained: status.publisher_retained,
                                refusal: status.refusal,
                                face: current,
                            },
                            output,
                        )
                        .map(Some)
                        .map_err(|_| ())
                    })
                    .map_err(|_| ())?
                },
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
    poll_fn(|cx| {
        let _ = sample.as_mut().poll(cx);
        let _ = output.as_mut().poll(cx);
        let _ = mask.as_mut().poll(cx);
        let _ = inspect.as_mut().poll(cx);
        Poll::<()>::Pending
    })
    .await;
    unreachable!()
}
