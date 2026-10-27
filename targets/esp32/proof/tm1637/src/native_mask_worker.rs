//! Timed ordinary Mask execution. The private Root issuer supplies every fresh
//! child lease; evidence acceptance precedes any rearm or custody release.
#[path = "native_mask_workspace.rs"]
pub(crate) mod workspace;
use super::session::*;
use conduit_core as c;
use conduit_environment_runtime::mask_renderer as r;
use conduit_face as f;
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
pub const PERIOD_MS: u64 = 3000;
pub const MAXIMUM_CHILDREN: u64 = 960 * 60_000 / PERIOD_MS;
pub type Lease = (
    c::ActivePlayIdentity,
    c::BaseCapabilityTable,
    c::BaseCapabilityHandle,
    c::BaseOperationClaim,
);
pub trait LeaseSource {
    /// Root's revocable current BodyPlay/scope witness, not an identity match.
    fn current(&self, now_ms: u64) -> bool;
    fn next(&mut self, owner: &NativeReady, now_ms: u64) -> Result<Option<Lease>, &'static str>;
    fn retired(
        &mut self,
        owner: &NativeReady,
        active: &c::ActivePlayIdentity,
        inspection: &c::CapabilityInspection,
    ) -> Result<(), &'static str>;
}
pub trait EvidenceSink {
    /// Exact local consumer acceptance. On refusal worker retains both the
    /// canonical Closed value and the original kernel sign log unchanged.
    fn accept(
        &mut self,
        invocation: u64,
        active: &c::ActivePlayIdentity,
        closed: &[u8],
        signs: &Log,
    ) -> Result<(), &'static str>;
}
fn encode(part: f::MaskJsonPart<'_>, out: &mut [u8]) -> Result<usize, f::MaskJsonRefusal> {
    serde_json_core::to_slice(&part, out).map_err(|_| f::MaskJsonRefusal::Capacity)
}
impl NativeMaskSession {
    /// No ownership reset on failure. Root retains this object for explicit
    /// failed cleanup/evidence recovery instead of issuing replacement owners.
    pub async fn run(
        &mut self,
        leases: &mut impl LeaseSource,
        evidence: &mut impl EvidenceSink,
    ) -> Result<u64, &'static str> {
        self.run_with_workspace(leases, evidence, None).await
    }
    pub(crate) async fn run_with_workspace(
        &mut self,
        leases: &mut impl LeaseSource,
        evidence: &mut impl EvidenceSink,
        workspace: Option<&workspace::Gate>,
    ) -> Result<u64, &'static str> {
        if self.faulted || self.pending.is_some() || self.call.is_some() {
            return Err("mask-session-not-ready");
        }
        let origin = embassy_time::Instant::now().as_millis();
        while self.index < MAXIMUM_CHILDREN {
            let start = origin
                .checked_add(self.index.checked_mul(PERIOD_MS).ok_or("mask-clock")?)
                .ok_or("mask-clock")?;
            let deadline = start.checked_add(PERIOD_MS).ok_or("mask-clock")?;
            let now = embassy_time::Instant::now().as_millis();
            if !leases.current(now) {
                return Ok(self.index);
            }
            if now < start || now >= deadline {
                return Err("mask-page-deadline");
            }
            // Cold child metadata cannot consume Root's independent control
            // reserve. This admission precedes lease/provider transfer.
            let memory = esp_alloc::HEAP.stats();
            if memory.size.saturating_sub(memory.current_usage) < 8192 + 8192 {
                return Err("mask-child-preparation-capacity");
            }
            let Some((active, table, handle, claim)) =
                leases.next(self.ready.as_ref().ok_or("mask-owner")?, now)?
            else {
                return Ok(self.index);
            };
            if &active.plan_id != self.context.plan_id() {
                return Err("mask-child-plan");
            }
            self.call = Some(self.invocation.bind_retained(
                &mut self.ready,
                table,
                handle,
                claim,
                &active,
            )?);
            let result = self
                .render_child(&active, now, deadline, leases, evidence)
                .await;
            if result.is_err() {
                self.cleanup_refusal = self.quiesce_failed().err();
                self.faulted = true;
                return result.map(|_| self.index);
            }
            // render_child returned only after accepted Closed evidence, actual
            // lease/renderer retirement, value discard and kernel rearm. All
            // child temporaries are gone before offering this exclusive window.
            if let Some(gate) = workspace {
                if let Some(pause) =
                    gate.retired_boundary(leases.current(embassy_time::Instant::now().as_millis()))
                {
                    while pause.waiting() {
                        if !leases.current(embassy_time::Instant::now().as_millis()) {
                            gate.cancel();
                            break;
                        }
                        embassy_time::Timer::after_millis(1).await;
                    }
                }
            }
        }
        Ok(self.index)
    }
    async fn render_child(
        &mut self,
        active: &c::ActivePlayIdentity,
        now: u64,
        deadline: u64,
        leases: &mut impl LeaseSource,
        evidence: &mut impl EvidenceSink,
    ) -> Result<(), &'static str> {
        let mut presentation = None;
        let body = self.routes.body_id();
        let producer = &self.producer;
        let epoch = &mut self.source_epoch;
        let page_offset = (self.index % 2) * PERIOD_MS;
        let call = self.call.as_mut().ok_or("mask-child-call")?;
        let session = &mut self.session;
        let context = &self.context;
        let face_fore = &self.face;
        self.reader
            .with_current(now, |face, status| -> Result<(), &'static str> {
                if !status.publisher_retained
                    || status.refusal.is_some()
                    || face.basis.body_id.as_ref() != Some(body)
                    || face.basis.wake_id.as_ref() != Some(&producer.wake)
                    || face.basis.plan_id.as_ref() != Some(&producer.active.plan_id)
                    || face.basis.active_play_id.as_ref() != Some(&producer.active.active_play_id)
                    || face.basis.source_document_id.as_ref()
                        != Some(&producer.producer.source_document_id)
                    || face.basis.checked_plot_id.as_ref()
                        != Some(&producer.producer.checked_plot_id)
                    || face.basis.expanded_plot_id.as_ref()
                        != Some(&producer.producer.expanded_plot_id)
                {
                    return Err("mask-current-face");
                }
                match epoch {
                    Some((sequence, _)) if *sequence == status.source_sequence => (),
                    Some((sequence, _)) if *sequence > status.source_sequence => {
                        return Err("mask-source-regressed");
                    }
                    // A new authenticated publication starts a new occurrence.
                    // Backdate by at most one page to preserve global parity;
                    // this shortens, and never extends, its 120s Look lifetime.
                    _ => {
                        *epoch = Some((
                            status.source_sequence,
                            now.checked_sub(page_offset).ok_or("mask-clock")?,
                        ));
                    }
                }
                let elapsed = now
                    .checked_sub(epoch.as_ref().ok_or("mask-clock")?.1)
                    .ok_or("mask-clock")?;
                session
                    .admit_owned_input_encoded(
                        face_fore.endpoint,
                        face_fore.cord,
                        self.index,
                        8192,
                        |out| {
                            let length = serde_json_core::to_slice(face, out)
                                .map_err(|_| "mask-face-encoding")?;
                            presentation = Some(
                                call.prepare_presentation(r::PresentationBasis {
                                    face,
                                    face_wire: &out[..length],
                                    context,
                                    elapsed_ms: elapsed,
                                    front: "environment".into(),
                                    target: "esp32/tm1637".into(),
                                    prepared_sign: c::bind_sign(
                                        &active.host_id,
                                        &active.boot_id,
                                        Some(&active.active_play_id),
                                        0,
                                    )
                                    .sign_id,
                                    available_sign: c::bind_sign(
                                        &active.host_id,
                                        &active.boot_id,
                                        Some(&active.active_play_id),
                                        1,
                                    )
                                    .sign_id,
                                })
                                .map_err(|_| "mask-presentation")?,
                            );
                            Ok::<_, &'static str>(length)
                        },
                    )
                    .map_err(|_| "mask-face-admission")?;
                Ok(())
            })??;
        let mut presentation = presentation.ok_or("mask-presentation")?;
        session
            .close_owned_input(face_fore.endpoint, face_fore.cord)
            .map_err(|_| "mask-face-close")?;
        let mut emitted = false;
        let mut drained = false;
        for _ in 0..96 {
            let status = session.step().map_err(|_| "mask-step")?;
            if let Some(request) = session.next_host_request() {
                if request.request != call.request_sequence() {
                    return Err("mask-request-generation");
                }
                let verified = call
                    .verify_input(
                        request.node,
                        request.call,
                        request.request,
                        session
                            .kernel()
                            .host_value(request.input.value)
                            .map_err(|_| "mask-input")?,
                        &presentation,
                    )
                    .map_err(|_| "mask-verify-input")?;
                let value = session
                    .store_host_value_encoded(8192, |out| {
                        call.present_verified(
                            request.node,
                            request.call,
                            request.request,
                            verified,
                            out,
                            encode,
                            || !leases.current(embassy_time::Instant::now().as_millis()),
                        )
                    })
                    .map_err(|_| "mask-present")?;
                session
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output: Some(
                                k::BoundedValueRef::new(value, 8192)
                                    .map_err(|_| "mask-output-bound")?,
                            ),
                            failure: None,
                        },
                    )
                    .map_err(|_| "mask-completion")?;
            }
            // A drained session has already closed its owned Fore interface.
            // The Available output must have been accepted on an earlier Step.
            if status == SchedulerStatus::Drained {
                drained = true;
                break;
            }
            if let Some(offer) = session
                .owned_output(self.show.endpoint, self.show.cord)
                .map_err(|_| "mask-show-offer")?
            {
                if emitted || offer.sequence != self.index {
                    return Err("mask-show-sequence");
                }
                // This exclusively owned Fore is consumed here only after the
                // physical renderer returned its acknowledged Available Show.
                let bytes = session
                    .kernel()
                    .host_value(offer.value)
                    .map_err(|_| "mask-show-value")?;
                if bytes.is_empty() {
                    return Err("mask-empty-show");
                }
                session
                    .accept_owned_output(offer.endpoint, offer.cord, offer.sequence)
                    .map_err(|_| "mask-show-accept")?;
                emitted = true;
            }
        }
        if !drained || !emitted {
            return Err("mask-not-drained");
        }
        if embassy_time::Instant::now().as_millis() >= deadline {
            return Err("mask-page-deadline");
        }
        while embassy_time::Instant::now().as_millis() < deadline
            && leases.current(embassy_time::Instant::now().as_millis())
        {
            embassy_time::Timer::after_millis(25).await;
        }
        call.withdraw(|| false).map_err(|_| "mask-withdraw")?;
        let closed = session
            .store_host_value_encoded(8192, |out| {
                call.closed_encoded(
                    &mut presentation,
                    c::bind_sign(
                        &active.host_id,
                        &active.boot_id,
                        Some(&active.active_play_id),
                        2,
                    )
                    .sign_id,
                    out,
                    encode,
                )
            })
            .map_err(|_| "mask-closed")?;
        self.pending = Some((active.clone(), closed));
        drop(presentation);
        self.accept_pending(leases, evidence)
    }
    fn quiesce_failed(&mut self) -> Result<(), &'static str> {
        let Some(call) = self.call.as_mut() else {
            return Ok(());
        };
        match call.state() {
            r::RendererState::Ready => call
                .cancel_before_delivery()
                .map_err(|_| "mask-cancel-cleanup"),
            r::RendererState::Available => {
                call.withdraw(|| false).map_err(|_| "mask-withdraw-cleanup")
            }
            r::RendererState::Withdrawn | r::RendererState::Cancelled => Ok(()),
            r::RendererState::Failed => Err("mask-electrical-cleanup-unconfirmed"),
        }
    }
    pub fn accept_pending(
        &mut self,
        leases: &mut impl LeaseSource,
        evidence: &mut impl EvidenceSink,
    ) -> Result<(), &'static str> {
        let (active, value) = self.pending.as_ref().ok_or("mask-evidence-unavailable")?;
        let inspection = self.call.as_ref().ok_or("mask-call")?.inspection().clone();
        if inspection.completed_operations != 2
            || inspection.in_flight_operations != 0
            || inspection.lifecycle != c::CapabilityLifecycle::Revoked
        {
            return Err("mask-child-retirement-budget");
        }
        evidence.accept(
            self.index,
            active,
            self.session
                .kernel()
                .host_value(*value)
                .map_err(|_| "mask-closed-value")?,
            self.session.kernel().signs(),
        )?;
        self.ready = Some(self.invocation.retire_invocation(&mut self.call)?);
        leases.retired(
            self.ready.as_ref().ok_or("mask-owner")?,
            active,
            &inspection,
        )?;
        self.session
            .discard_host_value(*value)
            .map_err(|_| "mask-closed-release")?;
        if self.index + 1 < MAXIMUM_CHILDREN {
            let retired = self
                .session
                .begin_next([(); 3], Self::fresh_log()?)
                .map_err(|_| "mask-rearm")?;
            // The exact log was consumed above. Transfer, then release this owned
            // old log; neither cancellation nor unacknowledged output reaches here.
            // End the drained invocation borrow before rearming the same session.
            #[allow(clippy::drop_non_drop)]
            drop(retired);
        }
        // The final accepted child stays normally drained. A future consuming
        // retirement may return its storage; no unused successor is rearmed.
        self.pending = None;
        self.index = self
            .index
            .checked_add(1)
            .ok_or("mask-invocation-exhausted")?;
        self.faulted = false;
        self.cleanup_refusal = None;
        Ok(())
    }
}
