//! Shared ordinary Body runner with an optional exact supplied encounter owner.
use super::*;

impl StdHost {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_body_plan_to_with_start_and_clock<
        'a,
        W: Write,
        T: TimerAdapter,
        F,
        A: StartupAcknowledgmentValue,
    >(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        correlation: Option<&BodyClockCorrelation>,
        execution_bounds: Option<(MonotonicDuration, MonotonicDuration)>,
        fore: Option<(
            &[ExternalForeInput],
            bool,
            &'a mut dyn BodyForeOutputAdapter,
        )>,
        live: Option<(&BodyLiveForeQueue, &'a mut dyn BodyForeOutputAdapter)>,
        checkpoint: Option<(&Path, crate::todo_durable_resource::CheckpointIdentity)>,
        terminal: Option<parsec_terminal::ParsecTerminalAttachment<'a>>,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<A, String>,
    {
        self.run_body_plan_to_with_start_and_clock_and_creche(
            request,
            output,
            timer,
            correlation,
            execution_bounds,
            fore,
            live,
            checkpoint,
            terminal,
            None,
            started,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_body_plan_to_with_start_and_clock_and_creche<
        'a,
        W: Write,
        T: TimerAdapter,
        F,
        A: StartupAcknowledgmentValue,
    >(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        correlation: Option<&BodyClockCorrelation>,
        execution_bounds: Option<(MonotonicDuration, MonotonicDuration)>,
        fore: Option<(
            &[ExternalForeInput],
            bool,
            &'a mut dyn BodyForeOutputAdapter,
        )>,
        live: Option<(&BodyLiveForeQueue, &'a mut dyn BodyForeOutputAdapter)>,
        checkpoint: Option<(&Path, crate::todo_durable_resource::CheckpointIdentity)>,
        terminal: Option<parsec_terminal::ParsecTerminalAttachment<'a>>,
        mut creche: Option<&mut crate::hosted_creche::CrecheHost<'_>>,
        mut started: F,
        custody: Option<&mut Option<crate::kernel_preparation::ImageMemoryLease>>,
        face_observer: Option<&mut dyn BodyFaceContributionObserver>,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<A, String>,
    {
        let _live_guard = LiveForeTerminalGuard(live.as_ref().map(|(queue, _)| *queue));
        let funded = if custody.is_some() {
            if correlation.is_some()
                || execution_bounds.is_some()
                || live.is_some()
                || checkpoint.is_some()
                || terminal.is_some()
                || creche.is_some()
                || fore
                    .as_ref()
                    .is_some_and(|(inputs, sequential, _)| !inputs.is_empty() || *sequential)
            {
                return Err("funded IMAGE supports direct Tick with bounded output only".into());
            }
            let layout = crate::installed_std::body_kernel::funded::TickPreparationLayout::prepare_with_playback(
                &request,
                &self.advertisement,
                &self.kernel_resources,
                timer,
                fore.is_some(),
                self.playback.as_ref(),
                self.native_source.as_ref(),
            )?;
            let lease = self
                .kernel_resources
                .fund_tick_preparation(layout.peak_bytes)?;
            Some((layout, lease))
        } else {
            None
        };
        let prospective_play = if funded.is_some() {
            request
                .plan
                .validate_for_borrowed(request.wake)
                .map_err(|error| format!("Body Plan validation: {error:?}"))?;
            self.next_kernel_play_sequence
                .checked_add(1)
                .ok_or("Body Play sequence exhausted")?;
            BodyPlayIdentity::bind(request.plan, self.next_kernel_play_sequence)
        } else {
            self.prospective_body_play(request.wake, request.plan)?
        };
        if funded.is_some() {
            self.kernel_resources.preflight_image_shape(
                request
                    .plan
                    .plots
                    .iter()
                    .flat_map(|part| &part.plan.fragments),
            )?;
        } else {
            self.kernel_resources.preflight_image_workset(
                request
                    .plan
                    .plots
                    .iter()
                    .flat_map(|part| &part.plan.fragments),
            )?;
        }
        let admitted_clock_quality = if let Some(requirement) = &request.plan.body_time_requirement
        {
            let correlation = correlation.ok_or_else(|| {
                "BodyTime-qualified Plan requires an explicit clock correlation".to_string()
            })?;
            let sample = timer
                .monotonic_observation(&self.advertisement.host_id, &self.advertisement.boot_id)
                .ok_or_else(|| "BodyTime-qualified Plan has no admitted local clock".to_string())?;
            if sample.clock().host_id() != &self.advertisement.host_id
                || sample.clock().boot_id() != &self.advertisement.boot_id
            {
                return Err("BodyTime-qualified Plan has a different Host/Boot clock".into());
            }
            match assess_body_clock(requirement, correlation, &sample, execution_bounds) {
                quality @ BodyTimeQuality::Ready { .. } => Some(quality),
                quality => return Err(format!("BodyTime-qualified Plan quality: {quality:?}")),
            }
        } else {
            None
        };
        let (fore_inputs, sequential_fore, has_fore_output) = fore
            .as_ref()
            .map_or((&[][..], false, false), |(inputs, sequential, _)| {
                (*inputs, *sequential, true)
            });
        // Bind the prospective identity without consuming the sequence. The
        // exact child pool must be prepared and correlated before Play start;
        // a preparation refusal leaves the Host sequence untouched.
        let live_queue = live.as_ref().map(|(queue, _)| *queue);
        let mut kernel = if let Some((layout, _)) = &funded {
            BodyKernel::prepare_funded(
                &request.plan.plots,
                &prospective_play.active_play_id,
                has_fore_output,
                layout,
                &self.advertisement,
                self.playback.as_ref(),
                self.native_source.as_mut().map(|owner| {
                    crate::installed_std::body_kernel::native_source::NativeBodySource {
                        owner,
                        registry: &self.base_registry,
                        host: &self.advertisement,
                        wake: request.wake,
                        plan: request.plan,
                        play: &prospective_play,
                    }
                }),
            )?
        } else if let Some((queue, _)) = &live {
            BodyKernel::prepare_live_on_host(
                &request.plan.plots,
                request.keyboard.is_some(),
                &prospective_play.active_play_id,
                queue,
                request.control,
                &self.advertisement,
                #[cfg(target_os = "linux")]
                self.selected_text_file.as_ref(),
            )?
        } else {
            BodyKernel::prepare_on_host(
                &request.plan.plots,
                request.keyboard.is_some(),
                &prospective_play.active_play_id,
                fore_inputs,
                sequential_fore,
                has_fore_output,
                &self.advertisement,
                #[cfg(target_os = "linux")]
                self.selected_text_file.as_ref(),
            )?
        };
        if funded.is_some() {
            kernel.prepare_home_face_routes(
                request.wake,
                request.plan,
                &prospective_play,
                &self.advertisement,
                face_observer.is_some(),
            )?;
        } else {
            kernel.prepare_startup_outputs(&request.plan.plots, self.playback.as_ref())?;
        }
        kernel.prepare_input_origin(&self.advertisement, request.plan, &prospective_play)?;
        if let Some(terminal) = &terminal {
            kernel.attach_parsec_terminal(
                request.plan,
                &self.advertisement,
                terminal.columns,
                terminal.rows,
            )?;
        }
        if let Some((root, checkpoint)) = checkpoint {
            kernel.attach_todo_checkpoint(&request.plan.plots, root, checkpoint)?;
        }
        if let Some(owner) = creche.as_deref_mut() {
            kernel.attach_creche(owner, request.plan, &prospective_play, &self.advertisement)?;
        }
        kernel.require_supported_execution()?;
        if kernel.requires_startup() && !A::PROVIDES_STARTUP {
            return Err("startup Source requires retained Body Started acknowledgment".into());
        }
        let prepared_lifecycle = if funded.is_some() {
            Some(image::prepare_lifecycle(
                &request,
                &prospective_play,
                &self.advertisement,
            )?)
        } else {
            None
        };
        let first = &request.plan.plots[0].plan;
        let mut plans = [(first, false); conduit_body::MAX_BODY_PLOTS];
        for (target, part) in plans.iter_mut().zip(&request.plan.plots) {
            *target = (&part.plan, false);
        }
        let plans = &plans[..request.plan.plots.len()];
        let reservations = if let Some((layout, lease)) = &funded {
            let (wake, sign) = prepared_lifecycle.as_ref().unwrap();
            let lifecycle = crate::installed_std::body_kernel::memory::lifecycle_bytes(
                &prospective_play,
                wake,
                sign,
            )?;
            super::prepared::reserve_prepared_kernel(
                &mut kernel,
                timer,
                &self.advertisement,
                &mut self.kernel_resources,
                plans,
                lifecycle,
                layout.clock_identity_bytes,
                lease,
            )?
        } else {
            self.kernel_resources
                .prepare_and_reserve_plans(&self.advertisement, plans)?
        };
        let result = (|| {
            let sequence = self.next_kernel_play_sequence;
            self.next_kernel_play_sequence = sequence
                .checked_add(1)
                .ok_or_else(|| "Body Play sequence exhausted".to_string())?;
            let play = prospective_play;
            if play.play_sequence != sequence {
                return Err("Body prospective Play identity changed before start".into());
            }
            kernel.confirm_input_play(&self.advertisement, &play)?;
            // Body lifecycle signs are scoped by this unique admitted Play.
            // The shared lifecycle session and browser producer use 0/1/2;
            // reusing the Host-wide cursor would make a second genuine start
            // impossible for BodyLifecycleSession::started to accept.
            let sign = |sequence| {
                bind_sign(
                    &self.advertisement.host_id,
                    &self.advertisement.boot_id,
                    Some(&play.active_play_id),
                    sequence,
                )
            };
            let (wake_at_start, prepared_terminal_sign) =
                if let Some((wake, sign)) = prepared_lifecycle {
                    (wake, Some(sign))
                } else {
                    (
                        request
                            .wake
                            .body_plan_ready(request.plan, sign(0).sign_id)
                            .and_then(|wake| {
                                wake.body_play_started(request.plan, &play, sign(1).sign_id)
                            })
                            .map_err(|error| format!("Body start lifecycle: {error:?}"))?,
                        None,
                    )
                };
            let acknowledgment: BodyStartedAcknowledgment = match started(&play, &wake_at_start) {
                Ok(value) => value.into(),
                Err(error) => {
                    if let Some(queue) = live_queue {
                        queue.mark_play_terminal();
                    }
                    return Err(error);
                }
            };
            let startup = match &acknowledgment {
                BodyStartedAcknowledgment::LifecycleOnly => None,
                BodyStartedAcknowledgment::CommittedStartup(witness) => {
                    Some(witness.for_play(request.plan, &play)?)
                }
            };
            kernel.bind_startup(startup)?;
            if let Some(owner) = creche.as_deref_mut() {
                owner.confirm_started(&self.advertisement, &play)?;
            }
            if let Some(queue) = live_queue {
                queue.mark_play_started();
            }
            let terminal_sign = prepared_terminal_sign.unwrap_or_else(|| sign(2));
            let fore_output = if let Some((_, adapter)) = live {
                Some(adapter)
            } else {
                fore.map(|(_, _, adapter)| adapter)
            };
            let result = kernel.run(
                output,
                timer,
                request.keyboard,
                fore_output,
                request.control,
                &self.advertisement.host_id,
                &self.advertisement.boot_id,
                request.plan.body_time_requirement.as_ref().zip(correlation),
                execution_bounds,
                admitted_clock_quality,
                terminal,
                creche.as_deref_mut(),
                face_observer,
            );
            if let Some(queue) = live_queue {
                queue.mark_play_terminal();
            }
            Ok(BodyRunReport {
                startup_chimes: result.startup_chimes,
                play,
                wake_at_start,
                terminal: result.terminal,
                failure: result.failure,
                kernel_failure: result.kernel_failure,
                cleanup_failure: result.cleanup_failure,
                terminal_sign,
                partitions: result.partitions,
                requests: result.requests,
                kernel_events: result.events,
                scan_child_signs: result.scan_child_signs,
                scan_cancellation_failed: result.scan_cancellation_failed,
                scan_output_completion_failed: result.scan_output_completion_failed,
                each_child_signs: result.each_child_signs,
                each_cancellation_failed: result.each_cancellation_failed,
                each_output_completion_failed: result.each_output_completion_failed,
                fore_deliveries: result.fore_deliveries,
                fore_pending_output: result.fore_pending_output,
                live_fore_status: live_queue.map(BodyLiveForeQueue::status),
                clock_observations: result.clock_observations,
                clock_quality: result.clock_quality,
                clock_execution_bounds: result.clock_execution_bounds,
            })
        })();
        // A failed platform drain must not turn occupied capacity into Ready.
        // Move exact charged tokens into terminal Host custody; retain the
        // execution's original failure separately from cleanup uncertainty.
        if let Some(owner) = creche {
            if let Err(error) = owner.retire() {
                self.kernel_resources.quarantine_retirement(reservations);
                return finish_body_release(
                    result,
                    vec![format!("Creche retirement quarantined: {error}")],
                );
            }
        }
        let mut release_errors = Vec::new();
        for reservation in reservations {
            if let Err(error) = self.kernel_resources.release(reservation) {
                release_errors.push(error);
            }
        }
        let report = finish_body_release(result, release_errors)?;
        if let Some((_, mut lease)) = funded {
            let retained = crate::installed_std::body_kernel::memory::result_bytes(&report)?;
            lease.retain_result(retained);
            *custody.ok_or("funded result custody missing")? = Some(lease);
        }
        Ok(report)
    }
}
