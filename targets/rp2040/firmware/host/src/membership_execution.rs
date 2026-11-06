//! Persistent Root custody, independent of one carrier or its acknowledgments.
use super::*;
use crate::resident_preparation as resident;
use conduit_kernel::scheduler::SchedulerError;
use core::{
    cell::RefCell,
    future::{poll_fn, Future},
    pin::pin,
    task::Poll,
};
use embassy_time::{Instant, Timer};

type Terminal<'a> =
    [Option<resident::TerminalFragment<'a>>; crate::resident::generated::ENTRY_COUNT];

#[derive(Clone, Copy)]
enum Phase {
    Prepared,
    Started,
    Retired,
    Fenced,
}

impl MembershipState {
    pub(super) async fn run_retained(
        &mut self,
        link: &mut Link,
        workspace: &conduit_body::HostCommandInput<LIMIT>,
        policy: &conduit_body::HostExecutionDelegation,
    ) -> Result<(), ()> {
        let digest = workspace.payload_digest().ok_or(())?;
        if !self.resources.admissions().is_empty() {
            return Err(());
        }
        let mut inventory = resident::inventory().map_err(|_| ())?;
        let stored = resident::prepare_retained_execution_storage(
            &mut inventory,
            crate::config::PREPARATION_DELEGATION.as_ref(),
            resident::ExecutionPolicy {
                installed: Some(policy),
                active_worksets: 0,
            },
            conduit_body::NativePreparationContext {
                bootstrap: &self.seed,
                member: self.receipt.as_ref().ok_or(())?,
                actual: self.resources.host(),
                image_id: crate::config::IMAGE_ID,
                configuration_id: crate::config::CONFIGURATION_ID,
                now_millis: Instant::now().as_millis(),
            },
            workspace,
            crate::config::RESIDENT_PREPARATION_MAXIMA,
            crate::config::RESIDENT_PREPARATION_STORAGE_BYTES,
        )
        .map_err(|_| ())?;
        self.freshness = self.freshness.checked_add(1).ok_or(())?;
        let committed = stored
            .admit_observed(
                &mut self.resources,
                crate::config::RESIDENT_WORK_POOL,
                crate::config::RESIDENT_WORK_UNITS,
                self.freshness,
                crate::config::RESIDENT_PREPARATION_STORAGE_BYTES,
            )
            .map_err(|_| ())?;
        let (mut fragments, _original_stamp) = committed.into_parts();
        // Fixed inline Root storage; the checked image must fund this frame
        // separately from its heap preparation arena. No runtime growth.
        let terminal = RefCell::new(core::array::from_fn(|_| None));
        let phase = Cell::new(Phase::Prepared);
        let fault = Cell::new(None);
        let mut running = true;
        let mut started = false;
        let start_refusal = Cell::new(None);
        let mut next_tick = Instant::now() + embassy_time::Duration::from_millis(10);
        loop {
            {
                // This IO future survives ticks, including partial USB frames.
                // Its failure drops only carrier state, never the workset.
                let mut io = pin!(serve_status(
                    link,
                    &phase,
                    &fault,
                    &start_refusal,
                    digest,
                    &terminal
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
                    if !started {
                        // Recheck at the actual first decision, not before an
                        // await that might carry preparation past its deadline.
                        let check = workspace.reborrow(
                            self.receipt.as_ref().ok_or(())?,
                            self.resources.host(),
                            &self.seed.owner,
                            conduit_body::HOST_PREPARED_WORKSET_CONTRACT,
                            Instant::now().as_millis(),
                        );
                        if let Err(reason) = check {
                            start_refusal.set(Some(reason));
                            running = false;
                            phase.set(
                                if resident::retire_workset(
                                    &mut fragments,
                                    &mut terminal.borrow_mut(),
                                    &mut self.resources,
                                    conduit_core::ResourceReleaseCause::FailedStart,
                                ) {
                                    Phase::Retired
                                } else {
                                    Phase::Fenced
                                },
                            );
                            continue;
                        }
                        // This is one synchronous Root Start transition. Later
                        // attribution expiry does not become a Play lease.
                        started = true;
                        phase.set(Phase::Started);
                    }
                    let decision = resident::step_workset(&mut fragments);
                    if matches!(decision, Ok(false)) {
                        continue;
                    }
                    fault.set(decision.err());
                    running = false;
                    phase.set(
                        if resident::retire_workset(
                            &mut fragments,
                            &mut terminal.borrow_mut(),
                            &mut self.resources,
                            conduit_core::ResourceReleaseCause::Cancelled,
                        ) {
                            Phase::Retired
                        } else {
                            Phase::Fenced
                        },
                    );
                }
            }
            link.reset();
            Timer::after_millis(1).await; // malformed carrier traffic cannot spin
        }
    }
}

async fn serve_status(
    link: &mut Link,
    phase: &Cell<Phase>,
    fault: &Cell<Option<SchedulerError>>,
    start_refusal: &Cell<Option<conduit_body::HostCommandRetentionRefusal>>,
    digest: &[u8; 32],
    terminal: &RefCell<Terminal<'_>>,
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
            let records = terminal.borrow();
            let selected = if request.len() == 3 && request[0] == b'S' {
                records
                    .get(usize::from(request[1]))
                    .and_then(Option::as_ref)
                    .and_then(|f| f.retired.signs.events().nth(usize::from(request[2])))
            } else {
                None
            };
            (records.iter().flatten().count(), selected)
        };
        let mut event = heapless::String::<256>::new();
        if let Some(selected) = selected {
            write!(&mut event, "{selected:?}").map_err(|_| ())?;
        }
        let mut reason = heapless::String::<128>::new();
        if let Some(error) = fault.get() {
            write!(&mut reason, "{error:?}").map_err(|_| ())?;
        }
        let mut refused_start = heapless::String::<64>::new();
        if let Some(reason) = start_refusal.get() {
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
