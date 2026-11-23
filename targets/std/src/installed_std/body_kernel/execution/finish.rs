//! Shared terminal finalization of the ordinary Body loop.
use super::state::BodyExecutionState;
use super::*;
impl BodyKernel<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish<T: TimerAdapter>(
        mut self,
        mut state: BodyExecutionState<'_, '_>,
        result: Result<TerminalDisposition, String>,
        clock: &mut T,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
        execution_bounds: Option<(
            conduit_core::MonotonicDuration,
            conduit_core::MonotonicDuration,
        )>,
        mut terminal: Option<crate::body_execution::ParsecTerminalAttachment<'_>>,
        creche: Option<&mut crate::hosted_creche::CrecheHost<'_>>,
    ) -> BodyKernelResult {
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
        state.keys.set_input_view_epoch(None);
        let result = result.and_then(|outcome| withdrawal.map(|()| outcome));
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
                let error = diagnostic::retained_error(&mut self.error_diagnostic, error);
                state.keys.cancel();
                state.deadlines.clear();
                if retirement.is_ok() {
                    let failure = self.scheduler.cancel().err().map(|error| {
                        diagnostic::error(
                            &mut self.cleanup_diagnostic,
                            format_args!("Body failure cleanup: {error:?}"),
                        )
                    });
                    if cleanup_failure.is_none() {
                        cleanup_failure = failure;
                    }
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
            startup_chimes: self.startup_receipts,
            terminal,
            failure,
            kernel_failure: state.kernel_failure,
            cleanup_failure,
            partitions: self.partitions,
            requests: self.requests,
            events: if let Some(mut prepared) = self.terminal_events.take() {
                prepared.extend(self.scheduler.signs().events());
                prepared
            } else {
                self.scheduler.signs().events().collect()
            },
            scan_child_signs,
            scan_cancellation_failed,
            scan_output_completion_failed,
            each_child_signs,
            each_cancellation_failed,
            each_output_completion_failed,
            fore_deliveries,
            fore_pending_output,
            clock_observations: self.clock_observations.into_observations(),
            clock_quality: state.clock_quality,
            clock_execution_bounds: execution_bounds,
        }
    }
}
