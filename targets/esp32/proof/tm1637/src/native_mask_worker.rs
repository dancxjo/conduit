//! Timed ordinary Mask execution. The private Root issuer supplies every fresh
//! child lease; evidence acceptance precedes any rearm or custody release.
#[path = "native_mask_continuous.rs"]
pub(super) mod continuous;
#[path = "native_mask_workspace.rs"]
pub(crate) mod workspace;
use super::session::*;
use conduit_core as c;
use conduit_environment_runtime::mask_renderer as r;
use conduit_face as f;
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
pub const PERIOD_MS: u64 = 3000;
pub const MAXIMUM_CHILDREN: u64 = 960 * 60_000 / PERIOD_MS;
// Keep the actual refusal class: failed GPIO delivery is materially different
// from an output reservation or a capability refusal before electrical work.
#[cold]
#[inline(never)]
fn report_wire_nack(packet: u8, byte: u8) {
    // Existing wire evidence only: zero-based Source packet/byte offsets. No
    // additional pin access, retry, allocation or retained diagnostic backing.
    esp_println::println!(
        "{{\"event\":\"native-mask-wire-nack\",\"packet\":{},\"byte\":{}}}",
        packet,
        byte
    );
}
fn presentation_refusal<E>(
    error: k::scheduler::HostEncodingRefusal<r::DeliveryRefusal<E>>,
) -> &'static str {
    use k::{scheduler::HostEncodingRefusal as H, EncodedValueRefusal as V};
    use r::DeliveryRefusal as D;
    match error {
        H::Scheduler(_) => "mask-present-scheduler",
        H::Encoding(V::Storage(_)) => "mask-present-storage",
        H::Encoding(V::Length) => "mask-present-length",
        H::Encoding(V::Encoder(error)) => match error {
            D::Binding => "mask-present-binding",
            D::Lifecycle => "mask-present-lifecycle",
            D::Face => "mask-present-face",
            D::Show(_) => "mask-present-show",
            D::Encoding(_) => "mask-present-encoding",
            D::Mask(_) => "mask-present-mask",
            D::Packets(_) => "mask-present-packets",
            D::Capability(_) => "mask-present-capability",
            D::Electrical(conduit_tm1637_wire::Failure::InvalidGeometry) => {
                "mask-present-wire-geometry"
            }
            D::Electrical(conduit_tm1637_wire::Failure::Cancelled) => "mask-present-wire-cancelled",
            D::Electrical(conduit_tm1637_wire::Failure::Nack { packet, byte }) => {
                report_wire_nack(packet, byte);
                "mask-present-wire-nack"
            }
            D::Electrical(conduit_tm1637_wire::Failure::Provider(_)) => {
                "mask-present-wire-provider"
            }
        },
    }
}
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
    fn stage_successor(
        &mut self,
        native: &r::RendererIdentity,
        predecessor: &c::ActivePlayIdentity,
        now_ms: u64,
    ) -> Result<Option<Lease>, &'static str>;
    fn replaced(
        &mut self,
        native: &r::RendererIdentity,
        previous: &c::ActivePlayIdentity,
        inspection: &c::CapabilityInspection,
        current: &c::CapabilityInspection,
    ) -> Result<(), &'static str>;
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
    fn accept_replaced(
        &mut self,
        invocation: u64,
        active: &c::ActivePlayIdentity,
        replaced: &[u8],
        signs: &Log,
    ) -> Result<(), &'static str>;
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
        if self.admit_continuity() {
            esp_println::println!("{{\"event\":\"mask-continuity-admitted\",\"retained_bytes\":{},\"free_bytes\":{}}}", core::mem::size_of::<Handoff>(), esp_alloc::HEAP.free());
            let result = self.run_continuous(leases, evidence, workspace).await;
            if result.is_err() {
                self.cleanup_refusal = self.quiesce_failed().err();
                self.faulted = true;
            }
            // The continuous future is terminal; empty staging is no longer
            // needed. This does not retire any actual failed native owner.
            self.release_empty_handoff();
            return result;
        }
        esp_println::println!("{{\"event\":\"mask-continuity-refused\",\"reason\":\"retained-workspace-capacity\",\"required_free_bytes\":{},\"actual_free_bytes\":{},\"closed_page_mode\":true}}", core::mem::size_of::<Handoff>() + 3 * 8192, esp_alloc::HEAP.free());
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
        let body = self.basis.body_id();
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
                    .map_err(presentation_refusal)?;
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
            r::RendererState::Available | r::RendererState::Interrupted => {
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
        if let Some(handoff) = self
            .handoff
            .as_mut()
            .filter(|state| state.receipt.is_some())
        {
            let receipt = handoff.receipt.as_ref().ok_or("mask-replacement-receipt")?;
            if receipt.active() != active {
                return Err("mask-replacement-evidence-scope");
            }
            evidence.accept_replaced(
                self.index,
                active,
                self.session
                    .kernel()
                    .host_value(*value)
                    .map_err(|_| "mask-replaced-value")?,
                handoff.previous_log.as_ref().ok_or("mask-replaced-log")?,
            )?;
            let call = self.call.as_ref().ok_or("mask-call")?;
            leases.replaced(
                call.retained_identity(),
                active,
                receipt.inspection(),
                call.inspection(),
            )?;
            self.session
                .discard_host_value(*value)
                .map_err(|_| "mask-replaced-release")?;
            handoff.receipt = None;
            handoff.previous_log = None;
            self.pending = None;
            self.index = self
                .index
                .checked_add(1)
                .ok_or("mask-invocation-exhausted")?;
            // A retry may acknowledge old evidence after fail-closed withdrawal
            // of the successor. That does not revive an Available invocation.
            self.faulted = call.state() != r::RendererState::Available;
            return Ok(());
        }
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
