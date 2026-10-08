//! Local body-wide execution through the installed std kernel.
use crate::{
    hosted_keyboard::HostedKeyboardAdapter, installed_std::body_kernel::BodyKernel,
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, RunControl, StdHost,
    TimerAdapter,
};
use conduit_body::{BodyPlan, BodyPlanTimeAdmission, BodyPlayIdentity, Wake};
use conduit_core::{
    bind_sign, BodyClockCorrelation, BodyTimeQuality, BodyTimeRefusal, BodyTimeRequirement,
    MonotonicDuration, MonotonicInstant, SignIdentity, TerminalDisposition,
};
use conduit_kernel::{scheduler::HostCallRequest, KernelEvent};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use std::io::Write;

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
    pub fore_deliveries: Vec<ExternalForeDelivery>,
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
    /// an acknowledging output adapter. This entrance still refuses activation
    /// coordinators until their scheduler Back is installed.
    pub fn run_body_plan_with_fore_to<W: Write, T: TimerAdapter>(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        sequential: bool,
        fore_output: &mut dyn ExternalForeOutputAdapter,
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
            |_, _| Ok(()),
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
            request, output, timer, None, None, None, started,
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
            started,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_body_plan_to_with_start_and_clock<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        correlation: Option<&BodyClockCorrelation>,
        execution_bounds: Option<(MonotonicDuration, MonotonicDuration)>,
        fore: Option<(
            &[ExternalForeInput],
            bool,
            &mut dyn ExternalForeOutputAdapter,
        )>,
        mut started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
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
        let (fore_inputs, sequential_fore, has_fore_output) = fore
            .as_ref()
            .map_or((&[][..], false, false), |(inputs, sequential, _)| {
                (*inputs, *sequential, true)
            });
        let kernel = BodyKernel::prepare(
            &request.plan.plots,
            request.keyboard.is_some(),
            fore_inputs,
            sequential_fore,
            has_fore_output,
        )?;
        kernel.require_supported_execution()?;
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
            let result = kernel.run(
                output,
                timer,
                request.keyboard,
                fore.map(|(_, _, adapter)| adapter),
                request.control,
                &self.advertisement.host_id,
                &self.advertisement.boot_id,
                request.plan.body_time_requirement.as_ref().zip(correlation),
                execution_bounds,
                admitted_clock_quality,
            );
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
                fore_deliveries: result.fore_deliveries,
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
