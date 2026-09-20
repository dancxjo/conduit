//! One exact admitted remote fragment using the installed production kernel.
//! This owns no transport, planner, membership authority, or Body coordinator.
mod abi;
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
    remotes: Vec<LoweredRemoteEndpoint>,
    _resources: ResourceAdmissionOwner,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RemoteOffer {
    pub sequence: u64,
    pub payload: Vec<u8>,
}

impl RemoteExecution {
    /// Wire currently binds a single-Form Play at sequence zero. A body Play
    /// identity is deliberately not accepted as an interchangeable identity.
    pub(super) fn prepare(
        plan: &Plan,
        host: &HostAdvertisement,
        bindings: &[SessionBinding],
        active_play_id: &ActivePlayId,
        observations: &[ResourceObservation],
    ) -> Result<Self, String> {
        let fragment = preparation::validate(plan, host, bindings, active_play_id)?;
        let resources = preparation::admit(fragment, host, observations)?;
        let (scheduler, lowered) = engine::prepare_remote_fragment(fragment)?;
        if lowered.remote_endpoints.len() != bindings.len() || bindings.is_empty() {
            return Err("remote endpoint count differs from the exact grants".into());
        }
        for remote in &lowered.remote_endpoints {
            let matches = bindings.iter().filter(|binding| {
                remote.connection_id == binding.connection_id
                    && remote.source_fragment_id == binding.source_fragment_id
                    && remote.sink_fragment_id == binding.sink_fragment_id
                    && remote.value_kind == binding.value_kind
            });
            if matches.count() != 1 {
                return Err("lowered remote endpoint differs from the exact grants".into());
            }
        }
        Ok(Self {
            scheduler,
            fragment: fragment.clone(),
            remotes: lowered.remote_endpoints,
            _resources: resources,
        })
    }

    pub(super) fn drive(&mut self) -> Result<DriveStatus, String> {
        let egress = self
            .remotes
            .iter()
            .filter(|remote| remote.direction == RemoteCordDirection::Egress)
            .map(|remote| (remote.endpoint, remote.cord))
            .collect::<Vec<_>>();
        engine::drive_remote(&mut self.scheduler, &self.fragment, &egress)
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

    fn remote(
        &self,
        endpoint: conduit_kernel::RemoteEndpointId,
        expected: RemoteCordDirection,
    ) -> Result<&LoweredRemoteEndpoint, String> {
        let remote = self
            .remotes
            .iter()
            .find(|remote| remote.endpoint == endpoint)
            .ok_or_else(|| "unknown remote endpoint".to_string())?;
        if remote.direction != expected {
            return Err("wrong remote endpoint direction".into());
        }
        Ok(remote)
    }

    pub(super) fn direction(
        &self,
        endpoint: conduit_kernel::RemoteEndpointId,
    ) -> Result<RemoteCordDirection, String> {
        self.remotes
            .iter()
            .find(|remote| remote.endpoint == endpoint)
            .map(|remote| remote.direction)
            .ok_or_else(|| "unknown remote endpoint".to_string())
    }

    pub(super) fn offer(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
    ) -> Result<Option<RemoteOffer>, String> {
        let cord = self.remote(endpoint, RemoteCordDirection::Egress)?.cord;
        self.scheduler
            .remote_egress_offer(endpoint, cord)
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
        endpoint: conduit_kernel::RemoteEndpointId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        let cord = self.remote(endpoint, RemoteCordDirection::Ingress)?.cord;
        self.scheduler
            .admit_remote_input(endpoint, cord, sequence, bytes)
            .map_err(debug)
    }

    pub(super) fn accepted(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
        sequence: u64,
    ) -> Result<(), String> {
        let cord = self.remote(endpoint, RemoteCordDirection::Egress)?.cord;
        self.scheduler
            .remote_egress_accept(endpoint, cord, sequence)
            .map_err(debug)
    }

    pub(super) fn delivered(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
        sequence: u64,
    ) -> Result<(), String> {
        let cord = self.remote(endpoint, RemoteCordDirection::Egress)?.cord;
        self.scheduler
            .remote_egress_delivered(endpoint, cord, sequence)
            .map_err(debug)
    }

    pub(super) fn close_ingress(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
    ) -> Result<(), String> {
        let cord = self.remote(endpoint, RemoteCordDirection::Ingress)?.cord;
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(debug)
    }

    pub(super) fn terminal(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
    ) -> Result<bool, String> {
        let cord = self.remote(endpoint, RemoteCordDirection::Egress)?.cord;
        self.scheduler
            .remote_egress_terminal(endpoint, cord)
            .map_err(debug)
    }

    pub(super) fn cancel(&mut self) -> Result<(), String> {
        self.scheduler.cancel().map_err(debug)
    }

    fn endpoint_for(&self, binding: &SessionBinding) -> Option<conduit_kernel::RemoteEndpointId> {
        self.remotes
            .iter()
            .find(|remote| {
                remote.connection_id == binding.connection_id
                    && remote.source_fragment_id == binding.source_fragment_id
                    && remote.sink_fragment_id == binding.sink_fragment_id
                    && remote.value_kind == binding.value_kind
            })
            .map(|remote| remote.endpoint)
    }
}

fn debug(error: impl core::fmt::Debug) -> String {
    format!("remote kernel: {error:?}")
}
