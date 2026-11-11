//! Shared native realization of the shipped startup plot through Body/Plan/Play.
//! Preparation renders finite PCM; only the admitted kernel Host Call plays it.
use crate::{
    identity::BootIdentities,
    offer::HostOffer,
    pcm_output::{PcmOutput, PcmProvider},
};
use alloc::{vec, vec::Vec};
use conduit_body::{BodyLifecycleSession, StartupScope};
use conduit_core::{InfoBool, PlanId};
use conduit_kernel::{
    Failure, FailureCode, HostCallDisposition, HostCallOutcome, scheduler::SchedulerStatus,
};
mod kernel;
mod lifecycle;
mod plan;

pub struct StartupPlot {
    session: BodyLifecycleSession,
    kernel: kernel::Kernel,
    plan_id: PlanId,
    samples: Vec<i16>,
    provider: Option<PcmProvider>,
    finished: bool,
}
pub struct StartupReceipt<'a> {
    pub body_id: &'a conduit_body::BodyId,
    pub plan_id: &'a PlanId,
    pub active_play_id: &'a conduit_core::ActivePlayId,
    pub outcome: &'static str,
    pub frames: u32,
}
impl StartupPlot {
    pub fn prepare(
        ids: &BootIdentities,
        host: &HostOffer<'_>,
        build: &str,
        provider: Option<PcmProvider>,
    ) -> Result<Self, &'static str> {
        if provider.is_some_and(|p| {
            p.provider_generation == 0
                || p.sample_rate != 48_000
                || p.maximum_frames < conduit_synth::STARTUP_CHIME_FRAMES
        }) {
            return Err("startup-pcm-profile");
        }
        let plan = plan::prepare(ids, host, build)?;
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0])
            .map_err(|_| "startup-lowering")?;
        let kernel = kernel::prepare(&plan.fragments[0], &lowered, true)?;
        let mut samples = vec![0_i16; conduit_synth::STARTUP_CHIME_FRAMES as usize];
        let mut renderer = conduit_synth::StartupChime::new();
        for chunk in samples.chunks_mut(256) {
            if renderer.render(chunk).map_err(|_| "startup-pcm-render")? != chunk.len() {
                return Err("startup-pcm-length");
            }
        }
        let plan_id = plan.plan_id.clone();
        let fragment = &plan.fragments[0];
        let session = lifecycle::start(
            plan.clone(),
            &fragment.host_id,
            &fragment.boot_id,
            fragment.offer_generation,
        )?;
        let realization = session.realization().ok_or("startup-lifecycle")?;
        let startup = session
            .evidence()
            .startup_for_play(
                &realization.plan,
                realization.play.as_ref().ok_or("startup-lifecycle")?,
            )
            .map_err(|_| "startup-eligibility")?;
        if !startup.eligible(StartupScope::Wake) {
            return Err("startup-ineligible");
        }
        Ok(Self {
            session,
            kernel,
            plan_id,
            samples,
            provider,
            finished: false,
        })
    }
    pub fn run(
        &mut self,
        mut output: Option<&mut dyn PcmOutput>,
    ) -> Result<StartupReceipt<'_>, &'static str> {
        if self.finished {
            return Err("startup-play-already-finished");
        }
        self.finished = true;
        let mut outcome = "not-requested";
        for _ in 0..64 {
            let status = self.kernel.step().map_err(|_| "startup-kernel-step")?;
            while let Some(request) = self.kernel.next_host_request() {
                if request.call.0 != 0 {
                    return Err("startup-host-call");
                }
                let pulse = InfoBool::decode(
                    self.kernel
                        .host_value(request.input.value)
                        .map_err(|_| "startup-host-value")?,
                )
                .map_err(|_| "startup-host-input")?;
                let failure = if !pulse.get() {
                    outcome = "silent";
                    None
                } else {
                    match output.as_deref_mut() {
                        Some(device) if Some(device.provider()) == self.provider => {
                            match device.play(&self.samples) {
                                Ok(()) => {
                                    outcome = "completed";
                                    None
                                }
                                Err(_) => {
                                    outcome = "failed";
                                    Some(Failure {
                                        code: FailureCode::HostCallFailed,
                                        detail: 1,
                                    })
                                }
                            }
                        }
                        _ => {
                            outcome = "denied";
                            Some(Failure {
                                code: FailureCode::HostCallDenied,
                                detail: 1,
                            })
                        }
                    }
                };
                let disposition = match outcome {
                    "denied" => HostCallDisposition::Denied,
                    "failed" => HostCallDisposition::Failed,
                    _ => HostCallDisposition::Completed,
                };
                self.kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        HostCallOutcome {
                            disposition,
                            output: None,
                            failure,
                        },
                    )
                    .map_err(|_| "startup-host-completion")?;
            }
            if status == SchedulerStatus::Drained {
                let realization = self.session.realization().ok_or("startup-lifecycle")?;
                return Ok(StartupReceipt {
                    body_id: &self.session.evidence().body_id,
                    plan_id: &self.plan_id,
                    active_play_id: &realization
                        .play
                        .as_ref()
                        .ok_or("startup-lifecycle")?
                        .active_play_id,
                    outcome,
                    frames: if outcome == "completed" {
                        self.samples.len() as u32
                    } else {
                        0
                    },
                });
            }
        }
        self.kernel.cancel().map_err(|_| "startup-cancel")?;
        Err("startup-step-budget")
    }
    pub fn biography(&self) -> &conduit_body::BodyBiographyEvidence {
        self.session.evidence()
    }
}

#[cfg(test)]
mod tests;
