//! Retained electrical custody across ordinary page replacements. Only actual
//! terminal withdrawal blanks the module. CPU/evidence ownership remains finite.
use super::*;
#[path = "native_mask_frame.rs"]
mod frame;
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
            presentation: None,
            previous: None,
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
        self.continuous_frame(&active, origin, &context, leases, false)?;
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
                let displayed = self
                    .handoff
                    .as_mut()
                    .ok_or("mask-handoff")?
                    .presentation
                    .as_mut()
                    .and_then(RetainedPresentation::presented_mut)
                    .ok_or("mask-displayed")?;
                let value = self
                    .session
                    .store_host_value_encoded(8192, |out| {
                        call.closed_presented_encoded(
                            displayed,
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
            self.continuous_frame(&next_active, now, &context, leases, true)?;
            let handoff = self.handoff.as_mut().ok_or("mask-handoff")?;
            handoff.successor = None;
            let receipt = handoff.receipt.as_ref().ok_or("mask-replacement-receipt")?;
            let previous = handoff
                .previous
                .as_mut()
                .ok_or("mask-previous-presentation")?;
            let value = self
                .session
                .store_host_value_encoded(8192, |out| {
                    receipt.terminal_encoded::<()>(
                        previous,
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
}
