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
        mut image_result: Option<&mut Option<crate::kernel_preparation::ImageMemoryLease>>,
        issuing_scope: Option<&issuing_host::IssuingHostScope<'_>>,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        let selected_memory = image_result.is_some();
        if issuing_scope.is_some()
            && (selected_memory
                || request.keyboard.is_some()
                || live.is_some()
                || checkpoint.is_some()
                || terminal.is_some()
                || creche.is_some())
        {
            return Err(
                "issued Host-local profile does not support IMAGE or encounter attachments".into(),
            );
        }
        if selected_memory
            && (request.keyboard.is_some()
                || request.plan.body_time_requirement.is_some()
                || correlation.is_some()
                || execution_bounds.is_some()
                || live.is_some()
                || checkpoint.is_some()
                || terminal.is_some()
                || creche.as_ref().is_some_and(|owner| !owner.image_prepared()))
        {
            return Err("selected IMAGE owned-memory profile supports qualified finite local Backs only; keyboard/activation/BodyTime attachments unsupported".into());
        }
        let _live_guard = LiveForeTerminalGuard(live.as_ref().map(|(queue, _)| *queue));
        let prospective_play = self.prospective_body_play(request.wake, request.plan)?;
        if let Some(scope) = issuing_scope {
            scope.recheck(&request, &prospective_play, &self.advertisement)?;
        }
        let whole_plans = if issuing_scope.is_none() {
            request
                .plan
                .plots
                .iter()
                .map(|part| &part.plan)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let execution_plans = issuing_scope.map_or(whole_plans.as_slice(), |scope| scope.plans());
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
        let fragments = execution_plans
            .iter()
            .flat_map(|plan| plan.fragments.iter());
        if selected_memory {
            self.kernel_resources.preflight_image_shape(fragments)?;
        } else {
            self.kernel_resources.preflight_image_workset(fragments)?;
        }
        let mut prepared_lifecycle = if selected_memory {
            Some(image::prepare_lifecycle(
                &request,
                &prospective_play,
                &self.advertisement,
            )?)
        } else {
            None
        };
        let live_queue = live.as_ref().map(|(queue, _)| *queue);
        let mut kernel = if let Some((queue, _)) = &live {
            BodyKernel::prepare_live_on_host(
                execution_plans,
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
                execution_plans,
                request.keyboard.is_some(),
                &prospective_play.active_play_id,
                fore_inputs,
                sequential_fore,
                has_fore_output,
                &self.advertisement,
                self.native_source.as_mut().map(|owner| {
                    crate::installed_std::body_kernel::NativeBodySource {
                        owner,
                        registry: &self.base_registry,
                        host: &self.advertisement,
                        wake: request.wake,
                        plan: request.plan,
                        play: &prospective_play,
                    }
                }),
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
        let plans = execution_plans
            .iter()
            .map(|plan| (*plan, false))
            .collect::<Vec<_>>();
        let (reservations, mut memory_lease) = if let Some((wake, sign)) = &prepared_lifecycle {
            let lifecycle = crate::installed_std::body_kernel::memory::lifecycle_bytes(
                &prospective_play,
                wake,
                sign,
            )?;
            let receipt = kernel.prepare_memory(
                timer,
                &self.advertisement.host_id,
                &self.advertisement.boot_id,
                lifecycle,
            )?;
            let (reservations, lease) =
                self.kernel_resources
                    .reserve_body_memory(&self.advertisement, &plans, &receipt)?;
            kernel.attach_image_memory(&lease);
            (reservations, Some(lease))
        } else {
            (
                self.kernel_resources
                    .prepare_and_reserve_plans(&self.advertisement, &plans)?,
                None,
            )
        };
        drop(plans);
        drop(whole_plans);
        let result = (|| {
            let sequence = self.next_kernel_play_sequence;
            let next_play = sequence
                .checked_add(1)
                .ok_or_else(|| "Body Play sequence exhausted".to_string())?;
            let play = prospective_play;
            if play.play_sequence != sequence {
                return Err("Body prospective Play identity changed before start".into());
            }
            let local_signs = if issuing_scope.is_some() {
                // Global lifecycle owns ordinals 0/1/2 for this parent Play.
                // Reserve distinct local evidence from the ordinary Host cursor;
                // never recycle it after a callback or effect uncertainty.
                let first = self
                    .next_kernel_sign_sequence
                    .max(issuing_host::FIRST_LOCAL_SIGN_SEQUENCE);
                let next = first.checked_add(2).ok_or("Host Sign sequence exhausted")?;
                Some((first, next))
            } else {
                None
            };
            self.next_kernel_play_sequence = next_play;
            if let Some((_, next)) = local_signs {
                self.next_kernel_sign_sequence = next;
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
            let (wake_at_start, prepared_terminal_sign) = if let Some(prepared) =
                prepared_lifecycle.take()
            {
                prepared
            } else if let Some((first, _)) = local_signs {
                // Preserve the original Wake unchanged: local kernel evidence
                // never manufactures global Ready/Playing or settlement.
                (request.wake.clone(), sign(first + 1))
            } else {
                let wake_at_start = request
                    .wake
                    .body_plan_ready(request.plan, sign(0).sign_id)
                    .and_then(|wake| wake.body_play_started(request.plan, &play, sign(1).sign_id))
                    .map_err(|error| format!("Body start lifecycle: {error:?}"))?;
                (wake_at_start, sign(2))
            };
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
            let terminal_sign = prepared_terminal_sign;
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
        let result = finish_body_release(result, release_errors);
        if let (Some(slot), Some(mut lease), Ok(report)) =
            (image_result.as_mut(), memory_lease.take(), result.as_ref())
        {
            let bytes = crate::installed_std::body_kernel::memory::result_bytes(report)?;
            lease.retain_result(bytes);
            **slot = Some(lease);
        }
        result
    }
}
