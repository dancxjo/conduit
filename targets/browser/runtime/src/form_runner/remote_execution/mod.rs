//! One exact admitted WebRTC fragment using the installed production kernel.
//! This owns no transport, planner, membership authority, or Body coordinator.
//! Currently test-scoped conformance scaffolding: no production ABI or live
//! transport is attached, and passing these tests is not remote-browser proof.
mod preparation;
#[cfg(test)]
mod tests;

use super::engine::{self, DriveStatus, PendingHostEffect, TourScheduler};
use conduit_core::{
    ActivePlayId, HostAdvertisement, Plan, PlanFragment, ResourceAdmissionOwner,
    ResourceObservation,
};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_plan_lowering::lowering::{LoweredRemoteEndpoint, RemoteCordDirection};
use conduit_wire::SessionBinding;

pub(super) struct RemoteExecution {
    scheduler: TourScheduler,
    fragment: PlanFragment,
    remote: LoweredRemoteEndpoint,
    _resources: ResourceAdmissionOwner,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RemoteOffer {
    pub sequence: u64,
    pub payload: Vec<u8>,
}

impl RemoteExecution {
    /// Wire currently binds a single-Form Play at sequence zero. A Body Play
    /// identity is deliberately not accepted as an interchangeable identity.
    pub(super) fn prepare(
        plan: &Plan,
        host: &HostAdvertisement,
        binding: &SessionBinding,
        active_play_id: &ActivePlayId,
        observations: &[ResourceObservation],
    ) -> Result<Self, String> {
        let fragment = preparation::validate(plan, host, binding, active_play_id)?;
        let resources = preparation::admit(fragment, host, observations)?;
        let (scheduler, lowered) = engine::prepare_remote_fragment(fragment)?;
        let remote = lowered
            .remote_endpoints
            .first()
            .ok_or("missing remote endpoint")?
            .clone();
        if remote.connection_id != binding.connection_id
            || remote.source_fragment_id != binding.source_fragment_id
            || remote.sink_fragment_id != binding.sink_fragment_id
            || remote.value_kind != binding.value_kind
        {
            return Err("lowered remote endpoint differs from the exact grant".into());
        }
        Ok(Self {
            scheduler,
            fragment: fragment.clone(),
            remote,
            _resources: resources,
        })
    }

    pub(super) fn drive(&mut self) -> Result<DriveStatus, String> {
        engine::drive_remote(
            &mut self.scheduler,
            &self.fragment,
            self.remote.endpoint,
            self.remote.cord,
            self.remote.direction == RemoteCordDirection::Egress,
        )
    }

    pub(super) fn complete_effect(
        &mut self,
        pending: &PendingHostEffect,
        output: Option<&[u8]>,
    ) -> Result<(), String> {
        match output {
            Some(bytes) => {
                engine::complete_host_effect_with_output(&mut self.scheduler, pending, bytes)
            }
            None => engine::complete_host_effect(&mut self.scheduler, pending),
        }
    }

    fn direction(&self, expected: RemoteCordDirection) -> Result<(), String> {
        if self.remote.direction != expected {
            return Err("wrong remote endpoint direction".into());
        }
        Ok(())
    }

    pub(super) fn offer(&mut self) -> Result<Option<RemoteOffer>, String> {
        self.direction(RemoteCordDirection::Egress)?;
        self.scheduler
            .remote_egress_offer(self.remote.endpoint, self.remote.cord)
            .map_err(debug)?
            .map(|offer| {
                Ok(RemoteOffer {
                    sequence: offer.sequence,
                    payload: self
                        .scheduler
                        .host_value(offer.value)
                        .map_err(debug)?
                        .to_vec(),
                })
            })
            .transpose()
    }

    pub(super) fn admit(
        &mut self,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        self.direction(RemoteCordDirection::Ingress)?;
        self.scheduler
            .admit_remote_input(self.remote.endpoint, self.remote.cord, sequence, bytes)
            .map_err(debug)
    }

    pub(super) fn accepted(&mut self, sequence: u64) -> Result<(), String> {
        self.direction(RemoteCordDirection::Egress)?;
        self.scheduler
            .remote_egress_accept(self.remote.endpoint, self.remote.cord, sequence)
            .map_err(debug)
    }

    pub(super) fn delivered(&mut self, sequence: u64) -> Result<(), String> {
        self.direction(RemoteCordDirection::Egress)?;
        self.scheduler
            .remote_egress_delivered(self.remote.endpoint, self.remote.cord, sequence)
            .map_err(debug)
    }

    pub(super) fn close_input(&mut self) -> Result<(), String> {
        self.direction(RemoteCordDirection::Ingress)?;
        self.scheduler
            .close_remote_input(self.remote.endpoint, self.remote.cord)
            .map_err(debug)
    }

    pub(super) fn terminal(&mut self) -> Result<bool, String> {
        self.direction(RemoteCordDirection::Egress)?;
        self.scheduler
            .remote_egress_terminal(self.remote.endpoint, self.remote.cord)
            .map_err(debug)
    }

    pub(super) fn cancel(&mut self) -> Result<(), String> {
        self.scheduler.cancel().map_err(debug)
    }
}

fn debug(error: impl core::fmt::Debug) -> String {
    format!("remote kernel: {error:?}")
}
