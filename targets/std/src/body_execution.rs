//! Local Body-wide execution through the installed std kernel.
use crate::{
    hosted_keyboard::HostedKeyboardAdapter, installed_std::body_kernel::BodyKernel, RunControl,
    StdHost, TimerAdapter,
};
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::{bind_sign, SignIdentity, TerminalDisposition};
use conduit_kernel::{scheduler::HostOperationRequest, KernelEvent};
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
    pub terminal: TerminalDisposition,
    pub failure: Option<String>,
    pub cleanup_failure: Option<String>,
    pub terminal_sign: SignIdentity,
    pub partitions: Vec<KernelIdentityMap>,
    pub requests: Vec<HostOperationRequest>,
    pub kernel_events: Vec<KernelEvent>,
}

impl StdHost {
    /// Execute the exact local workload. Unsupported contracts refuse before
    /// Play; remote, State, fusion and shared-pool composition remain separate.
    pub fn run_body_plan_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
    ) -> Result<BodyRunReport, String> {
        request
            .plan
            .validate_for(request.wake)
            .map_err(|error| format!("Body Plan validation: {error:?}"))?;
        let fragments = request
            .plan
            .forms
            .iter()
            .map(|partition| {
                if partition.plan.fragments.len() != 1 {
                    return Err(
                        "local Body execution requires one local fragment per Form".to_string()
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
            let first_sign = self.next_kernel_sign_sequence;
            self.next_kernel_sign_sequence = first_sign
                .checked_add(3)
                .ok_or_else(|| "Body Sign sequence exhausted".to_string())?;
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
                .body_plan_ready(request.plan, sign(first_sign).sign_id)
                .and_then(|wake| {
                    wake.body_play_started(request.plan, &play, sign(first_sign + 1).sign_id)
                })
                .map_err(|error| format!("Body start lifecycle: {error:?}"))?;
            let terminal_sign = sign(first_sign + 2);
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
        let mut release_error = None;
        for reservation in reservations {
            if let Err(error) = self.kernel_resources.release(reservation) {
                release_error.get_or_insert(error);
            }
        }
        if let Some(error) = release_error {
            return Err(format!("Body reservation release: {error}"));
        }
        result
    }
}
