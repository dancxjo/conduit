//! Persistent custody over the existing kernels, independent of carrier life.
use crate::{
    config,
    transport::{Link, LIMIT},
};
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::KernelEvent;
use core::{
    cell::{Cell, RefCell},
    fmt::{Debug, Write},
    future::{poll_fn, Future},
    pin::pin,
    task::Poll,
};
use embassy_time::{Instant, Timer};

/// Root lifecycle hooks only. Implementations advance the production kernel;
/// none may add a scheduler, transport authority or automatic grant renewal.
pub trait OwnedWorkset {
    type StartError: Debug;
    type StepError: Debug;
    fn is_started(&self) -> bool;
    fn start(
        &mut self,
        input: &b::HostCommandInput<LIMIT>,
        context: b::NativePreparationContext<'_>,
        resources: &c::ResourceAdmissionOwner,
    ) -> Result<(), Self::StartError>;
    fn step(&mut self) -> Result<bool, Self::StepError>;
    fn retire(
        &mut self,
        resources: &mut c::ResourceAdmissionOwner,
        cause: c::ResourceReleaseCause,
    ) -> bool;
    fn terminal_count(&self) -> usize;
    fn terminal_event(&self, fragment: usize, event: usize) -> Option<KernelEvent>;
}
#[derive(Clone, Copy)]
enum Phase {
    Prepared,
    Started,
    Retired,
    Fenced,
}

pub async fn supervise<W: OwnedWorkset>(
    link: &mut Link,
    input: &b::HostCommandInput<LIMIT>,
    bootstrap: &b::MemberBootstrap<'_>,
    member: &b::MembershipCredential,
    resources: &mut c::ResourceAdmissionOwner,
    workset: &RefCell<W>,
) -> ! {
    let digest = input.payload_digest().expect("admitted retained command");
    let phase = Cell::new(if workset.borrow().is_started() {
        Phase::Started
    } else {
        Phase::Prepared
    });
    let fault = RefCell::new(None);
    let start_refusal = RefCell::new(None);
    let mut running = true;
    let mut next_tick = Instant::now() + embassy_time::Duration::from_millis(10);
    loop {
        {
            // Preserve partial IO across ticks. Losing the carrier never drops
            // or restarts the independently retained workset.
            let mut io = pin!(serve_status(
                link,
                &phase,
                &fault,
                &start_refusal,
                digest,
                workset
            ));
            loop {
                let mut tick = pin!(Timer::at(next_tick));
                let connected = poll_fn(|cx| {
                    if tick.as_mut().poll(cx).is_ready() {
                        return Poll::Ready(true);
                    }
                    if io.as_mut().poll(cx).is_ready() {
                        return Poll::Ready(false);
                    }
                    Poll::Pending
                })
                .await;
                if !connected {
                    break;
                }
                next_tick = Instant::now() + embassy_time::Duration::from_millis(10);
                if !running {
                    continue;
                }
                if !workset.borrow().is_started() {
                    let context = b::NativePreparationContext {
                        bootstrap,
                        member,
                        actual: resources.host(),
                        image_id: config::IMAGE_ID,
                        configuration_id: config::CONFIGURATION_ID,
                        now_millis: Instant::now().as_millis(),
                    };
                    let check = workset.borrow_mut().start(input, context, resources);
                    if let Err(reason) = check {
                        *start_refusal.borrow_mut() = Some(reason);
                        running = false;
                        phase.set(
                            if workset
                                .borrow_mut()
                                .retire(resources, c::ResourceReleaseCause::FailedStart)
                            {
                                Phase::Retired
                            } else {
                                Phase::Fenced
                            },
                        );
                        continue;
                    }
                    phase.set(Phase::Started);
                }
                let decision = workset.borrow_mut().step();
                if matches!(decision, Ok(false)) {
                    continue;
                }
                *fault.borrow_mut() = decision.err();
                running = false;
                phase.set(
                    if workset
                        .borrow_mut()
                        .retire(resources, c::ResourceReleaseCause::Cancelled)
                    {
                        Phase::Retired
                    } else {
                        Phase::Fenced
                    },
                );
            }
        }
        link.reset();
        Timer::after_millis(1).await;
    }
}

async fn serve_status<W: OwnedWorkset>(
    link: &mut Link,
    phase: &Cell<Phase>,
    fault: &RefCell<Option<W::StepError>>,
    start_refusal: &RefCell<Option<W::StartError>>,
    digest: &[u8; 32],
    workset: &RefCell<W>,
) -> Result<(), ()> {
    link.connected().await;
    let mut input = [0; crate::bootsel::CONTROL_FRAME_BYTES];
    loop {
        // Recovery remains the existing build-bound exchange. Other bytes
        // request inspection only; they cannot inject values, cancel or Start.
        let request = link.receive(&mut input).await?;
        // Read-only bounded inspection: S, fragment ordinal, Sign ordinal.
        // Copy just one event and end the RefCell loan before any await.
        let (retired_fragments, selected) = {
            let records = workset.borrow();
            let selected = if request.len() == 3 && request[0] == b'S' {
                records.terminal_event(usize::from(request[1]), usize::from(request[2]))
            } else {
                None
            };
            (records.terminal_count(), selected)
        };
        let mut event = heapless::String::<256>::new();
        if let Some(selected) = selected {
            write!(&mut event, "{selected:?}").map_err(|_| ())?;
        }
        let mut reason = heapless::String::<128>::new();
        if let Some(error) = fault.borrow().as_ref() {
            write!(&mut reason, "{error:?}").map_err(|_| ())?;
        }
        let mut refused_start = heapless::String::<192>::new();
        if let Some(reason) = start_refusal.borrow().as_ref() {
            write!(&mut refused_start, "{reason:?}").map_err(|_| ())?;
        }
        #[derive(serde::Serialize)]
        struct Status<'a> {
            phase: &'a str,
            payload_sha256: &'a str,
            resources: &'a str,
            ingress: &'a str,
            fault: &'a str,
            start_refusal: &'a str,
            terminal_fragments: usize,
            terminal_event: &'a str,
        }
        let (phase, resources) = match phase.get() {
            Phase::Prepared => ("prepared-awaiting-local-start", "admitted"),
            Phase::Started => ("local-kernels-started", "admitted"),
            Phase::Retired => ("terminal-evidence-retained", "released"),
            Phase::Fenced => ("retirement-fenced", "admitted"),
        };
        let mut hex = [0u8; 64];
        for (index, byte) in digest.iter().enumerate() {
            hex[index * 2] = b"0123456789abcdef"[usize::from(byte >> 4)];
            hex[index * 2 + 1] = b"0123456789abcdef"[usize::from(byte & 15)];
        }
        let mut output = [0; 1024];
        let n = serde_json_core::to_slice(
            &Status {
                phase,
                payload_sha256: core::str::from_utf8(&hex).map_err(|_| ())?,
                resources,
                ingress: "not-installed",
                fault: reason.as_str(),
                start_refusal: refused_start.as_str(),
                terminal_fragments: retired_fragments,
                terminal_event: event.as_str(),
            },
            &mut output,
        )
        .map_err(|_| ())?;
        link.send(&output[..n]).await?;
        Timer::after_millis(1).await; // bound control servicing per executor turn
    }
}
