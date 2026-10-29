//! Continuous renderer delivery retaining exact cold and acknowledged owners.
use super::*;
impl NativeMaskSession {
    pub(super) fn continuous_frame(
        &mut self,
        active: &c::ActivePlayIdentity,
        now: u64,
        context: &Arc<f::PreparedMaskShowContext>,
        leases: &impl LeaseSource,
        replacing: bool,
    ) -> Result<(), &'static str> {
        let body = self.basis.body_id();
        let producer = &self.producer;
        let epoch = &mut self.source_epoch;
        let sequence = self.index + u64::from(replacing);
        let page_offset = (sequence % 2) * PERIOD_MS;
        let call = self.call.as_mut().ok_or("mask-child-call")?;
        let session = &mut self.session;
        let invocation = &mut self.invocation;
        let handoff = self.handoff.as_mut().ok_or("mask-handoff")?;
        if handoff
            .presentation
            .as_ref()
            .is_some_and(|value| value.prepared().is_some())
            || handoff.previous.is_some()
            || handoff.receipt.is_some()
            || (replacing != handoff.presentation.is_some())
        {
            return Err("mask-frame-custody");
        }
        if replacing {
            handoff.previous = match handoff.presentation.take() {
                Some(RetainedPresentation::Presented(value)) => Some(value),
                other => {
                    handoff.presentation = other;
                    return Err("mask-frame-custody");
                }
            };
        }
        let successor = &mut handoff.successor;
        let presentation = &mut handoff.presentation;
        let replacement_receipt = &mut handoff.receipt;
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
                                context: Arc::as_ref(context),
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
                            let prepared = match successor.as_ref() {
                                Some(next) => call.prepare_replacement(next, basis),
                                None => call.prepare_presentation(basis),
                            }
                            .map_err(|_| "mask-presentation")?;
                            // Still cold: failure here has performed no delivery.
                            *presentation = Some(RetainedPresentation::Prepared(
                                prepared
                                    .retain_context(Arc::clone(context))
                                    .map_err(|_| "mask-presentation-context")?,
                            ));
                            Ok::<_, &'static str>(length)
                        },
                    )
                    .map_err(|_| "mask-face-admission")?;
                Ok(())
            })??;
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
                let verified = match successor.as_ref() {
                    Some(next) => call.verify_replacement(
                        next,
                        request.node,
                        request.call,
                        request.request,
                        input,
                        presentation
                            .as_ref()
                            .and_then(RetainedPresentation::prepared)
                            .ok_or("mask-presentation")?,
                    ),
                    None => call.verify_input(
                        request.node,
                        request.call,
                        request.request,
                        input,
                        presentation
                            .as_ref()
                            .and_then(RetainedPresentation::prepared)
                            .ok_or("mask-presentation")?,
                    ),
                }
                .map_err(|_| "mask-verify-input")?;
                let mut acknowledged = false;
                let value = session.store_host_value_encoded(8192, |out| {
                    let cancelled = || !leases.current(embassy_time::Instant::now().as_millis());
                    let result = match successor.as_mut() {
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
                                *replacement_receipt = Some(receipt);
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
                    };
                    acknowledged = result.is_ok();
                    result
                });
                // Preserve the exact metadata even if storing the successfully
                // acknowledged delivery result subsequently refuses.
                if acknowledged {
                    match presentation.take() {
                        Some(RetainedPresentation::Prepared(prepared)) => {
                            *presentation = Some(RetainedPresentation::Acknowledged(prepared));
                        }
                        other => {
                            *presentation = other;
                            return Err("mask-presentation");
                        }
                    }
                }
                let value = value.map_err(presentation_refusal)?;
                // Electrical ACK has occurred. Transfer its exact prepared
                // occurrence into retained custody BEFORE any later failure.
                let prepared = RetainedPresentation::take_prepared(presentation)?;
                let next = match call.retain_presentation(prepared) {
                    Ok(next) => next,
                    Err(prepared) => {
                        *presentation = Some(RetainedPresentation::Acknowledged(prepared));
                        return Err("mask-retain-presentation");
                    }
                };
                *presentation = Some(RetainedPresentation::Presented(next));
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
        Ok(())
    }
}
