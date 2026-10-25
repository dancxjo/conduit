//! Body Play lifecycle, polling and terminal evidence custody.
use super::super as installed;
use super::*;

impl<'a> BodyKernel<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run<W: Write, T: TimerAdapter>(
        mut self,
        output: &mut W,
        clock: &mut T,
        input: Option<&mut dyn HostedKeyboardAdapter>,
        mut fore_output: Option<&mut dyn BodyForeOutputAdapter>,
        control: &RunControl,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
        body_time: Option<(&BodyTimeRequirement, &BodyClockCorrelation)>,
        execution_bounds: Option<(
            conduit_core::MonotonicDuration,
            conduit_core::MonotonicDuration,
        )>,
        admitted_clock_quality: Option<BodyTimeQuality>,
        mut terminal: Option<crate::body_execution::ParsecTerminalAttachment<'_>>,
        mut creche: Option<&mut crate::hosted_creche::CrecheHost<'_>>,
    ) -> BodyKernelResult {
        let mut keys = installed::keyboard_input_host::KeyboardInputHost::new(
            input,
            self.operations
                .iter()
                .map(|operation| &operation.contract_id),
        );
        if let Some(admission) = self.input_origin.take() {
            // Installation can only fail for a reused or already-active host;
            // this host is freshly constructed before its first request.
            keys.install_request_admission(admission)
                .expect("fresh Body keyboard input host");
        }
        let mut deadlines =
            installed::deadline_host::InstalledDeadlineHost::<PENDING_REQUESTS>::new();
        let mut clock_quality = admitted_clock_quality;
        let mut kernel_failure = None;
        let result = (|| -> Result<TerminalDisposition, String> {
            let mut cancelling = false;
            loop {
                self.fore.drain(
                    &mut self.scheduler,
                    &mut fore_output,
                    self.supported_parsec_input_scan.then_some(&mut keys),
                    self.parsec_map.as_mut(),
                )?;
                if !cancelling && !control.stop_requested() {
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
                    if cancelling || control.stop_requested() {
                        continue;
                    }
                    if let (Some(owner), Some(map), Some(attachment)) = (
                        self.parsec_terminal.as_mut(),
                        self.parsec_map.as_ref(),
                        terminal.as_mut(),
                    ) {
                        let input_plan = keys.require_completed_event_scope(
                            &observed.event,
                            owner.body_plan(),
                            map.current_play()?,
                            &attachment.host,
                        )?;
                        if input_plan != &owner.body_plan().plots[0].plan.plan_id {
                            return Err(
                                "Parsec completed event belongs to a different input Plan".into()
                            );
                        }
                        if !owner.authorize_prompted_event(
                            map,
                            &attachment.host,
                            &keys,
                            &observed.event,
                        )? {
                            owner.sync_prompt(map, &attachment.host, &keys, attachment.output)?;
                            continue;
                        }
                        owner.admit_completed_input(
                            map,
                            &attachment.host,
                            &observed.event,
                            self.fore.acknowledged_input_bytes(&observed),
                            attachment.queue,
                        )?;
                        owner.sync_prompt(map, &attachment.host, &keys, attachment.output)?;
                    }
                }
                self.clock_observations.capture_new(
                    self.scheduler.signs().events(),
                    clock,
                    host_id,
                    boot_id,
                );
                if !cancelling && control.requested_stop().is_some() {
                    if let Some(owner) = creche.as_deref_mut() {
                        owner.retire()?;
                    }
                    self.scheduler
                        .cancel()
                        .map_err(|error| format!("Body cancel: {error:?}"))?;
                    keys.cancel();
                    // No acknowledged map remains current while the owning
                    // Body Play is being cancelled.
                    if let (Some(owner), Some(attachment)) =
                        (self.parsec_terminal.as_mut(), terminal.as_mut())
                    {
                        owner.withdraw_prompt(attachment.output)?;
                    }
                    keys.set_input_view_epoch(None);
                    self.parsec_map = None;
                    self.parsec_terminal = None;
                    deadlines.clear();
                    cancelling = true;
                }
                let live_observed = self.fore.activity_generation();
                if !cancelling {
                    self.fore.start_and_feed(&mut self.scheduler)?;
                    if let Some((requirement, correlation)) = body_time {
                        let sample = match clock.monotonic_observation(host_id, boot_id) {
                            Some(sample) => sample,
                            None => {
                                clock_quality = Some(BodyTimeQuality::Unsupported {
                                    reason: BodyTimeRefusal::Unavailable,
                                });
                                return Err(
                                    "BodyTime-qualified Play lost its local monotonic clock".into(),
                                );
                            }
                        };
                        let quality = crate::body_execution::assess_continuing_body_clock(
                            requirement,
                            correlation,
                            &sample,
                            execution_bounds,
                            clock_quality.as_ref(),
                        );
                        clock_quality = Some(quality.clone());
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
                    let operation = self
                        .operations
                        .iter()
                        .find(|op| op.node == cancellation.node && op.call == cancellation.call)
                        .ok_or("Body cancellation has no exact operation")?;
                    if keyboard(&operation.contract_id) || button(&operation.contract_id) {
                        keys.cancel();
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
                    } else if operation.contract_id.as_str()
                        == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
                    {
                        self.cancel_creche_call(cancellation, creche.as_deref_mut())?;
                    } else {
                        deadlines.cancel(cancellation, &mut self.scheduler)?;
                    }
                }
                self.dispatch_host_calls(
                    &mut keys,
                    &mut deadlines,
                    clock,
                    output,
                    host_id,
                    boot_id,
                    creche.as_deref_mut(),
                )?;
                if !cancelling {
                    self.poll_creche(creche.as_deref_mut())?;
                }
                let status = self.scheduler.step().map_err(|error| {
                    if let conduit_kernel::scheduler::SchedulerError::BackFailed(failure) = error {
                        kernel_failure = Some(failure);
                    }
                    format!("Body kernel: {error:?}")
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
                let input_view_epoch = if !cancelling && !control.stop_requested() {
                    match (
                        self.parsec_terminal.as_mut(),
                        self.parsec_map.as_ref(),
                        terminal.as_mut(),
                    ) {
                        (Some(owner), Some(map), Some(attachment)) => {
                            if owner.sync_prompt(map, &attachment.host, &keys, attachment.output)? {
                                if let Some(observation) =
                                    owner.instruction_observation(map, &attachment.host, &keys)?
                                {
                                    attachment.observer.instruction_observed(&observation)?;
                                }
                            }
                            owner.prompt_input_view_epoch(map, &attachment.host, &keys)?
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
                keys.set_input_view_epoch(input_view_epoch);
                match status {
                    SchedulerStatus::Drained => {
                        self.fore.require_normal_terminal(&self.scheduler)?;
                        return Ok(TerminalDisposition::Completed);
                    }
                    SchedulerStatus::Cancelled => {
                        return Ok(TerminalDisposition::Cancelled {
                            reason: CancellationReason::OperatorRequested,
                        });
                    }
                    SchedulerStatus::Progress { .. } => {
                        if keys.is_pending() {
                            keys.poll(&mut self.scheduler)?;
                        }
                    }
                    SchedulerStatus::Idle => {
                        if keys.poll(&mut self.scheduler)?
                            || deadlines.complete_next(&mut self.scheduler, clock)?
                        {
                            continue;
                        }
                        // A pending admitted keyboard request owns progress even
                        // when no command has reached the live Fore queue yet.
                        if !keys.is_pending() && !self.creche_pending() {
                            if let Some(observed) = live_observed {
                                if self.fore.wait_for_activity(observed)? {
                                    continue;
                                }
                            }
                        }
                        if !keys.is_pending() && deadlines.is_empty() && !self.creche_pending() {
                            return Err("Body kernel has no admitted progress source".into());
                        }
                        if self.creche_pending() {
                            if let Some(owner) = creche.as_deref() {
                                std::thread::park_timeout(
                                    owner.poll_wait().min(std::time::Duration::from_millis(10)),
                                );
                            }
                        }
                        std::thread::yield_now();
                    }
                }
            }
        })();
        self.clock_observations.capture_new(
            self.scheduler.signs().events(),
            clock,
            host_id,
            boot_id,
        );
        // Revoke the prompt on every terminal outcome, including failures
        // before the next adapter poll. A withdrawal failure is itself a failure.
        let withdrawal = match (self.parsec_terminal.as_mut(), terminal.as_mut()) {
            (Some(owner), Some(attachment)) => owner.withdraw_prompt(attachment.output).map(|_| ()),
            _ => Ok(()),
        };
        keys.set_input_view_epoch(None);
        let result = result.and_then(|outcome| withdrawal.map(|()| outcome));
        // Platform work must be revoked/drained before pending kernel ownership
        // is discarded. Preserve cleanup failure separately from the cause.
        let retirement = match creche {
            Some(owner) => owner.retire(),
            None => Ok(()),
        };
        let mut cleanup_failure = retirement
            .as_ref()
            .err()
            .map(|e| format!("Creche owner cleanup: {e}"));
        let result = result.and_then(|outcome| {
            retirement
                .as_ref()
                .map(|()| outcome)
                .map_err(|e| format!("Creche owner retirement: {e}"))
        });
        let (terminal, failure) = match result {
            Ok(terminal) => (terminal, None),
            Err(error) => {
                keys.cancel();
                deadlines.clear();
                if retirement.is_ok() {
                    cleanup_failure = self
                        .scheduler
                        .cancel()
                        .err()
                        .map(|error| format!("Body failure cleanup: {error:?}"));
                }
                (
                    TerminalDisposition::Failed {
                        reason: FailureReason::RequiredBranchFailed,
                    },
                    Some(error),
                )
            }
        };
        self.clock_observations.capture_new(
            self.scheduler.signs().events(),
            clock,
            host_id,
            boot_id,
        );
        // Face snapshots run after the sealed Play. Preserve child
        // identity and any receipt refusal instead of mixing child events
        // into the parent's unqualified kernel Sign stream.
        let scan_child_signs = self
            .scheduler
            .drivers()
            .iter()
            .filter_map(|driver| match driver {
                InstalledBack::BodyScan(scan) => Some(scan.as_ref()),
                _ => None,
            })
            .try_fold(Vec::new(), |mut receipts, scan| {
                receipts.extend(scan.child_sign_receipts()?);
                Ok(receipts)
            });
        let scan_cancellation_failed = self.scheduler.drivers().iter().any(
            |driver| matches!(driver, InstalledBack::BodyScan(scan) if scan.cancellation_failed()),
        );
        let scan_output_completion_failed = self.scheduler.drivers().iter().any(|driver| {
            matches!(driver, InstalledBack::BodyScan(scan) if scan.output_completion_failed())
        });
        let each_child_signs = self
            .scheduler
            .drivers()
            .iter()
            .filter_map(|driver| match driver {
                InstalledBack::BodyEach(each) => Some(each.as_ref()),
                _ => None,
            })
            .try_fold(Vec::new(), |mut receipts, each| {
                receipts.extend(each.child_sign_receipts()?);
                Ok(receipts)
            });
        let each_cancellation_failed = self.scheduler.drivers().iter().any(
            |driver| matches!(driver,InstalledBack::BodyEach(each) if each.cancellation_failed()),
        );
        let each_output_completion_failed = self.scheduler.drivers().iter().any(|driver| matches!(driver,InstalledBack::BodyEach(each) if each.output_completion_failed()));
        let (fore_deliveries, fore_pending_output) = self.fore.into_custody();
        BodyKernelResult {
            terminal,
            failure,
            kernel_failure,
            cleanup_failure,
            partitions: self.partitions,
            requests: self.requests,
            events: self.scheduler.signs().events().collect(),
            scan_child_signs,
            scan_cancellation_failed,
            scan_output_completion_failed,
            each_child_signs,
            each_cancellation_failed,
            each_output_completion_failed,
            fore_deliveries,
            fore_pending_output,
            clock_observations: self.clock_observations.into_observations(),
            clock_quality,
            clock_execution_bounds: execution_bounds,
        }
    }
}
