//! One complete protocol workset, using the canonical Body lifecycle and kernel.
use super::PreparedProtocolArtifact;
use crate::{
    i2c_base::I2cProvider,
    monotonic_clock::owner::MonotonicDeadlineProvider,
    protocol_host_calls::ProtocolCallRefusal,
    protocol_play::{ClockAdmission, I2cAdmission, PreparedTimedProtocolPlay},
};
use alloc::boxed::Box;
use conduit_body::{BodyLifecycleSession, BodyLifecycleSessionError, BodyPlayIdentity};
use conduit_composite::KernelCompositeStatus;
use conduit_core::{BootId, HostId, PortId, ValuePayload, bind_sign};

#[derive(Debug)]
pub enum ProtocolBodyRefusal {
    UnsupportedWorkset,
    Lifecycle(BodyLifecycleSessionError),
    Call(ProtocolCallRefusal),
    NotPlaying,
}

/// A refused preparation returns the untouched authoritative biography to its owner.
#[derive(Debug)]
pub struct ProtocolBodyPreparationRefusal {
    pub session: BodyLifecycleSession,
    pub reason: ProtocolBodyRefusal,
}

/// The first native protocol profile executes exactly one resident Source Plot.
/// It cannot silently omit other residents or independently selected Mask chains.
/// Preparation allocates the start biography before invoking the only kernel.
pub struct PreparedProtocolBodyPlay<P, C> {
    session: BodyLifecycleSession,
    start_session: Option<BodyLifecycleSession>,
    kernel: Option<PreparedTimedProtocolPlay<P, C>>,
    host: HostId,
    boot: BootId,
}

impl<P: I2cProvider, C: MonotonicDeadlineProvider> PreparedProtocolBodyPlay<P, C> {
    pub fn prepare(
        artifact: PreparedProtocolArtifact,
        session: BodyLifecycleSession,
        play_sequence: u64,
        bus: I2cAdmission<P>,
        clock: ClockAdmission<C>,
    ) -> Result<Self, Box<ProtocolBodyPreparationRefusal>> {
        let prepared = (|| {
            validate_workset(&session, &artifact.body_partition())?;
            let current = session.realization().expect("validated proposal");
            let fragments = &artifact.artifact().definition().internal_plan.fragments;
            let [fragment] = fragments.as_slice() else {
                return Err(ProtocolBodyRefusal::UnsupportedWorkset);
            };
            let host = fragment.host_id.clone();
            let boot = fragment.boot_id.clone();
            let play = BodyPlayIdentity::bind(&current.plan, play_sequence);
            let sign =
                |sequence| bind_sign(&host, &boot, Some(&play.active_play_id), sequence).sign_id;
            let wake = current
                .wake
                .body_plan_ready(&current.plan, sign(0))
                .and_then(|wake| wake.body_play_started(&current.plan, &play, sign(1)))
                .map_err(|error| {
                    ProtocolBodyRefusal::Lifecycle(BodyLifecycleSessionError::Lifecycle(error))
                })?;
            let mut start_session = session.clone();
            start_session
                .started(&host, &boot, play, wake)
                .map_err(ProtocolBodyRefusal::Lifecycle)?;
            let kernel = artifact
                .prepare_timed(bus, clock)
                .map_err(ProtocolBodyRefusal::Call)?;
            Ok::<_, ProtocolBodyRefusal>((start_session, kernel, host, boot))
        })();
        match prepared {
            Ok((start_session, kernel, host, boot)) => Ok(Self {
                session,
                start_session: Some(start_session),
                kernel: Some(kernel),
                host,
                boot,
            }),
            Err(reason) => Err(Box::new(ProtocolBodyPreparationRefusal { session, reason })),
        }
    }

    pub fn session(&self) -> &BodyLifecycleSession {
        &self.session
    }

    /// Inspection exposes the same retained kernel, never a mutable execution path.
    pub fn kernel(&self) -> Option<&conduit_composite::KernelCompositeHost> {
        self.kernel.as_ref().map(PreparedTimedProtocolPlay::kernel)
    }

