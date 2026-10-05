use crate::prelude::*;
mod host_dispatch;
#[cfg(test)]
use host_dispatch::{dispatch_matches, outstanding_host_call_index};
mod preparation;
mod sign_storage;
use crate::child::{
    BoundaryEndpoint, ChildExecutionError, ChildKernel, ChildTerminalError, ChildTransportError,
};
use crate::{KernelCompositeDefinition, KernelOperationRegistry};
use alloc::collections::BTreeMap;
use conduit_core::{
    ActivePlayId, AuthorityBinding, ConnectionId, HostCallRequirement, HostId, Plan, PortDirection,
    PortId, PreparationHostIdentity, ResourceBinding, ValuePayload,
};
use conduit_kernel::scheduler::{HostCallRequest, RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::RemoteTerminalDisposition;
use conduit_kernel::{HostCallId, HostCallOutcome, KernelEvent, NodeId, RemoteEndpointId};
use conduit_plan_lowering::lowering::{LoweredPlanFragment, LoweringError};
#[cfg(test)]
use preparation::host_call_obligation_identity;
pub use sign_storage::KernelCompositeSignStorage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositePreparation {
    plan: Plan,
    children: BTreeMap<HostId, LoweredPlanFragment>,
}

mod error;
pub use error::KernelCompositeError;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FaceRoute {
    child: HostId,
    child_index: usize,
    direction: PortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InternalLink {
    connection_id: ConnectionId,
    source_child: HostId,
    source_index: usize,
    source_endpoint: RemoteEndpointId,
    source_cord: conduit_kernel::CordId,
    sink_child: HostId,
    sink_index: usize,
    sink_endpoint: RemoteEndpointId,
    sink_cord: conduit_kernel::CordId,
    closed: bool,
    transfer: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelCompositeHostRequest {
    pub dispatch_token: u64,
}

/// Borrowed adapter-facing fields for one live Host Call dispatch.
#[derive(Debug, Clone, Copy)]
pub struct KernelCompositeHostRequestView<'a> {
    pub child: &'a HostId,
    pub request: &'a HostCallRequest,
    /// Commitment to the selected fragment, placement, call and Host Call
    /// contract. It remains owned by the prepared outstanding slot.
    pub obligation_identity: &'a [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositeHostCallObligation {
    pub host: PreparationHostIdentity,
    pub requirement: HostCallRequirement,
    pub resources: Vec<ResourceBinding>,
    pub authorities: Vec<AuthorityBinding>,
}

/// A dispatch admitted against the exact selected host identity, resources and grants.
/// Its fields are private so an adapter cannot manufacture admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedKernelCompositeHostRequest {
    dispatch_token: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutstandingHostCall {
    token: u64,
    child_index: usize,
    request: HostCallRequest,
    obligation_identity: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeStatus {
    Active,
    Complete,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeTerminal {
    Normal,
    Abnormal,
}

pub struct KernelCompositeHost {
    definition: KernelCompositeDefinition,
    children: BTreeMap<HostId, ChildKernel>,
    fronts: BTreeMap<PortId, FaceRoute>,
    links: Vec<InternalLink>,
    active_plays: BTreeMap<HostId, ActivePlayId>,
    started: bool,
    cancelled: bool,
    host_call_obligations:
        BTreeMap<(HostId, NodeId, HostCallId), ([u8; 32], KernelCompositeHostCallObligation)>,
    outstanding_host_calls: Vec<Option<OutstandingHostCall>>,
    next_dispatch_token: u64,
    cancellation_failures: Vec<(usize, conduit_kernel::scheduler::SchedulerError)>,
}

impl KernelCompositeHost {
    pub(crate) fn has_exact_definition(&self, definition: &KernelCompositeDefinition) -> bool {
        &self.definition == definition
    }

    /// Resolve the prepared child identity retained by an execution refusal.
    pub fn child_identity(&self, child_index: usize) -> Option<&HostId> {
        self.children.keys().nth(child_index)
    }

    pub fn definition(&self) -> &KernelCompositeDefinition {
        &self.definition
    }

    pub fn internal_transfer_capacities(&self) -> (usize, usize) {
        (
            self.links.capacity(),
            self.links.iter().map(|link| link.transfer.capacity()).sum(),
        )
    }

    pub fn start(&mut self) -> Result<&BTreeMap<HostId, ActivePlayId>, KernelCompositeError> {
        if self.started || self.cancelled {
            return Err(KernelCompositeError::InvalidLifecycle);
        }
        self.started = true;
        Ok(&self.active_plays)
    }

    pub fn active_plays(&self) -> &BTreeMap<HostId, ActivePlayId> {
        &self.active_plays
    }

    pub fn admit_input(
        &mut self,
        port_id: &PortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<RemoteIngressOutcome, KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Input)
            .ok_or(KernelCompositeError::UnknownFront)?;
        let child = &route.child;
        self.children
            .get_mut(child)
            .ok_or(KernelCompositeError::StaleRuntimeChild {
                child: route.child_index,
            })?
            .admit_boundary(port_id, sequence, value)
            .map_err(|reason| execution(route.child_index, reason))
    }

    pub fn close_input(&mut self, port_id: &PortId) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Input)
            .ok_or(KernelCompositeError::UnknownFront)?;
        let child = &route.child;
        self.children
            .get_mut(child)
            .ok_or(KernelCompositeError::StaleRuntimeChild {
                child: route.child_index,
            })?
            .close_boundary(port_id)
            .map_err(|reason| execution(route.child_index, reason))
    }

    pub fn close_input_abnormal(
        &mut self,
        port_id: &PortId,
        terminal: &ValuePayload,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Input)
            .ok_or(KernelCompositeError::UnknownFront)?;
        let child = &route.child;
        self.children
            .get_mut(child)
            .ok_or(KernelCompositeError::StaleRuntimeChild {
                child: route.child_index,
            })?
            .close_boundary_abnormal(port_id, terminal)
            .map_err(|reason| execution(route.child_index, reason))
    }

    /// Allocating presentation convenience; sealed Play uses `output_into`.
    pub fn output(
        &mut self,
        port_id: &PortId,
    ) -> Result<Option<(u64, ValuePayload)>, KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Output)
            .ok_or(KernelCompositeError::UnknownFront)?;
        self.children
            .get_mut(&route.child)
            .ok_or(KernelCompositeError::StaleRuntimeChild {
                child: route.child_index,
            })?
            .boundary_output(port_id)
            .map_err(|reason| execution(route.child_index, reason))
    }

    pub fn output_into(
        &mut self,
        port_id: &PortId,
        output: &mut ValuePayload,
    ) -> Result<Option<u64>, KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Output)
            .ok_or(KernelCompositeError::UnknownFront)?;
        self.children
            .get_mut(&route.child)
            .ok_or(KernelCompositeError::StaleRuntimeChild {
                child: route.child_index,
            })?
            .boundary_output_into(port_id, output)
            .map_err(|reason| execution(route.child_index, reason))
    }

    pub fn complete_output(
        &mut self,
        port_id: &PortId,
        sequence: u64,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Output)
            .ok_or(KernelCompositeError::UnknownFront)?;
        self.children
            .get_mut(&route.child)
            .ok_or(KernelCompositeError::StaleRuntimeChild {
                child: route.child_index,
            })?
            .deliver_boundary(port_id, sequence)
            .map_err(|reason| execution(route.child_index, reason))
    }

    pub fn output_terminal_into(
        &self,
        port_id: &PortId,
        abnormal: &mut ValuePayload,
    ) -> Result<Option<KernelCompositeTerminal>, KernelCompositeError> {
        self.require_started()?;
        let route = self.front(port_id, PortDirection::Output)?;
        self.children
            .get(&route.child)
            .ok_or(KernelCompositeError::StaleRuntimeChild {
                child: route.child_index,
            })?
            .boundary_terminal_into(port_id, abnormal)
            .map(|terminal| {
                terminal.map(|terminal| match terminal {
                    RemoteTerminalDisposition::NormalClose => KernelCompositeTerminal::Normal,
                    RemoteTerminalDisposition::Abnormal => KernelCompositeTerminal::Abnormal,
                })
            })
            .map_err(KernelCompositeError::Terminal)
    }

    pub fn step(&mut self) -> Result<KernelCompositeStatus, KernelCompositeError> {
        if self.cancelled {
            return Ok(KernelCompositeStatus::Cancelled);
        }
        self.require_started()?;
        for (child_index, kernel) in self.children.values_mut().enumerate() {
            kernel
                .step()
                .map_err(|reason| execution(child_index, reason))?;
        }
        self.pump_internal()?;
        if self
            .children
            .values()
            .all(|child| child.status() == SchedulerStatus::Drained)
            && self.links.iter().all(|link| link.closed)
        {
            Ok(KernelCompositeStatus::Complete)
        } else {
            Ok(KernelCompositeStatus::Active)
        }
    }

    pub fn cancel(&mut self) -> Result<(), KernelCompositeError> {
        // Cancellation consumes adapter authority even if a child later
        // reports mechanism trouble while cancelling.
        self.cancelled = true;
        self.outstanding_host_calls.fill(None);
        self.cancellation_failures.clear();
        for (child_index, kernel) in self.children.values_mut().enumerate() {
            if let Err(reason) = kernel.cancel() {
                self.cancellation_failures.push((child_index, reason));
            }
        }
        if self.cancellation_failures.is_empty() {
            Ok(())
        } else {
            Err(KernelCompositeError::CancellationRefused {
                failed_children: self.cancellation_failures.len(),
            })
        }
    }

    /// Finite failures indexed into the same prepared child identity table.
    pub fn cancellation_failures(&self) -> &[(usize, conduit_kernel::scheduler::SchedulerError)] {
        &self.cancellation_failures
    }

    /// Allocating presentation snapshot, outside sealed Play.
    pub fn signs(&self) -> BTreeMap<HostId, Vec<KernelEvent>> {
        self.children
            .iter()
            .map(|(child, kernel)| (child.clone(), kernel.signs()))
            .collect()
    }

    fn front(
        &self,
        port_id: &PortId,
        direction: PortDirection,
    ) -> Result<&FaceRoute, KernelCompositeError> {
        self.fronts
            .get(port_id)
            .filter(|route| route.direction == direction)
            .ok_or(KernelCompositeError::UnknownFront)
    }

    fn require_started(&self) -> Result<(), KernelCompositeError> {
        if self.started && !self.cancelled {
            Ok(())
        } else {
            Err(KernelCompositeError::InvalidLifecycle)
        }
    }

    fn pump_internal(&mut self) -> Result<(), KernelCompositeError> {
        let (children, links) = (&mut self.children, &mut self.links);
        for (link_index, link) in links.iter_mut().enumerate() {
            if link.closed {
                continue;
            }
            let offer = children
                .get_mut(&link.source_child)
                .ok_or(KernelCompositeError::StaleRuntimeChild {
                    child: link.source_index,
                })?
                .remote_offer_into(link.source_endpoint, link.source_cord, &mut link.transfer)
                .map_err(|reason| KernelCompositeError::InternalTransport {
                    link: link_index,
                    reason,
                })?;
            if let Some(sequence) = offer {
                let accepted = children
                    .get_mut(&link.sink_child)
                    .ok_or(KernelCompositeError::StaleRuntimeChild {
                        child: link.sink_index,
                    })?
                    .remote_admit(link.sink_endpoint, link.sink_cord, sequence, &link.transfer)
                    .map_err(|reason| KernelCompositeError::InternalTransport {
                        link: link_index,
                        reason,
                    })?;
                if matches!(accepted, RemoteIngressOutcome::Accepted { .. }) {
                    children
                        .get_mut(&link.source_child)
                        .ok_or(KernelCompositeError::StaleRuntimeChild {
                            child: link.source_index,
                        })?
                        .remote_delivered(link.source_endpoint, link.source_cord, sequence)
                        .map_err(|reason| KernelCompositeError::InternalTransport {
                            link: link_index,
                            reason,
                        })?;
                }
            } else {
                let terminal = children
                    .get(&link.source_child)
                    .ok_or(KernelCompositeError::StaleRuntimeChild {
                        child: link.source_index,
                    })?
                    .remote_terminal_disposition(link.source_endpoint, link.source_cord)
                    .map_err(|reason| KernelCompositeError::InternalTransport {
                        link: link_index,
                        reason,
                    })?;
                match terminal {
                    Some(RemoteTerminalDisposition::NormalClose) => {
                        children
                            .get_mut(&link.sink_child)
                            .ok_or(KernelCompositeError::StaleRuntimeChild {
                                child: link.sink_index,
                            })?
                            .remote_close(link.sink_endpoint, link.sink_cord)
                            .map_err(|reason| KernelCompositeError::InternalTransport {
                                link: link_index,
                                reason,
                            })?;
                        link.closed = true;
                    }
                    Some(RemoteTerminalDisposition::Abnormal) => {
                        let abnormal = children
                            .get(&link.source_child)
                            .ok_or(KernelCompositeError::StaleRuntimeChild {
                                child: link.source_index,
                            })?
                            .remote_abnormal_terminal(link.source_endpoint, link.source_cord)
                            .map_err(|reason| KernelCompositeError::InternalTransport {
                                link: link_index,
                                reason,
                            })?
                            .ok_or(KernelCompositeError::InternalTransport {
                                link: link_index,
                                reason: ChildTransportError::TransferBufferTooSmall,
                            })?;
                        children
                            .get_mut(&link.sink_child)
                            .ok_or(KernelCompositeError::StaleRuntimeChild {
                                child: link.sink_index,
                            })?
                            .remote_close_abnormal(link.sink_endpoint, link.sink_cord, abnormal)
                            .map_err(|reason| KernelCompositeError::InternalTransport {
                                link: link_index,
                                reason,
                            })?;
                        link.closed = true;
                    }
                    None => {}
                }
            }
        }
        Ok(())
    }
}

fn invalid_front(port_id: &PortId, reason: &str) -> KernelCompositeError {
    KernelCompositeError::InvalidBoundary(format!("front '{}': {reason}", port_id.as_str()))
}

fn execution(child: usize, reason: ChildExecutionError) -> KernelCompositeError {
    KernelCompositeError::Execution { child, reason }
}

#[cfg(test)]
mod tests;
