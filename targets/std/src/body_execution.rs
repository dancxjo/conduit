//! Local body-wide execution through the installed std kernel.
use crate::{
    hosted_keyboard::HostedKeyboardAdapter, installed_std::body_kernel::BodyKernel,
    BodyLiveForeQueue, BodyLiveForeStatus, ExternalForeDelivery, ExternalForeInput, RunControl,
    StdHost, TimerAdapter,
};
use conduit_body::{BodyPlan, BodyPlanTimeAdmission, BodyPlayIdentity, Wake};
use conduit_core::{
    bind_sign, BodyClockCorrelation, BodyTimeQuality, BodyTimeRefusal, BodyTimeRequirement,
    MonotonicDuration, MonotonicInstant, SignIdentity, TerminalDisposition,
};
use conduit_kernel::{scheduler::HostCallRequest, KernelEvent};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use std::io::Write;
use std::path::Path;

mod todo_checkpoint;

#[derive(Debug, Clone)]
pub struct ObservedKernelEvent {
    pub sequence: u32,
    pub time: crate::body_causal_evidence::BodyEventTimeObservation,
}

pub struct BodyRunRequest<'a> {
    pub wake: &'a Wake,
    pub plan: &'a BodyPlan,
    pub control: &'a RunControl,
    pub keyboard: Option<&'a mut dyn HostedKeyboardAdapter>,
}

/// Acknowledges one bounded Body Fore delivery by borrowing its prepared
/// storage. Returning an error leaves the kernel output unacknowledged.
pub trait BodyForeOutputAdapter {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String>;
}

/// The selected residence and immutable checkpoint namespace for one Play.
pub struct TodoCheckpointSelection<'a> {
    pub root: &'a Path,
    pub identity: crate::todo_durable_resource::CheckpointIdentity,
}

/// The typed input and acknowledged output Fores of one checkpoint Play.
pub struct BodyForeExchange<'a> {
    pub inputs: &'a [ExternalForeInput],
    pub output: &'a mut dyn BodyForeOutputAdapter,
}

/// Wake an external producer on every preparation, admission, and start
/// refusal, including paths before the kernel has begun to run.
struct LiveForeTerminalGuard<'a>(Option<&'a BodyLiveForeQueue>);

impl Drop for LiveForeTerminalGuard<'_> {
    fn drop(&mut self) {
        if let Some(queue) = self.0 {
            queue.mark_play_terminal();
        }
    }
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
    /// Receipt-correlated child Signs; a refusal remains distinct from an
    /// empty child stream and never borrows the parent's Sign identity.
    pub scan_child_signs:
        Result<Vec<conduit_composite::ScanChildSignReceipt>, conduit_composite::BoundedScanError>,
    pub scan_cancellation_failed: bool,
    pub scan_output_completion_failed: bool,
    pub fore_deliveries: Vec<ExternalForeDelivery>,
    /// Host-staged versus kernel-admitted live values at terminal. Neither
    /// count claims that the Todo state transition committed.
    pub live_fore_status: Option<BodyLiveForeStatus>,
    pub clock_observations: Vec<ObservedKernelEvent>,
    pub clock_quality: Option<BodyTimeQuality>,
    pub clock_execution_bounds: Option<(MonotonicDuration, MonotonicDuration)>,
}

pub(crate) fn assess_body_clock(
    requirement: &BodyTimeRequirement,
    correlation: &BodyClockCorrelation,
    sample: &MonotonicInstant,
    execution_bounds: Option<(MonotonicDuration, MonotonicDuration)>,
) -> BodyTimeQuality {
    match execution_bounds {
        Some((transport, scheduler)) => {
            requirement.assess_with_execution_bounds(correlation, sample, transport, scheduler)
        }
        None => requirement.assess(correlation, sample),
    }
}

