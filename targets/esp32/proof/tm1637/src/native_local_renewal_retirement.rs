//! Consuming terminal-owner handoff. Normal completion and capacity-return
//! interruption keep separate proofs through issuer finish and canonical Lull.
use super::*;
impl NativeRoot {
    // Keep affine retirement payload temporaries out of replacement preparation's
    // async ancestor frame. Exactly one phase still advances per service tick.
    #[inline(never)]
    pub(super) fn advance_local_retirement_phase(
        &mut self,
        state: &mut Renewal,
    ) -> Result<bool, &'static str> {
        match state.phase {
            Phase::RequestInterruption => {
                // All cold control allocations and exact current Body/issued
                // scopes were checked before either cancellation signal. The
                // replacement reuses exclusive value/provider backing, so its
                // physical preparation necessarily follows actual retirement.
                if !self.local_capacity_returned() {
                    return Err("renewal-capacity-trigger-invalid");
                }
                esp_println::println!("{{\"event\":\"local-recording-recovery-requested\",\"reason\":\"{}\",\"old_sampler_sequence\":{},\"old_mask_sequence\":{}}}", state.reason.name(), state.old_sampler.play_sequence, state.old_mask.play_sequence);
                self.worker
                    .as_ref()
                    .ok_or("renewal-sampler-handle")?
                    .abort_preparation();
                self.local_mask_worker
                    .as_ref()
                    .ok_or("renewal-mask-handle")?
                    .abort_preparation();
                state.phase = Phase::AwaitInterruption;
            }
            Phase::AwaitInterruption => {
                let sampler = self
                    .worker
                    .as_ref()
                    .ok_or("renewal-sampler-handle")?
                    .status();
                let mask = self
                    .local_mask_worker
                    .as_ref()
                    .ok_or("renewal-mask-handle")?
                    .status();
                if sampler.task_active || mask.task_active {
                    return Ok(true);
                }
                if sampler.failure.is_some()
                    || sampler.cleanup_failure
                    || mask.failure.is_some()
                    || mask.cleanup_refusal.is_some()
                {
                    return Err("renewal-interruption-invalid");
                }
                // Handle retirement still consumes and checks real provider,
                // kernel, renderer and accepted-evidence owners independently.
                state.phase = Phase::RetireSampler;
            }
            Phase::RetireSampler => {
                let worker = self.worker.take().ok_or("renewal-sampler-handle")?;
                match worker.retire_with_publication() {
                    Ok((handle, retired)) => {
                        state.sampler_handle = Some(handle);
                        state.sampler = Some(retired);
                        state.phase = Phase::RetireMask;
                    }
                    Err((handle, reason)) => {
                        self.worker = Some(handle);
                        return Err(reason);
                    }
                }
            }
            Phase::RetireMask => {
                let backing = state.mask_backing.take().ok_or("renewal-mask-backing")?;
                let worker = self.local_mask_worker.take().ok_or("renewal-mask-handle")?;
                let result = match state.reason {
                    RenewalReason::NormalCompleted => worker
                        .retire_completed()
                        .map(|(handle, retired)| (handle, MaskReturn::Completed(retired))),
                    RenewalReason::CapacityReturned => {
                        if worker.status().completed == crate::native_display::MAXIMUM_CHILDREN {
                            // The final child can finish between trigger and
                            // cancellation. Require actual normal retirement
                            // in that race, never an invented interrupted count.
                            worker
                                .retire_completed()
                                .map(|(handle, retired)| (handle, MaskReturn::Completed(retired)))
                        } else {
                            worker
                                .retire_interrupted()
                                .map(|(handle, retired)| (handle, MaskReturn::Interrupted(retired)))
                        }
                    }
                };
                match result {
                    Ok((handle, retired)) => {
                        state.mask_handle = Some(handle);
                        state.mask_interrupted = matches!(&retired, MaskReturn::Interrupted(_));
                        state.mask = Some(Box::write(backing, retired));
                        state.phase = Phase::FinishMask;
                    }
                    Err((handle, reason)) => {
                        state.mask_backing = Some(backing);
                        self.local_mask_worker = Some(handle);
                        return Err(reason);
                    }
                }
            }
            Phase::FinishMask => {
                inspect_terminal_sampler_signs(
                    &state
                        .sampler
                        .as_ref()
                        .ok_or("renewal-retired-sampler")?
                        .execution,
                    state.reason,
                )?;
                let body = self.body.as_ref().ok_or("renewal-body")?;
                match state.mask.as_deref().ok_or("renewal-retired-mask")? {
                    MaskReturn::Completed(retired) => {
                        self.issuer
                            .finish_mask_play(body, &state.old_mask, retired)?
                    }
                    MaskReturn::Interrupted(retired) => {
                        self.issuer
                            .finish_interrupted_mask_play(body, &state.old_mask, retired)?
                    }
                }
                state.phase = Phase::FinishSampler;
            }
            Phase::FinishSampler => {
                self.issuer
                    .finish_native_play(
                        &state.old_sampler,
                        &state
                            .sampler
                            .as_ref()
                            .ok_or("renewal-retired-sampler")?
                            .execution
                            .providers,
                    )
                    .map_err(|_| "renewal-native-finish")?;
                state.phase = Phase::ReuniteFace;
            }
            Phase::ReuniteFace => {
                if state.face.is_none() {
                    let backing = reserve_box::<FaceReturn>()?;
                    let reader = self.face_reader.take().ok_or("renewal-face-reader")?;
                    let retired = state.sampler.take().expect("retained settled sampler");
                    state.receipt = Some(retired.execution.scope_receipt());
                    state.execution = Some(retired.execution);
                    let returned = match crate::native_face::reunite(retired.publisher, reader) {
                        Ok(face) => FaceReturn::Returned(face),
                        Err((returning, _)) => FaceReturn::Returning(returning),
                    };
                    state.face = Some(Box::write(backing, returned));
                }
                let (face, backing) = take_box(state.face.take().expect("retained Face return"));
                let returned = match face {
                    FaceReturn::Returning(returning) => match returning.retry() {
                        Ok(face) => FaceReturn::Returned(face),
                        Err((returning, reason)) => {
                            state.face =
                                Some(Box::write(backing, FaceReturn::Returning(returning)));
                            return Err(reason);
                        }
                    },
                    retained => retained,
                };
                state.face = Some(Box::write(backing, returned));
                if !matches!(state.face.as_deref(), Some(FaceReturn::Returned(_))) {
                    return Err("renewal-face-return-state");
                }
                state.phase = Phase::Lull;
            }
            Phase::Lull => {
                self.body
                    .as_mut()
                    .ok_or("renewal-body")?
                    .lull(&self.host, &self.boot, Some(&state.old_body_play))
                    .map_err(|_| "renewal-body-lull")?;
                state.phase = Phase::CheckpointLull;
            }
            Phase::CheckpointLull => {
                // First persist the Lull, or retry an archive already staged by
                // a previous failed commit. Never stage another segment while
                // its predecessor still awaits durable checkpoint/ACK.
                self.checkpoint_body()?;
                loop {
                    let sealed = self
                        .body
                        .as_mut()
                        .ok_or("renewal-body")?
                        .seal_terminal_wake_for_checkpoint()
                        .map_err(|error| match error {
                            conduit_body::BodyLifecycleSessionError::ArchivePersistenceRequired
                            | conduit_body::BodyLifecycleSessionError::Biography(
                                conduit_body::BodyBiographyError::CapacityExhausted,
                            ) => "renewal-wake-archive-capacity",
                            _ => "renewal-wake-archive-invalid",
                        })?;
                    if !sealed {
                        break;
                    }
                    self.checkpoint_body()?;
                }
                state.phase = Phase::Reclaim;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
fn inspect_terminal_sampler_signs(
    execution: &crate::native_worker::RetiredExecution<
        crate::native_root::DevicePulse<'static>,
        crate::native_wait::EspWait,
    >,
    reason: RenewalReason,
) -> Result<(), &'static str> {
    let disposition = match reason {
        RenewalReason::NormalCompleted => sampler_terminal::Disposition::Completed,
        RenewalReason::CapacityReturned => sampler_terminal::Disposition::CapacityInterrupted,
    };
    sampler_terminal::inspect::<{ conduit_environment_runtime::appliance::assigned::NODES }>(
        execution
            .material
            .kernel()
            .signs()
            .events()
            .map(|event| (event.node.0, event.kind)),
        disposition,
    )
}
