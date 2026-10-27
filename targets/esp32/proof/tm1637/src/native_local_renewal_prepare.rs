//! Cold preparation using only actual returned owner/store/task custody.
//! Kept separate from the initial boot planner and its stack-sensitive frames.
use super::*;
use crate::native_root::local_renewal;

impl NativeRoot {
    #[inline(never)]
    fn prepare_returned_sampler(
        &mut self,
        state: &mut local_renewal::Renewal,
    ) -> Result<(), &'static str> {
        if self.native_bindings.is_some() {
            return Err("renewal-native-binding-retained");
        }
        if state.kernel_backing.is_none() {
            state.kernel_backing = Some(crate::native_kernel_storage::preclaim_local_backing()?);
        }
        let plan = self.prepare_local_sampler_plan()?;
        let metadata = plan.preparation_metadata();
        let input = conduit_body::VerifiedBodyPlotPlan {
            plot: conduit_body::ResidentPlot::new(
                metadata.source_document_id.clone(),
                metadata.checked_plot_id.clone(),
            ),
            plan: &plan,
        };
        let seal = state
            .parked
            .as_ref()
            .ok_or("renewal-parked-body")?
            .prepare_verified_plot(&input)?;
        let parts = self.prepare_local_sampler_parts(&plan, true)?;
        state.sample_seal = Some(seal);
        state.sampler_parts = Some(parts);
        Ok(())
    }
    #[inline(never)]
    fn prepare_returned_mask(
        &mut self,
        state: &mut local_renewal::Renewal,
    ) -> Result<(), &'static str> {
        let mask = self.prepare_local_mask_plan()?;
        let context = Self::prepare_local_show_context(
            &mask,
            state
                .mask_values
                .as_deref_mut()
                .ok_or("renewal-mask-values")?,
            true,
        )?;
        let proposal = self.prepare_local_body_proposal(
            state.sample_seal.as_ref().ok_or("renewal-sampler-seal")?,
            &mask,
            state.parked.as_mut().ok_or("renewal-parked-body")?,
        )?;
        state.proposal = Some(proposal);
        state.context = context;
        state.mask_plan = Some(mask);
        state.sample_seal = None;
        Ok(())
    }
    pub(in crate::native_root) async fn advance_returned_local_preparation(
        &mut self,
        state: &mut local_renewal::Renewal,
    ) -> Result<(), &'static str> {
        use local_renewal::Phase;
        match state.phase {
            Phase::Reclaim => self.reclaim_local_renewal(state)?,
            Phase::PrepareSoftware => {
                if self.software.is_none() {
                    let values = state.values.take().ok_or("renewal-sampler-values")?;
                    match crate::native_software::prepare_with_returned_values_checked(values) {
                        Ok(software) => self.software = Some(software),
                        Err((error, values)) => {
                            state.values = Some(values);
                            use conduit_environment_runtime::appliance::assigned::NativePreparationRefusal as E;
                            return Err(match error {
                                E::Storage(
                                    conduit_kernel::StorageError::ItemCapacityExceeded
                                    | conduit_kernel::StorageError::ByteCapacityExceeded,
                                ) => "renewal-software-capacity",
                                _ => "renewal-software-preparation",
                            });
                        }
                    }
                }
                let key = self.restart_verifying_key();
                state.parked = Some(crate::native_restart::park_box(
                    &mut self.body,
                    &key,
                    &self.host,
                    &self.boot,
                )?);
                state.phase = Phase::PrepareSampler;
            }
            Phase::PrepareSampler => {
                self.prepare_returned_sampler(state)?;
                state.phase = Phase::PrepareMask;
            }
            Phase::PrepareMask => {
                self.prepare_returned_mask(state)?;
                state.phase = Phase::RestoreBody;
            }
            Phase::RestoreBody => {
                let key = self.restart_verifying_key();
                self.body = Some(crate::native_restart::restore_parked_retained(
                    &mut state.parked,
                    &self.host,
                    &self.boot,
                    &key,
                )?);
                state.phase = Phase::CommitProposal;
            }
            Phase::CommitProposal => {
                let _ = material::role().map_err(|_| "local-mask-role")?;
                let body = self.body.as_mut().ok_or("body-owner")?;
                let proposal = state.proposal.take().ok_or("renewal-proposal")?;
                if let Err((proposal, error)) = body.commit_verified_proposal_retained(proposal) {
                    state.proposal = Some(proposal);
                    return Err(match error {
                        conduit_body::BodyLifecycleSessionError::Biography(
                            conduit_body::BodyBiographyError::CapacityExhausted,
                        ) => "renewal-body-publication-capacity",
                        _ => "local-mask-body-publication",
                    });
                }
                state.phase = Phase::CheckpointProposal;
            }
            Phase::CheckpointProposal => {
                self.checkpoint_body()?;
                state.phase = Phase::PrepareMaskCustody;
            }
            Phase::PrepareMaskCustody => {
                if state.retained_mask.is_none() {
                    let backing = local_renewal::reserve_box::<Option<RetainedMask>>()?;
                    state.retained_mask = Some(alloc::boxed::Box::write(backing, None));
                }
                if state.retained_mask.as_ref().is_some_and(|r| r.is_none()) {
                    self.prepare_local_mask_custody(
                        state.mask_plan.as_ref().ok_or("renewal-mask-plan")?,
                        true,
                        state
                            .retained_mask
                            .as_mut()
                            .expect("preclaimed Mask custody"),
                    )?;
                }
                state.mask_parts = Some(Self::prepare_local_mask_kernel_parts(
                    state.mask_plan.as_ref().ok_or("renewal-mask-plan")?,
                    c::OfferGeneration(self.offer_generation),
                    &self.host,
                    &self.boot,
                )?);
                state.mask_plan = None;
                state.phase = Phase::Assemble;
            }
            Phase::Assemble => {
                self.assemble_returned_local(state)?;
                state.phase = Phase::Register;
            }
            Phase::Register => {
                self.register_returned_local(state).await?;
                state.phase = Phase::Start;
            }
            Phase::Start => {
                self.start_returned_local(state)?;
                state.phase = Phase::Complete;
            }
            Phase::Complete => {}
            _ => return Err("renewal-preparation-phase"),
        }
        Ok(())
    }
}

