//! Shared ordinary Body runner with an optional exact supplied encounter owner.
use super::*;

impl StdHost {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_body_plan_to_with_start_and_clock<'a, W: Write, T: TimerAdapter, F>(
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
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
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
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_body_plan_to_with_start_and_clock_and_creche<
        'a,
        W: Write,
        T: TimerAdapter,
        F,
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
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        let _live_guard = LiveForeTerminalGuard(live.as_ref().map(|(queue, _)| *queue));
        request
            .plan
            .validate_for(request.wake)
            .map_err(|error| format!("Body Plan validation: {error:?}"))?;
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
        self.kernel_resources.preflight_image_workset(
            request
                .plan
                .plots
                .iter()
                .flat_map(|part| part.plan.fragments.iter()),
        )?;
        let prospective_play = BodyPlayIdentity::bind(request.plan, self.next_kernel_play_sequence);
        let live_queue = live.as_ref().map(|(queue, _)| *queue);
        let mut kernel = if let Some((queue, _)) = &live {
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
        let reservations = self.kernel_resources.prepare_and_reserve_plans(
            &self.advertisement,
            &request
                .plan
                .plots
                .iter()
                .map(|part| (&part.plan, false))
                .collect::<Vec<_>>(),
        )?;
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
            let wake_at_start = request
                .wake
                .body_plan_ready(request.plan, sign(0).sign_id)
                .and_then(|wake| wake.body_play_started(request.plan, &play, sign(1).sign_id))
                .map_err(|error| format!("Body start lifecycle: {error:?}"))?;
            if let Err(error) = started(&play, &wake_at_start) {
                if let Some(queue) = live_queue {
                    queue.mark_play_terminal();
                }
                return Err(error);
            }
            if let Some(owner) = creche.as_deref_mut() {
                owner.confirm_started(&self.advertisement, &play)?;
            }
            if let Some(queue) = live_queue {
                queue.mark_play_started();
            }
            let terminal_sign = sign(2);
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
            );
            if let Some(queue) = live_queue {
                queue.mark_play_terminal();
            }
            Ok(BodyRunReport {
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
        finish_body_release(result, release_errors)
    }
}
