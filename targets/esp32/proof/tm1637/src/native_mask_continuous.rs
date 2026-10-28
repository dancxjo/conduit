//! Retained electrical custody across ordinary page replacements. Only actual
//! terminal withdrawal blanks the module. CPU/evidence ownership remains finite.
use super::*;
use alloc::{boxed::Box, sync::Arc};

/// Fallible concrete backing for the extra predecessor log/table owner. The
/// unchanged 8KiB Root reserve and 8KiB child preparation allowance remain
/// free, plus 8KiB for overlapping predecessor lifecycle/capability metadata.
fn admit() -> Option<Box<Handoff>> {
    let layout = core::alloc::Layout::new::<Handoff>();
    let required = layout.size().checked_add(3 * 8192)?;
    if esp_alloc::HEAP.free() < required {
        return None;
    }
    // SAFETY: exact layout, checked allocation; write initializes all fields
    // before the matching Box owner is constructed. Null preserves all owners.
    let raw = unsafe { alloc::alloc::alloc(layout) }.cast::<Handoff>();
    if raw.is_null() {
        return None;
    }
    unsafe {
        raw.write(Handoff {
            previous_log: None,
            receipt: None,
            successor: None,
            refused: None,
        });
        Some(Box::from_raw(raw))
    }
}
impl NativeMaskSession {
    pub(super) fn admit_continuity(&mut self) -> bool {
        if self.handoff.is_none() {
            self.handoff = admit();
        }
        self.handoff.is_some()
    }
    pub(super) async fn run_continuous(
        &mut self,
        leases: &mut impl LeaseSource,
        evidence: &mut impl EvidenceSink,
        workspace: Option<&workspace::Gate>,
    ) -> Result<u64, &'static str> {
        let context = Arc::clone(&self.context);
        let origin = embassy_time::Instant::now().as_millis();
        let Some((mut active, table, handle, claim)) =
            leases.next(self.ready.as_ref().ok_or("mask-owner")?, origin)?
        else {
            return Ok(self.index);
        };
        self.call =
            Some(
                self.invocation
                    .bind_retained(&mut self.ready, table, handle, claim, &active)?,
            );
        let (prepared, _) = self.continuous_frame(&active, origin, &context, leases, false)?;
        let mut displayed = self
            .call
            .as_ref()
            .ok_or("mask-call")?
            .retain_presentation(prepared)
            .map_err(|_| "mask-retain-presentation")?;
        loop {
            let deadline = origin
                .checked_add(
                    (self.index + 1)
                        .checked_mul(PERIOD_MS)
                        .ok_or("mask-clock")?,
                )
                .ok_or("mask-clock")?;
            if embassy_time::Instant::now().as_millis() >= deadline {
                return Err("mask-page-deadline");
            }
            while embassy_time::Instant::now().as_millis() < deadline
                && leases.current(embassy_time::Instant::now().as_millis())
            {
                embassy_time::Timer::after_millis(25).await;
            }
            let now = embassy_time::Instant::now().as_millis();
            if !leases.current(now) || self.index + 1 >= MAXIMUM_CHILDREN {
                let call = self.call.as_mut().ok_or("mask-call")?;
                call.withdraw(|| false).map_err(|_| "mask-withdraw")?;
                let value = self
                    .session
                    .store_host_value_encoded(8192, |out| {
                        call.closed_presented_encoded(
                            &mut displayed,
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
                self.pending = Some((active, value));
                self.accept_pending(leases, evidence)?;
                return Ok(self.index);
            }
            // Preflight precedes successor issuance or transfer. No reserve or
            // quota is enlarged to force the continuing path onto a small Host.
            if esp_alloc::HEAP.free() < 2 * 8192 {
                return Err("mask-replacement-capacity");
            }
            let call = self.call.as_ref().ok_or("mask-call")?;
            let incoming = leases
                .stage_successor(call.retained_identity(), &active, now)?
                .ok_or("mask-successor-exhausted")?;
            let next_active = incoming.0.clone();
            let staged = match self.invocation.stage_replacement(call, incoming) {
                Ok(staged) => staged,
                Err((incoming, reason)) => {
                    self.handoff.as_mut().ok_or("mask-handoff")?.refused = Some(incoming);
                    return Err(reason);
                }
            };
            self.handoff.as_mut().ok_or("mask-handoff")?.successor = Some(staged);
            let old = self
                .session
                .begin_next([(); 3], Self::fresh_log()?)
                .map_err(|_| "mask-rearm")?;
            self.handoff.as_mut().ok_or("mask-handoff")?.previous_log = Some(old.signs);
            let (prepared, receipt) =
                self.continuous_frame(&next_active, now, &context, leases, true)?;
            let next_displayed = self
                .call
                .as_ref()
                .ok_or("mask-call")?
                .retain_presentation(prepared)
                .map_err(|_| "mask-retain-presentation")?;
            let handoff = self.handoff.as_mut().ok_or("mask-handoff")?;
            handoff.receipt = Some(receipt.ok_or("mask-replacement-receipt")?);
            handoff.successor = None;
            let receipt = handoff.receipt.as_ref().ok_or("mask-replacement-receipt")?;
            let value = self
                .session
                .store_host_value_encoded(8192, |out| {
                    receipt.terminal_encoded::<()>(
                        &mut displayed,
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
                .map_err(|_| "mask-replaced")?;
            self.pending = Some((active, value));
            self.accept_pending(leases, evidence)?;
            active = next_active;
            displayed = next_displayed;
            if let Some(gate) = workspace {
                if let Some(pause) =
                    gate.replaced_boundary(leases.current(embassy_time::Instant::now().as_millis()))
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
    }
    fn continuous_frame<'context>(
        &mut self,
        active: &c::ActivePlayIdentity,
        now: u64,
        context: &'context f::PreparedMaskShowContext,
        leases: &impl LeaseSource,
        replacing: bool,
    ) -> Result<
        (
            r::PreparedPresentation<'context>,
            Option<r::ReplacedRenderer>,
        ),
        &'static str,
    > {
        let mut presentation = None;
        let body = self.basis.body_id();
        let producer = &self.producer;
        let epoch = &mut self.source_epoch;
        let sequence = self.index + u64::from(replacing);
        let page_offset = (sequence % 2) * PERIOD_MS;
        let call = self.call.as_mut().ok_or("mask-child-call")?;
        let session = &mut self.session;
        let invocation = &mut self.invocation;
        let mut successor = self
            .handoff
            .as_mut()
            .and_then(|state| state.successor.as_mut());
        let mut replacement_receipt = None;
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
                        sequence,
                        8192,
                        |out| {
                            let length = serde_json_core::to_slice(face, out)
                                .map_err(|_| "mask-face-encoding")?;
                            let basis = r::PresentationBasis {
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
                            };
                            presentation = Some(
                                match successor.as_deref() {
                                    Some(next) => call.prepare_replacement(next, basis),
                                    None => call.prepare_presentation(basis),
                                }
                                .map_err(|_| "mask-presentation")?,
                            );
                            Ok::<_, &'static str>(length)
                        },
                    )
                    .map_err(|_| "mask-face-admission")?;
                Ok(())
            })??;
        let presentation = presentation.ok_or("mask-presentation")?;
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
                let input = session
                    .kernel()
                    .host_value(request.input.value)
                    .map_err(|_| "mask-input")?;
                let verified = match successor.as_deref() {
                    Some(next) => call.verify_replacement(
                        next,
                        request.node,
                        request.call,
                        request.request,
                        input,
                        &presentation,
                    ),
                    None => call.verify_input(
                        request.node,
                        request.call,
                        request.request,
                        input,
                        &presentation,
                    ),
                }
                .map_err(|_| "mask-verify-input")?;
                let value = session
                    .store_host_value_encoded(8192, |out| {
                        let cancelled =
                            || !leases.current(embassy_time::Instant::now().as_millis());
                        match successor.as_deref_mut() {
                            Some(next) => invocation
                                .replace_verified(
                                    call,
                                    next,
                                    request.node,
                                    request.call,
                                    request.request,
                                    verified,
                                    out,
                                    encode,
                                    cancelled,
                                )
                                .map(|(length, receipt)| {
                                    replacement_receipt = Some(receipt);
                                    length
                                }),
                            None => call.present_verified(
                                request.node,
                                request.call,
                                request.request,
                                verified,
                                out,
                                encode,
                                cancelled,
                            ),
                        }
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
                if emitted || offer.sequence != sequence {
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
        Ok((presentation, replacement_receipt))
    }
}