impl NativeRoot {
    #[inline(never)]
    fn replace_returned_local_face(
        &mut self,
        state: &mut local_renewal::Renewal,
    ) -> Result<(), &'static str> {
        use local_renewal::{reserve_box, take_box, FaceReturn, HistoryCustody};
        let sampler = state
            .sampler_kernel
            .as_ref()
            .ok_or("renewal-sampler-kernel")?;
        if !state.history_done {
            #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
            {
                // Reserve preservation of the exact original journal scope
                // BEFORE rotation can refuse; existing custody is never replaced.
                let preservation = if self.local_history_custody.is_none() {
                    Some(reserve_box::<HistoryCustody>()?)
                } else {
                    None
                };
                let (old, receipt) = match self.local_history_custody.as_ref() {
                    Some(pending) => (&pending.active, &pending.receipt),
                    None => (
                        &state.old_sampler,
                        state.receipt.as_ref().ok_or("renewal-settlement-receipt")?,
                    ),
                };
                match crate::native_observed_archive::rotate_local(
                    self.body.as_ref().ok_or("renewal-body")?,
                    &sampler.fragment,
                    &sampler.active,
                    c::OfferGeneration(self.offer_generation),
                    old,
                    receipt,
                ) {
                    Ok(()) => {
                        self.local_history_custody = None;
                    }
                    Err("observed-history-no-next-epoch") => {
                        if self.local_history_custody.is_none() {
                            let receipt = state
                                .receipt
                                .take()
                                .expect("preflighted original outbox settlement");
                            self.local_history_custody = Some(alloc::boxed::Box::write(
                                preservation.expect("preclaimed original outbox custody"),
                                HistoryCustody {
                                    active: state.old_sampler.clone(),
                                    receipt,
                                },
                            ));
                        }
                        state.archive_refusal = Some("observed-history-no-next-epoch");
                    }
                    Err(reason) => return Err(reason),
                }
            }
            state.history_done = true;
        }
        if matches!(state.face.as_deref(), Some(FaceReturn::Returned(_))) {
            let replacement = crate::native_face::prepare_installed_replacement(
                self.body.as_ref().ok_or("renewal-body")?,
                &sampler.fragment,
                &sampler.active,
                c::OfferGeneration(self.offer_generation),
            )?;
            let (face, backing) = take_box(state.face.take().expect("returned Face"));
            let FaceReturn::Returned(returned) = face else {
                unreachable!("exclusively preflighted returned Face");
            };
            let replaced = match returned.replace(replacement) {
                Ok((publisher, reader)) => FaceReturn::Fresh(publisher, reader),
                Err((replacing, reason)) => {
                    state.face = Some(alloc::boxed::Box::write(
                        backing,
                        FaceReturn::Replacing(replacing),
                    ));
                    return Err(reason);
                }
            };
            state.face = Some(alloc::boxed::Box::write(backing, replaced));
        }
        let (face, backing) = take_box(state.face.take().ok_or("renewal-face-transaction")?);
        let replaced = match face {
            FaceReturn::Replacing(replacing) => match replacing.retry() {
                Ok((publisher, reader)) => FaceReturn::Fresh(publisher, reader),
                Err((replacing, reason)) => {
                    state.face = Some(alloc::boxed::Box::write(
                        backing,
                        FaceReturn::Replacing(replacing),
                    ));
                    return Err(reason);
                }
            },
            retained => retained,
        };
        state.face = Some(alloc::boxed::Box::write(backing, replaced));
        if !matches!(state.face.as_deref(), Some(FaceReturn::Fresh(..))) {
            return Err("renewal-face-replacement-state");
        }
        Ok(())
    }
    #[inline(never)]
    fn assemble_returned_local(
        &mut self,
        state: &mut local_renewal::Renewal,
    ) -> Result<(), &'static str> {
        use local_renewal::FaceReturn;
        if state.sampler_kernel.is_none() {
            let output_backing = local_renewal::reserve_box::<PreparedSamplerKernel>()?;
            let parts = state
                .sampler_parts
                .as_mut()
                .ok_or("renewal-sampler-parts")?;
            let kernel = crate::native_kernel::prepare_local_direct_with_backing_retained(
                &mut parts.sampler_parts,
                &mut state.kernel_backing,
            )?;
            // The constructor consumes the actual parts only after every
            // structural/capacity check. Refusal leaves this same Box intact.
            let PreparedSamplerParts {
                native,
                active,
                fragment,
                ..
            } = *state
                .sampler_parts
                .take()
                .expect("successfully prepared sampler");
            state.sampler_kernel = Some(alloc::boxed::Box::write(
                output_backing,
                PreparedSamplerKernel {
                    kernel,
                    native,
                    active,
                    fragment,
                },
            ));
        }
        self.replace_returned_local_face(state)?;
        if state.local_preparation.is_none() && state.fresh_session.is_none() {
            let preparation_backing =
                local_renewal::reserve_box::<crate::native_display::LocalPreparation>()?;
            let leases_backing =
                local_renewal::reserve_box::<crate::native_authority::MaskLeaseIssuer>()?;
            if state.mask_values.is_none() {
                return Err("renewal-mask-values");
            }
            let first = self
                .display
                .renderer()
                .ok_or("renewal-renderer")?
                .request_sequence();
            let count = u32::try_from(crate::native_display::MAXIMUM_CHILDREN)
                .map_err(|_| "mask-request-exhausted")?;
            first.0.checked_add(count).ok_or("mask-request-exhausted")?;
            state
                .mask_parts
                .as_mut()
                .ok_or("renewal-mask-parts")?
                .set_renderer_request_sequence(first)?;
            let (parts, parts_backing) =
                local_renewal::take_box(state.mask_parts.take().ok_or("renewal-mask-parts")?);
            let values = state
                .mask_values
                .take()
                .expect("preflighted returned Mask values");
            let kernel = match parts.finish_with_fixed_storage_retained(values) {
                Ok(kernel) => kernel,
                Err((parts, values, reason)) => {
                    state.mask_parts = Some(alloc::boxed::Box::write(parts_backing, parts));
                    state.mask_values = Some(values);
                    return Err(reason);
                }
            };
            let retained = state
                .retained_mask
                .as_mut()
                .and_then(|r| r.take())
                .expect("prepared returned Mask custody");
            let RetainedMask {
                leases,
                invocation,
                routes,
                wardrobe,
                wake,
            } = retained;
            state.fresh_leases = Some(alloc::boxed::Box::write(leases_backing, leases));
            let sampler = state
                .sampler_kernel
                .as_ref()
                .expect("retained sampler kernel");
            let reader = match state.face.as_deref() {
                Some(FaceReturn::Fresh(_, reader)) => reader.lease(),
                _ => unreachable!("preflighted fresh Face"),
            };
            state.local_preparation = Some(alloc::boxed::Box::write(
                preparation_backing,
                crate::native_display::LocalPreparation {
                    context: state
                        .context
                        .take()
                        .expect("prepared returned Show context"),
                    kernel,
                    invocation,
                    routes,
                    wardrobe,
                    producer: crate::native_display::FaceScope {
                        producer: crate::native_software::source_identity(),
                        active: sampler.active.clone(),
                        wake,
                    },
                    reader,
                },
            ));
        }
        if let Some(preparation) = state.local_preparation.take() {
            let (preparation, backing) = local_renewal::take_box(preparation);
            if let Err((preparation, reason)) = self.display.retain_local_retained(preparation) {
                state.local_preparation = Some(alloc::boxed::Box::write(backing, preparation));
                return Err(reason);
            }
            drop(backing);
            state.fresh_session = Some(self.display.take_local()?);
        }
        let evidence_backing =
            local_renewal::reserve_box::<crate::native_mask_evidence::Consumer>()?;
        let (face, _backing) =
            local_renewal::take_box(state.face.take().ok_or("renewal-fresh-face")?);
        let FaceReturn::Fresh(publisher, reader) = face else {
            unreachable!("exclusively preflighted fresh Face");
        };
        state.registered_publisher = Some(publisher);
        self.face_reader = Some(reader);
        let PreparedSamplerKernel {
            kernel,
            native,
            active,
            fragment,
        } = *state
            .sampler_kernel
            .take()
            .expect("retained sampler kernel");
        state.fresh_kernel = Some(kernel);
        state.fresh_native = native;
        state.fresh_active = Some(active);
        state.fresh_fragment = Some(fragment);
        state.fresh_evidence = Some(alloc::boxed::Box::write(
            evidence_backing,
            crate::native_mask_evidence::Consumer::new(),
        ));
        Ok(())
    }
    async fn register_returned_local(
        &mut self,
        state: &mut local_renewal::Renewal,
    ) -> Result<(), &'static str> {
        let spawner = self.worker_spawner.ok_or("renewal-worker-executor")?;
        if self.local_mask_worker.is_none() {
            let old = state
                .mask_handle
                .take()
                .ok_or("renewal-retired-mask-handle")?;
            match old.prepare_next(
                &mut state.fresh_session,
                &mut state.fresh_leases,
                &mut state.fresh_evidence,
                spawner,
            ) {
                Ok(handle) => self.local_mask_worker = Some(handle),
                Err((handle, reason)) => {
                    state.mask_handle = Some(handle);
                    return Err(reason);
                }
            }
            self.display.mark_local_task_registered()?;
        }
        let mask = self
            .local_mask_worker
            .as_ref()
            .expect("retained next Mask task");
        embassy_time::with_timeout(embassy_time::Duration::from_secs(2), mask.wait_ready())
            .await
            .map_err(|_| "renewal-mask-ready-timeout")?;
        if mask.status().phase != "ready-held" {
            return Err("renewal-mask-not-held");
        }
        self.prepare_returned_sampler_worker(
            &mut state.fresh_kernel,
            &mut state.fresh_native,
            &mut state.registered_publisher,
            state.fresh_active.as_ref().ok_or("renewal-fresh-active")?,
            state
                .fresh_fragment
                .as_ref()
                .ok_or("renewal-fresh-fragment")?,
            &mut state.sampler_handle,
        )
        .await
    }
    fn start_returned_local(
        &mut self,
        state: &mut local_renewal::Renewal,
    ) -> Result<(), &'static str> {
        if !state.body_started {
            let body = self.body.as_mut().ok_or("renewal-body")?;
            let current = body.realization().ok_or("renewal-body-proposal")?;
            if current.play.is_some() {
                return Err("renewal-body-already-playing");
            }
            let play = conduit_body::BodyPlayIdentity::bind(&current.plan, 1);
            let sign = |sequence| {
                c::bind_sign(&self.host, &self.boot, Some(&play.active_play_id), sequence).sign_id
            };
            let started = current
                .wake
                .body_plan_ready(&current.plan, sign(0))
                .and_then(|wake| wake.body_play_started(&current.plan, &play, sign(1)))
                .map_err(|_| "renewal-body-start-transition")?;
            body.started(&self.host, &self.boot, play, started)
                .map_err(|_| "renewal-body-start")?;
            state.body_started = true;
        }
        self.checkpoint_body()?;
        let mask = self
            .local_mask_worker
            .as_ref()
            .ok_or("renewal-mask-worker")?;
        mask.activate(
            self.body.as_ref().ok_or("renewal-body")?,
            embassy_time::Instant::now().as_millis(),
        )?;
        let sampler = self.worker.as_ref().ok_or("renewal-sampler-worker")?;
        sampler.accept_start()?;
        mask.accept_start()?;
        sampler.release()?;
        mask.release()?;
        Ok(())
    }
}
