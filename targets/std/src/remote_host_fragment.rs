//! host-owned admission for one long-lived remote kernel fragment.
//!
//! The generic remote kernel owns execution and Cord lifecycle. `StdHost`
//! remains the owner of exact capability instances and resource pools, so a
//! remote fragment cannot bypass the same pre-play reservation boundary used
//! by local execution.

use crate::{
    kernel_preparation::KernelResourceReservation, InstalledRemoteFragment, StdHost, TimerAdapter,
};
use conduit_body::{BodyHostTimeAdmission, BodyPlan, BodyPlanTimeAdmission};
use conduit_core::{
    ActivePlayIdentity, BodyTimeQuality, BodyTimeRefusal, BodyTimeRequirement, PlanFragment,
};
use conduit_kernel::scheduler::{HostCallRequest, SchedulerStatus};
use core::ops::{Deref, DerefMut};

pub struct AdmittedRemoteFragment {
    runtime: InstalledRemoteFragment,
    reservation: Option<KernelResourceReservation>,
    identity: ActivePlayIdentity,
    body_time: Option<(BodyTimeRequirement, BodyHostTimeAdmission)>,
    clock_quality: Option<BodyTimeQuality>,
    last_clock_sample: Option<conduit_core::MonotonicInstant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum RemoteBodyTimeStepRefusal {
    NotQualified,
    Quality(BodyTimeQuality),
    Kernel(String),
}

impl AdmittedRemoteFragment {
    pub fn runtime(&self) -> &InstalledRemoteFragment {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut InstalledRemoteFragment {
        &mut self.runtime
    }

    pub fn identity(&self) -> &ActivePlayIdentity {
        &self.identity
    }

    pub fn clock_quality(&self) -> Option<&BodyTimeQuality> {
        self.clock_quality.as_ref()
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, String> {
        self.runtime.step()
    }

    #[allow(clippy::result_large_err)]
    pub fn step_with_body_time<T: TimerAdapter>(
        &mut self,
        timer: &mut T,
    ) -> Result<SchedulerStatus, RemoteBodyTimeStepRefusal> {
        let (requirement, admission) = self
            .body_time
            .as_ref()
            .ok_or(RemoteBodyTimeStepRefusal::NotQualified)?;
        if let Some(quality) = self.clock_quality.as_ref() {
            if !matches!(quality, BodyTimeQuality::Ready { .. }) {
                return Err(RemoteBodyTimeStepRefusal::Quality(quality.clone()));
            }
        }
        let quality = match timer.monotonic_observation(admission.host_id(), admission.boot_id()) {
            Some(sample) => {
                if self
                    .last_clock_sample
                    .as_ref()
                    .is_some_and(|previous| previous.clock() != sample.clock())
                {
                    BodyTimeQuality::Unsupported {
                        reason: BodyTimeRefusal::DifferentClock,
                    }
                } else if self
                    .last_clock_sample
                    .as_ref()
                    .is_some_and(|previous| sample.ticks() < previous.ticks())
                {
                    BodyTimeQuality::Unsupported {
                        reason: BodyTimeRefusal::Regressed,
                    }
                } else {
                    let (transport, scheduler) = admission.execution_bounds();
                    let quality = requirement.assess_with_execution_bounds(
                        admission.correlation(),
                        &sample,
                        transport,
                        scheduler,
                    );
                    if matches!(quality, BodyTimeQuality::Ready { .. }) {
                        self.last_clock_sample = Some(sample);
                    }
                    quality
                }
            }
            None => BodyTimeQuality::Unsupported {
                reason: BodyTimeRefusal::Unavailable,
            },
        };
        self.clock_quality = Some(quality.clone());
        if !matches!(quality, BodyTimeQuality::Ready { .. }) {
            return Err(RemoteBodyTimeStepRefusal::Quality(quality));
        }
        self.runtime
            .step_body_time_admitted()
            .map_err(RemoteBodyTimeStepRefusal::Kernel)
    }
}

impl Deref for AdmittedRemoteFragment {
    type Target = InstalledRemoteFragment;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl DerefMut for AdmittedRemoteFragment {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runtime
    }
}

impl StdHost {
    pub fn prepare_remote_fragment_with_body_time<T: TimerAdapter>(
        &mut self,
        plan: &BodyPlan,
        admission: &BodyPlanTimeAdmission,
        fragment: &PlanFragment,
        timer: &mut T,
    ) -> Result<AdmittedRemoteFragment, String> {
        plan.verify_seal()
            .map_err(|error| format!("Body Plan validation: {error:?}"))?;
        if admission.plan_id() != &plan.plan_id {
            return Err("BodyTime admission belongs to a different Body Plan".into());
        }
        if !plan
            .plots
            .iter()
            .any(|plot| plot.plan.fragments.contains(fragment))
            && !plan.mask_topologies.iter().any(|topology| {
                topology
                    .chains
                    .iter()
                    .any(|chain| chain.plan.fragments.contains(fragment))
            })
        {
            return Err("remote fragment is not in the admitted Body Plan".into());
        }
        let requirement = plan
            .body_time_requirement
            .as_ref()
            .ok_or_else(|| "remote Body Plan has no BodyTime requirement".to_string())?;
        let local = admission
            .for_host(&self.advertisement.host_id, &self.advertisement.boot_id)
            .ok_or_else(|| "BodyTime admission does not include this Host/Boot".to_string())?;
        let sample = timer
            .monotonic_observation(&self.advertisement.host_id, &self.advertisement.boot_id)
            .ok_or_else(|| "remote BodyTime fragment has no local clock".to_string())?;
        let (transport, scheduler) = local.execution_bounds();
        match requirement.assess_with_execution_bounds(
            local.correlation(),
            &sample,
            transport,
            scheduler,
        ) {
            quality @ BodyTimeQuality::Ready { .. } => {
                let mut prepared = self.prepare_remote_fragment(fragment)?;
                prepared.runtime.require_body_time();
                prepared.body_time = Some((requirement.clone(), local.clone()));
                prepared.clock_quality = Some(quality);
                prepared.last_clock_sample = Some(sample);
                Ok(prepared)
            }
            quality => Err(format!("remote BodyTime fragment quality: {quality:?}")),
        }
    }

    pub fn poll_remote_body_conversation_context(
        &self,
        fragment: &mut AdmittedRemoteFragment,
    ) -> Result<bool, String> {
        fragment
            .runtime
            .poll_body_conversation_context(self.body_conversation_context.as_ref())
    }

    pub fn complete_remote_voice_host_call<F>(
        &mut self,
        fragment: &mut AdmittedRemoteFragment,
        request: HostCallRequest,
        cancelled: F,
    ) -> Result<bool, String>
    where
        F: Fn() -> bool + Copy,
    {
        fragment.runtime.complete_voice_provider_host_call(
            request,
            self.speech_recognition.as_mut(),
            self.local_model.as_deref_mut(),
            cancelled,
        )
    }

    pub fn complete_remote_vision_host_call(
        &mut self,
        fragment: &mut AdmittedRemoteFragment,
        request: HostCallRequest,
        observed_at_micros: u64,
    ) -> Result<bool, String> {
        fragment.runtime.complete_vision_host_call(
            request,
            self.vision.as_mut(),
            observed_at_micros,
        )
    }

    pub fn prepare_remote_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<AdmittedRemoteFragment, String> {
        let advertisement = self.advertisement().clone();
        let reservation = self
            .kernel_resources
            .prepare_and_reserve(&advertisement, fragment)?;
        let play = match self.issue_kernel_play(fragment) {
            Ok(play) => play,
            Err(error) => {
                self.kernel_resources.release(reservation)?;
                return Err(error);
            }
        };
        let runtime = match InstalledRemoteFragment::prepare(
            &advertisement,
            fragment,
            play.identity().play_sequence,
        ) {
            Ok(runtime) => runtime,
            Err(error) => {
                self.kernel_resources.release(reservation)?;
                return Err(error);
            }
        };
        Ok(AdmittedRemoteFragment {
            runtime,
            reservation: Some(reservation),
            identity: play.identity().clone(),
            body_time: None,
            clock_quality: None,
            last_clock_sample: None,
        })
    }

    pub fn release_remote_fragment(
        &mut self,
        mut fragment: AdmittedRemoteFragment,
    ) -> Result<(), String> {
        let reservation = fragment
            .reservation
            .take()
            .ok_or_else(|| "remote fragment reservation was already released".to_string())?;
        self.kernel_resources.release(reservation)
    }
}
