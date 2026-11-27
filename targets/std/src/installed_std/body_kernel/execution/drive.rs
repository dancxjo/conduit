//! Shared single iteration of the ordinary Body loop.
use super::state::BodyExecutionState;
use super::*;
impl BodyKernel<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn drive<W: Write, T: TimerAdapter>(
        &mut self,
        state: &mut BodyExecutionState<'_, '_>,
        output: &mut W,
        clock: &mut T,
        fore_output: &mut Option<&mut dyn BodyForeOutputAdapter>,
        control: &RunControl,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
        body_time: Option<(&BodyTimeRequirement, &BodyClockCorrelation)>,
        execution_bounds: Option<(
            conduit_core::MonotonicDuration,
            conduit_core::MonotonicDuration,
        )>,
        terminal: &mut Option<crate::body_execution::ParsecTerminalAttachment<'_>>,
        creche: &mut Option<&mut crate::hosted_creche::CrecheHost<'_>>,
        wait: bool,
        cancel_requested: bool,
        external_cancel: &dyn Fn() -> bool,
    ) -> Result<Option<TerminalDisposition>, String> {
        let cancel_requested = cancel_requested || external_cancel();
        if !state.cancelling && !(control.stop_requested() || cancel_requested) {
            self.poll_creche(creche.as_deref_mut())?;
            if let Some(observer) = state.face_observer.as_deref_mut() {
                for route in &mut self.face_contribution_routes {
                    if !route.is_retired() || route.is_pending() {
                        route.poll(&mut self.scheduler, observer)?;
                    }
                }
            }
        }
        self.fore.drain(
            &mut self.scheduler,
            fore_output,
            self.supported_parsec_input_scan.then_some(&mut state.keys),
            self.parsec_map.as_mut(),
        )?;
        if !state.cancelling && !(control.stop_requested() || cancel_requested) {
            if let (Some(owner), Some(map), Some(attachment)) = (
                self.parsec_terminal.as_mut(),
                self.parsec_map.as_ref(),
                terminal.as_mut(),
            ) {
                if owner.ensure_present(map, &attachment.host, attachment.output)? {
                    let (face, show) = owner.current_face_show(map, &attachment.host)?;
                    attachment.observer.showed(face, show)?;
                }
            }
        }
        // Drain every source state before checking an input. An event
        // observed against an earlier Show must never be reinterpreted
        // against a newer source map merely because Fore order differs.
        while let Some(observed) = self.fore.take_acknowledged_input() {
            if state.cancelling || control.stop_requested() {
                continue;
            }
            if let (Some(owner), Some(map), Some(attachment)) = (
                self.parsec_terminal.as_mut(),
                self.parsec_map.as_ref(),
                terminal.as_mut(),
            ) {
                let input_plan = state.keys.require_completed_event_scope(
                    &observed.event,
                    owner.body_plan(),
                    map.current_play()?,
                    &attachment.host,
                )?;
                if input_plan != &owner.body_plan().plots[0].plan.plan_id {
                    return Err("Parsec completed event belongs to a different input Plan".into());
                }
                if !owner.authorize_prompted_event(
                    map,
                    &attachment.host,
                    &state.keys,
                    &observed.event,
                )? {
                    owner.sync_prompt(map, &attachment.host, &state.keys, attachment.output)?;
                    continue;
                }
                owner.admit_completed_input(
                    map,
                    &attachment.host,
                    &observed.event,
                    self.fore.acknowledged_input_bytes(&observed),
                    attachment.queue,
                )?;
                owner.sync_prompt(map, &attachment.host, &state.keys, attachment.output)?;
            }
        }
        self.clock_observations.capture_new(
            self.scheduler.signs().events(),
            clock,
            host_id,
            boot_id,
        );
        if !state.cancelling && (control.requested_stop().is_some() || cancel_requested) {
            if let Some(owner) = creche.as_deref_mut() {
                owner.retire()?;
            }
            self.scheduler.cancel().map_err(|error| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body cancel: {error:?}"),
                )
            })?;
            state.keys.cancel();
            // No acknowledged map remains current while the owning
            // Body Play is being cancelled.
            if let (Some(owner), Some(attachment)) =
                (self.parsec_terminal.as_mut(), terminal.as_mut())
            {
                owner.withdraw_prompt(attachment.output)?;
            }
            state.keys.set_input_view_epoch(None);
            self.parsec_map = None;
            self.parsec_terminal = None;
            state.deadlines.clear();
            state.cancelling = true;
        }
        let live_observed = self.fore.activity_generation();
        if !state.cancelling {
            self.fore.start_and_feed(&mut self.scheduler)?;
            if let Some((requirement, correlation)) = body_time {
                let sample = match clock.monotonic_observation(host_id, boot_id) {
                    Some(sample) => sample,
                    None => {
                        state.clock_quality = Some(BodyTimeQuality::Unsupported {
                            reason: BodyTimeRefusal::Unavailable,
                        });
                        return Err("BodyTime-qualified Play lost its local monotonic clock".into());
                    }
                };
                let quality = crate::body_execution::assess_continuing_body_clock(
                    requirement,
                    correlation,
                    &sample,
                    execution_bounds,
                    state.clock_quality.as_ref(),
                );
                state.clock_quality = Some(quality.clone());
                match quality {
                    BodyTimeQuality::Ready { .. } => {}
                    quality => {
                        return Err(format!(
                            "BodyTime-qualified Play lost clock quality: {quality:?}"
                        ));
                    }
                }
            }
        }
        while let Some(cancellation) = self.scheduler.next_host_cancellation() {
            if self.cancel_native_source(cancellation)? {
                continue;
            }
            let operation = self
                .operations
                .iter()
                .find(|op| op.node == cancellation.node && op.call == cancellation.call)
                .ok_or("Body cancellation has no exact operation")?;
            if operation.contract_id.as_str()
                == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
            {
                self.cancel_creche_call(cancellation, creche.as_deref_mut())?;
            } else if super::super::face_contributions::is_face_contribution_call(operation) {
                self.scheduler
                    .complete_host_call(
                        cancellation.node,
                        cancellation.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Cancelled,
                            output: None,
                            failure: None,
                        },
                    )
                    .map_err(|error| format!("Face contribution cancellation: {error:?}"))?;
            } else if keyboard(&operation.contract_id) || button(&operation.contract_id) {
                state.keys.cancel();
                self.scheduler
                    .complete_host_call(
                        cancellation.node,
                        cancellation.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Cancelled,
                            output: None,
                            failure: None,
                        },
                    )
                    .map_err(|error| format!("Body keyboard cancellation: {error:?}"))?;
            } else {
                state
                    .deadlines
                    .cancel(cancellation, &mut self.scheduler)
                    .map_err(|error| {
                        diagnostic::error(&mut self.error_diagnostic, format_args!("{error}"))
                    })?;
            }
        }
        self.poll_native_source()?;
        while let Some(request) = self.scheduler.next_host_request() {
            self.clock_observations.capture_new(
                self.scheduler.signs().events(),
                clock,
                host_id,
                boot_id,
            );
            self.perform_request(
                request,
                &mut state.keys,
                &mut state.deadlines,
                clock,
                output,
                fore_output,
                creche.as_deref_mut(),
                control,
                external_cancel,
            )?;
            if control.stop_requested() || external_cancel() {
                return Ok(None);
            }
        }
        let status = self.scheduler.step().map_err(|error| {
            if let conduit_kernel::scheduler::SchedulerError::BackFailed(failure) = error {
                state.kernel_failure = Some(failure);
            }
            diagnostic::error(
                &mut self.error_diagnostic,
                format_args!("Body kernel: {error:?}"),
            )
        })?;
        self.clock_observations.capture_new(
            self.scheduler.signs().events(),
            clock,
            host_id,
            boot_id,
        );
        // Snapshot the privately owned, acknowledged Show immediately
        // before any adapter poll. This is observation metadata only;
        // later action admission must still recheck exact current custody.
        let input_view_epoch = if !state.cancelling
            && !(control.stop_requested() || cancel_requested)
        {
            match (
                self.parsec_terminal.as_mut(),
                self.parsec_map.as_ref(),
                terminal.as_mut(),
            ) {
                (Some(owner), Some(map), Some(attachment)) => {
                    if owner.sync_prompt(map, &attachment.host, &state.keys, attachment.output)? {
                        if let Some(observation) =
                            owner.instruction_observation(map, &attachment.host, &state.keys)?
                        {
                            attachment.observer.instruction_observed(&observation)?;
                        }
                    }
                    owner.prompt_input_view(map, &attachment.host, &state.keys)?
                }
                _ => None,
            }
        } else {
            if let (Some(owner), Some(attachment)) =
                (self.parsec_terminal.as_mut(), terminal.as_mut())
            {
                owner.withdraw_prompt(attachment.output)?;
            }
            None
        };
        state.keys.set_input_view_epoch(input_view_epoch);
        match status {
            SchedulerStatus::Drained => {
                self.fore.require_normal_terminal(&self.scheduler)?;
                return Ok(Some(TerminalDisposition::Completed));
            }
            SchedulerStatus::Cancelled => {
                return Ok(Some(TerminalDisposition::Cancelled {
                    reason: CancellationReason::OperatorRequested,
                }));
            }
            SchedulerStatus::Progress { .. } => {
                if state.keys.is_pending() {
                    state.keys.poll(&mut self.scheduler)?;
                }
            }
            SchedulerStatus::Idle => {
                let pending_native = self.native_source_pending()
                    || self
                        .face_contribution_routes
                        .iter()
                        .any(|route| route.is_pending());
                if state.keys.poll(&mut self.scheduler)?
                    || (if wait && !pending_native {
                        state.deadlines.complete_next(&mut self.scheduler, clock)
                    } else {
                        state.deadlines.complete_ready(&mut self.scheduler, clock)
                    })
                    .map_err(|error| {
                        diagnostic::error(&mut self.error_diagnostic, format_args!("{error}"))
                    })?
                {
                    return Ok(None);
                }
                // A pending admitted keyboard request owns progress even
                // when no command has reached the live Fore queue yet.
                if pending_native {
                    if wait {
                        std::thread::park_timeout(std::time::Duration::from_millis(2));
                    }
                    return Ok(None);
                }
                if !state.keys.is_pending() && !self.creche_pending() {
                    if let Some(observed) = live_observed {
                        if wait && self.fore.wait_for_activity(observed)? {
                            return Ok(None);
                        }
                    }
                }
                if !state.keys.is_pending() && state.deadlines.is_empty() && !self.creche_pending()
                {
                    return Err(diagnostic::error(
                        &mut self.error_diagnostic,
                        format_args!("Body kernel has no admitted progress source"),
                    ));
                }
                if wait && self.creche_pending() {
                    if let Some(owner) = creche.as_deref() {
                        std::thread::park_timeout(
                            owner.poll_wait().min(std::time::Duration::from_millis(10)),
                        );
                    }
                }
                if wait {
                    std::thread::yield_now();
                }
            }
        }
        Ok(None)
    }
}
