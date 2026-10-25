//! Attended, two-frame physical check alongside the native sampler. The owner
//! must include both ordinary Plans in the same Body realization. This does not
//! claim admission of an ordinary timed Mask Plot or continuous output.
use super::NativeRoot;
use crate::{native_owner_control as owner, native_owner_receipt as ack};
use conduit_core as c;
use conduit_environment_runtime::display_output::AssignedDisplayScope;
impl NativeRoot {
    pub fn stage_check_display_assignment(
        &mut self,
        wire: &[u8],
    ) -> Result<c::AssignedPlanView, &'static str> {
        let stats = esp_alloc::HEAP.stats();
        self.display.stage_assignment(
            wire,
            AssignedDisplayScope {
                host: &self.host,
                boot: &self.boot,
                generation: c::OfferGeneration(self.offer_generation),
            },
            stats.size.saturating_sub(stats.current_usage),
        )
    }
    pub fn display_check_seal(&self) -> Option<[u8; 32]> {
        self.display.assignment().map(|s| s.seal())
    }
    pub fn authenticate_display_owner<'a>(
        &mut self,
        request: owner::Request<'a>,
        signature: &[u8; 64],
    ) -> Result<owner::AuthenticatedRequest<'a>, owner::Refusal> {
        let generation =
            crate::native_owner_reservation::OwnerReservation::authentication_generation(
                request.operation,
                self.offer_generation,
                self.display_reservation.as_ref(),
            );
        self.owner_control.authenticate(
            request,
            signature,
            self.host.as_str(),
            self.boot.as_str(),
            generation,
        )
    }
    pub async fn apply_display_check(
        &mut self,
        token: owner::AuthenticatedRequest<'_>,
    ) -> Result<Option<ack::Receipt>, &'static str> {
        use owner::Operation as O;
        let operation = token.operation();
        if operation == O::Reserve {
            if self.display_reservation.is_some() {
                return Err("display-already-reserved");
            }
            let request = token.request();
            let sampler = self
                .owner_reservation
                .as_ref()
                .ok_or("sampler-unreserved")?;
            if request.body != sampler.body
                || request.wake != sampler.wake
                || request.body_plan != sampler.body_plan
                || request.body_play != sampler.body_play
            {
                return Err("display-body-scope");
            }
            let staged = self
                .display
                .assignment()
                .ok_or("display-assignment-unavailable")?;
            self.display_reservation = Some(
                crate::native_owner_reservation::OwnerReservation::reserve_display(
                    token,
                    &staged.admitted,
                    &staged.seal(),
                    &mut self.issuer,
                )?,
            );
            return Ok(None);
        }
        let request = token.into_request();
        let command = request.transcript().map_err(|_| "command-scope")?;
        let reservation = self
            .display_reservation
            .as_ref()
            .ok_or("display-unreserved")?;
        reservation.check_request(&request, operation)?;
        let state = match operation {
            O::Prepare => {
                if self.display.native_bound() || self.display.kernel_prepared() {
                    return Err("display-already-prepared");
                }
                let withdrawn = self
                    .offer_generation
                    .checked_add(1)
                    .ok_or("offer-generation-exhausted")?;
                let stats = esp_alloc::HEAP.stats();
                self.display.bind_native(
                    &mut self.issuer,
                    reservation,
                    crate::native_display::CONSTRUCTION_FRAMES,
                    stats.size.saturating_sub(stats.current_usage),
                )?;
                self.offer_generation = withdrawn;
                let stats = esp_alloc::HEAP.stats();
                self.display.prepare_kernel(
                    AssignedDisplayScope {
                        host: &self.host,
                        boot: &self.boot,
                        generation: c::OfferGeneration(reservation.offer_generation),
                    },
                    crate::native_display::CONSTRUCTION_FRAMES,
                    stats.size.saturating_sub(stats.current_usage),
                )?;
                let stats = esp_alloc::HEAP.stats();
                self.display
                    .prepare_context(stats.size.saturating_sub(stats.current_usage))?;
                return Ok(None);
            }
            O::Start => {
                let stats = esp_alloc::HEAP.stats();
                if stats.size.saturating_sub(stats.current_usage) < 8192 {
                    return Err("display-task-capacity");
                }
                self.display
                    .register_check_task(
                        self.worker_spawner.ok_or("worker-executor-unavailable")?,
                        self.face_reader.ok_or("face-reader-unavailable")?,
                        reservation,
                    )
                    .await?;
                self.display.accept_start()?;
                ack::State::StartedHeld
            }
            O::Release => {
                self.display.release()?;
                ack::State::Released
            }
            O::Cancel => {
                self.display.cancel_held().await?;
                ack::State::Quiesced
            }
            O::Retire => {
                self.display
                    .retire_held(&mut self.issuer, &reservation.active)?;
                ack::State::Retired
            }
            O::Inspect => return Ok(None),
            O::Reserve => unreachable!(),
        };
        Ok(Some(ack::Receipt {
            command,
            state,
            snapshot: None,
        }))
    }
    pub fn encode_display_check_reply(
        &self,
        output: &mut [u8],
        authenticated: bool,
        refusal: Option<&str>,
        receipt: Option<&ack::Receipt>,
    ) -> Result<usize, ()> {
        #[derive(serde::Serialize)]
        struct Reply<'a> {
            event: &'static str,
            authenticated: bool,
            refusal: Option<&'a str>,
            sequence: u64,
            worker: Option<crate::native_display::DisplayTaskStatus>,
            acknowledgement: Option<&'a ack::Receipt>,
            acknowledgement_signature: Option<&'a [u8]>,
            physical_mask_plot_installed: bool,
        }
        let signature = receipt.map(|r| self.sign_owner_receipt(r));
        serde_json_core::to_slice(
            &Reply {
                event: "native-display-check-control",
                authenticated,
                refusal,
                sequence: self.owner_control.last_sequence(),
                worker: self.display.task_status(),
                acknowledgement: receipt,
                acknowledgement_signature: signature.as_ref().map(|s| s.as_slice()),
                physical_mask_plot_installed: false,
            },
            output,
        )
        .map_err(|_| ())
    }
}
