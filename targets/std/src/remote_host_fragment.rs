//! Host-owned admission for one long-lived remote kernel fragment.
//!
//! The generic remote kernel owns execution and Cord lifecycle. `StdHost`
//! remains the owner of exact capability instances and resource pools, so a
//! remote fragment cannot bypass the same pre-Play reservation boundary used
//! by local execution.

use crate::{kernel_preparation::KernelResourceReservation, InstalledRemoteFragment, StdHost};
use conduit_core::{ActivePlayIdentity, PlanFragment};
use conduit_kernel::scheduler::HostOperationRequest;
use core::ops::{Deref, DerefMut};

pub struct AdmittedRemoteFragment {
    runtime: InstalledRemoteFragment,
    reservation: Option<KernelResourceReservation>,
    identity: ActivePlayIdentity,
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
    pub fn poll_remote_body_conversation_context(
        &self,
        fragment: &mut AdmittedRemoteFragment,
    ) -> Result<bool, String> {
        fragment
            .runtime
            .poll_body_conversation_context(self.body_conversation_context.as_ref())
    }

    pub fn complete_remote_voice_host_operation<F>(
        &mut self,
        fragment: &mut AdmittedRemoteFragment,
        request: HostOperationRequest,
        cancelled: F,
    ) -> Result<bool, String>
    where
        F: Fn() -> bool + Copy,
    {
        fragment.runtime.complete_voice_provider_host_operation(
            request,
            self.speech_recognition.as_mut(),
            self.local_model.as_deref_mut(),
            self.speech_synthesis.as_mut(),
            cancelled,
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