pub(crate) fn assess_continuing_body_clock(
    requirement: &BodyTimeRequirement,
    correlation: &BodyClockCorrelation,
    sample: &MonotonicInstant,
    execution_bounds: Option<(MonotonicDuration, MonotonicDuration)>,
    previous: Option<&BodyTimeQuality>,
) -> BodyTimeQuality {
    if let Some(BodyTimeQuality::Ready { now, .. }) = previous {
        if sample.clock() != now.local_sample.clock() {
            return BodyTimeQuality::Unsupported {
                reason: BodyTimeRefusal::DifferentClock,
            };
        }
        if sample.ticks() < now.local_sample.ticks() {
            return BodyTimeQuality::Unsupported {
                reason: BodyTimeRefusal::Regressed,
            };
        }
    }
    assess_body_clock(requirement, correlation, sample, execution_bounds)
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

    /// Execute a sealed local Body with caller-supplied, typed Fore values and
    /// an acknowledging output adapter. The exact scoped, finite preloaded
    /// Todo scan is admitted; later Mask actions and other activation routes
    /// remain unavailable.
    pub fn run_body_plan_with_fore_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        sequential: bool,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
    ) -> Result<BodyRunReport, String> {
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            Some((inputs, sequential, fore_output)),
            None,
            None,
            |_, _| Ok(()),
        )
    }

    /// Run one retained Body Play while an admitted typed queue receives
    /// later values. Face/Mask action routing is a separate semantic boundary;
    /// the queue must be closed explicitly to complete.
    pub fn run_body_plan_with_live_fore_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        queue: &BodyLiveForeQueue,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
    ) -> Result<BodyRunReport, String> {
        self.run_body_plan_with_live_fore_to_with_start(
            request,
            queue,
            fore_output,
            output,
            timer,
            |_, _| Ok(()),
        )
    }

    /// Publish the admitted live Play before waiting for later typed actions.
    pub fn run_body_plan_with_live_fore_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        queue: &BodyLiveForeQueue,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            None,
            Some((queue, fore_output)),
            None,
            started,
        )
    }

    /// Publish the exact admitted Play before the kernel advances. A caller may
    /// durably retain the start and refuse execution if that publication fails.
    /// The callback is never invoked for preparation or reservation refusal.
    pub fn run_body_plan_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_to_with_start_and_clock(
            request, output, timer, None, None, None, None, None, started,
        )
    }

    pub fn run_body_plan_to_with_body_time<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        correlation: &BodyClockCorrelation,
    ) -> Result<BodyRunReport, String> {
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            Some(correlation),
            None,
            None,
            None,
            None,
            |_, _| Ok(()),
        )
    }

    pub fn run_body_plan_to_with_body_time_bounds<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        correlation: &BodyClockCorrelation,
        transport_uncertainty: MonotonicDuration,
        scheduler_uncertainty: MonotonicDuration,
    ) -> Result<BodyRunReport, String> {
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            Some(correlation),
            Some((transport_uncertainty, scheduler_uncertainty)),
            None,
            None,
            None,
            |_, _| Ok(()),
        )
    }

    pub fn run_body_plan_to_with_admitted_body_time<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        admission: &BodyPlanTimeAdmission,
    ) -> Result<BodyRunReport, String> {
        if admission.plan_id() != &request.plan.plan_id {
            return Err("BodyTime admission belongs to a different Body Plan".into());
        }
        let local = admission
            .for_host(&self.advertisement.host_id, &self.advertisement.boot_id)
            .ok_or_else(|| "BodyTime admission does not include this Host/Boot".to_string())?;
        let (transport, scheduler) = local.execution_bounds();
        self.run_body_plan_to_with_body_time_bounds(
            request,
            output,
            timer,
            local.correlation(),
            transport,
            scheduler,
        )
    }

    pub fn run_body_plan_to_with_start_and_body_time<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        correlation: &BodyClockCorrelation,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            Some(correlation),
            None,
            None,
            None,
            None,
            started,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_body_plan_to_with_start_and_clock<'a, W: Write, T: TimerAdapter, F>(
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
        let prospective_play = BodyPlayIdentity::bind(request.plan, self.next_kernel_play_sequence);
        let live_queue = live.as_ref().map(|(queue, _)| *queue);
        let mut kernel = if let Some((queue, _)) = &live {
            if request.keyboard.is_some() {
                return Err("live Body Fore cannot also claim keyboard ingress".into());
            }
            BodyKernel::prepare_live(
                &request.plan.plots,
                &prospective_play.active_play_id,
                queue,
                request.control,
            )?
        } else {
            BodyKernel::prepare(
                &request.plan.plots,
                request.keyboard.is_some(),
                &prospective_play.active_play_id,
                fore_inputs,
                sequential_fore,
                has_fore_output,
            )?
        };
        if let Some((root, checkpoint)) = checkpoint {
            kernel.attach_todo_checkpoint(&request.plan.plots, root, checkpoint)?;
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
            );
            if let Some(queue) = live_queue {
                queue.mark_play_terminal();
            }
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
                scan_child_signs: result.scan_child_signs,
                scan_cancellation_failed: result.scan_cancellation_failed,
                scan_output_completion_failed: result.scan_output_completion_failed,
                fore_deliveries: result.fore_deliveries,
                live_fore_status: live_queue.map(BodyLiveForeQueue::status),
                clock_observations: result.clock_observations,
                clock_quality: result.clock_quality,
                clock_execution_bounds: result.clock_execution_bounds,
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        BodyTimeTolerance, BootId, ClockProvenance, HostId, MonotonicClockIdentity, TemporalScale,
    };

    #[test]
    fn continuing_body_clock_refuses_regression_and_basis_replacement() {
        let clock = MonotonicClockIdentity::new(
            HostId::from("host/a"),
            BootId::from("boot/a"),
            "steady".into(),
            TemporalScale::Milliseconds,
            1,
            1,
        )
        .unwrap();
        let initial = MonotonicInstant::new(1_000, clock.clone()).unwrap();
        let correlation = BodyClockCorrelation::new(
            "body/a".into(),
            TemporalScale::Milliseconds,
            1,
            initial.clone(),
            10_000,
            0,
            100,
            1,
            10_000,
            ClockProvenance::External {
                provider_id: "test/source".into(),
                admission_reference: "test/admission".into(),
                policy_id: "test/policy".into(),
            },
        )
        .unwrap();
        let requirement = BodyTimeRequirement::new(
            "body/a".into(),
            BodyTimeTolerance::new(10, TemporalScale::Milliseconds),
            MonotonicDuration::new(100, TemporalScale::Milliseconds),
        )
        .unwrap();
        let ready = assess_body_clock(&requirement, &correlation, &initial, None);
        assert!(matches!(ready, BodyTimeQuality::Ready { .. }));
        assert_eq!(
            assess_continuing_body_clock(
                &requirement,
                &correlation,
                &MonotonicInstant::new(999, clock.clone()).unwrap(),
                None,
                Some(&ready),
            ),
            BodyTimeQuality::Unsupported {
                reason: BodyTimeRefusal::Regressed,
            }
        );
        let replacement = MonotonicClockIdentity::new(
            HostId::from("host/a"),
            BootId::from("boot/b"),
            "steady".into(),
            TemporalScale::Milliseconds,
            1,
            1,
        )
        .unwrap();
        assert_eq!(
            assess_continuing_body_clock(
                &requirement,
                &correlation,
                &MonotonicInstant::new(1_001, replacement).unwrap(),
                None,
                Some(&ready),
            ),
            BodyTimeQuality::Unsupported {
                reason: BodyTimeRefusal::DifferentClock,
            }
        );
    }
}