    pub fn start(&mut self) -> Result<(), ProtocolBodyRefusal> {
        let next = self
            .start_session
            .take()
            .ok_or(ProtocolBodyRefusal::NotPlaying)?;
        let kernel = self
            .kernel
            .as_mut()
            .ok_or(ProtocolBodyRefusal::NotPlaying)?;
        if let Err(error) = kernel.start() {
            // A failed start cannot be retried with the same native possession.
            // Keep the proposal inspectable until explicit cancellation settles it.
            kernel.cancel().map_err(ProtocolBodyRefusal::Call)?;
            return Err(ProtocolBodyRefusal::Call(error));
        }
        self.session = next;
        Ok(())
    }

    fn playing(&mut self) -> Result<&mut PreparedTimedProtocolPlay<P, C>, ProtocolBodyRefusal> {
        if self
            .session
            .realization()
            .and_then(|current| current.play.as_ref())
            .is_none()
        {
            return Err(ProtocolBodyRefusal::NotPlaying);
        }
        self.kernel.as_mut().ok_or(ProtocolBodyRefusal::NotPlaying)
    }

    /// Advance only the already admitted kernel. Completion is a kernel receipt;
    /// the owner explicitly cancels/retires before recording Lull and its biography.
    pub fn step(&mut self) -> Result<KernelCompositeStatus, ProtocolBodyRefusal> {
        self.playing()?.step().map_err(ProtocolBodyRefusal::Call)
    }

    pub fn has_pending_clock(&self) -> bool {
        self.kernel
            .as_ref()
            .is_some_and(PreparedTimedProtocolPlay::has_pending_clock)
    }

    pub fn cancel(&mut self) -> Result<(), ProtocolBodyRefusal> {
        let kernel = self
            .kernel
            .as_mut()
            .ok_or(ProtocolBodyRefusal::NotPlaying)?;
        kernel.cancel().map_err(ProtocolBodyRefusal::Call)?;
        let play = self
            .session
            .realization()
            .and_then(|current| current.play.clone());
        self.session
            .lull(&self.host, &self.boot, play.as_ref())
            .map_err(ProtocolBodyRefusal::Lifecycle)?;
        self.kernel = None;
        self.start_session = None;
        Ok(())
    }

    pub fn admit_input(
        &mut self,
        port: &PortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<conduit_kernel::scheduler::RemoteIngressOutcome, ProtocolBodyRefusal> {
        self.playing()?
            .admit_input(port, sequence, value)
            .map_err(ProtocolBodyRefusal::Call)
    }
    pub fn close_input(&mut self, port: &PortId) -> Result<(), ProtocolBodyRefusal> {
        self.playing()?
            .close_input(port)
            .map_err(ProtocolBodyRefusal::Call)
    }
    pub fn output_into(
        &mut self,
        port: &PortId,
        output: &mut ValuePayload,
    ) -> Result<Option<u64>, ProtocolBodyRefusal> {
        self.playing()?
            .output_into(port, output)
            .map_err(ProtocolBodyRefusal::Call)
    }
    pub fn complete_output(
        &mut self,
        port: &PortId,
        sequence: u64,
    ) -> Result<(), ProtocolBodyRefusal> {
        self.playing()?
            .complete_output(port, sequence)
            .map_err(ProtocolBodyRefusal::Call)
    }
}

fn validate_workset(
    session: &BodyLifecycleSession,
    partition: &conduit_body::BodyPlotPlan,
) -> Result<(), ProtocolBodyRefusal> {
    let current = session.realization().ok_or(ProtocolBodyRefusal::Lifecycle(
        BodyLifecycleSessionError::NoProposal,
    ))?;
    if current.play.is_some() {
        return Err(ProtocolBodyRefusal::Lifecycle(
            BodyLifecycleSessionError::AlreadyPlaying,
        ));
    }
    current
        .plan
        .validate_for(&current.wake)
        .map_err(|error| ProtocolBodyRefusal::Lifecycle(BodyLifecycleSessionError::Plan(error)))?;
    if current.plan.plots.len() != 1
        || current.plan.plots.first() != Some(partition)
        || !current.plan.mask_topologies.is_empty()
    {
        return Err(ProtocolBodyRefusal::UnsupportedWorkset);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
