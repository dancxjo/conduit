//! Local body-wide execution through the installed std kernel.
use crate::{
    hosted_keyboard::HostedKeyboardAdapter, installed_std::body_kernel::BodyKernel, RunControl,
    StdHost, TimerAdapter,
};
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::{bind_sign, SignIdentity, TerminalDisposition};
use conduit_kernel::{scheduler::HostCallRequest, KernelEvent};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use std::io::Write;

pub struct BodyRunRequest<'a> {
    pub wake: &'a Wake,
    pub plan: &'a BodyPlan,
    pub control: &'a RunControl,
    pub keyboard: Option<&'a mut dyn HostedKeyboardAdapter>,
}

#[derive(Debug)]
pub struct BodyRunReport {
    pub play: BodyPlayIdentity,
    /// Historical lifecycle record at start, not a current liveness claim.
    pub wake_at_start: Wake,
    /// Kernel execution disposition; cleanup may independently fail afterward.
    pub terminal: TerminalDisposition,
    pub failure: Option<String>,
    pub cleanup_failure: Option<String>,
    pub terminal_sign: SignIdentity,
    pub partitions: Vec<KernelIdentityMap>,
    pub requests: Vec<HostCallRequest>,
    pub kernel_events: Vec<KernelEvent>,
}

impl StdHost {
    /// Execute the exact local workload. Unsupported contracts refuse before
    /// Play; remote, State, fusion and shared-pool composition remain separate.
    /// `Ok` returns execution evidence, not a success claim: callers must inspect
    /// both `terminal` and `cleanup_failure`. Post-execution cleanup never erases
    /// the report or replaces the original execution failure.
    pub fn run_body_plan_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
    ) -> Result<BodyRunReport, String> {
        self.run_body_plan_to_with_start(request, output, timer, |_, _| Ok(()))
    }

    /// Publish the exact admitted Play before the kernel advances. A caller may
    /// durably retain the start and refuse execution if that publication fails.
    /// The callback is never invoked for preparation or reservation refusal.
    pub fn run_body_plan_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        mut started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        request
            .plan
            .validate_for(request.wake)
            .map_err(|error| format!("Body Plan validation: {error:?}"))?;
        let fragments = request
            .plan
            .plots
            .iter()
            .map(|partition| {
                if partition.plan.fragments.len() != 1 {
                    return Err(
                        "local body execution requires one local fragment per Plot".to_string()
                    );
                }
                Ok(&partition.plan.fragments[0])
            })
            .collect::<Result<Vec<_>, _>>()?;
        let kernel = BodyKernel::prepare(&fragments, request.keyboard.is_some())?;
        let reservations = self.kernel_resources.prepare_and_reserve_partitions(
            &self.advertisement,
            &fragments
                .iter()
                .map(|part| (*part, false))
                .collect::<Vec<_>>(),
        )?;
        let result = (|| {
            let sequence = self.next_kernel_play_sequence;
            self.next_kernel_play_sequence = sequence
                .checked_add(1)
                .ok_or_else(|| "Body Play sequence exhausted".to_string())?;
            let play = BodyPlayIdentity::bind(request.plan, sequence);
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
            started(&play, &wake_at_start)?;
            let terminal_sign = sign(2);
            let result = kernel.run(output, timer, request.keyboard, request.control);
            Ok(BodyRunReport {
                play,
                wake_at_start,
                terminal: result.terminal,
                failure: result.failure,
                cleanup_failure: result.cleanup_failure,
                terminal_sign,
                partitions: result.partitions,
                requests: result.requests,
                kernel_events: result.events,
            })
        })();
        let mut release_errors = Vec::new();
        for reservation in reservations {
            if let Err(error) = self.kernel_resources.release(reservation) {
                release_errors.push(error);
            }
        }
        finish_body_release(result, release_errors)
    }
}

pub(crate) fn finish_body_release(
    result: Result<BodyRunReport, String>,
    release_errors: Vec<String>,
) -> Result<BodyRunReport, String> {
    if release_errors.is_empty() {
        return result;
    }
    let release_failure = format!("Body reservation release: {}", release_errors.join("; "));
    match result {
        Ok(mut report) => {
            report.cleanup_failure = Some(match report.cleanup_failure.take() {
                Some(previous) => format!("{previous}; {release_failure}"),
                None => release_failure,
            });
            Ok(report)
        }
        Err(original) => Err(format!("{original}; {release_failure}")),
    }
}
