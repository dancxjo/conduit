mod host_dispatch;
#[cfg(test)]
use host_dispatch::{dispatch_matches, outstanding_host_call_index};
mod preparation;
use crate::child::{BoundaryEndpoint, ChildKernel, ChildTerminalError, ChildTransportError};
use crate::{KernelCompositeDefinition, KernelOperationRegistry};
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
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositePreparation {
    plan: Plan,
    children: BTreeMap<HostId, LoweredPlanFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeError {
    Empty,
    DuplicateChild(HostId),
    Lowering {
        child: HostId,
        error: LoweringError,
    },
    InvalidBoundary(String),
    InvalidHostCallToken,
    HostCallDispatchMismatch,
    HostCallOutputExceeded,
    StaleHostCallChild,
    ChildRefused {
        child: HostId,
        reason: String,
    },
    Execution {
        child: HostId,
        reason: String,
    },
    UnknownFront,
    StaleChild(HostId),
    MalformedBoundary(PortId),
    InvalidLifecycle,
    CancellationRefused {
        failed_children: usize,
    },
    InternalTransport {
        link: usize,
        reason: ChildTransportError,
    },
    Terminal(ChildTerminalError),
}

impl core::fmt::Display for KernelCompositeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for KernelCompositeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FaceRoute {
    child: HostId,
    direction: PortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InternalLink {
    connection_id: ConnectionId,
    source_child: HostId,
    source_endpoint: RemoteEndpointId,
    source_cord: conduit_kernel::CordId,
    sink_child: HostId,
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
    cancellation_failures: Vec<(HostId, String)>,
}

impl KernelCompositeHost {
    pub(crate) fn has_exact_definition(&self, definition: &KernelCompositeDefinition) -> bool {
        &self.definition == definition
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
            .ok_or_else(|| KernelCompositeError::StaleChild(child.clone()))?
            .admit_boundary(port_id, sequence, value)
            .map_err(|_| KernelCompositeError::MalformedBoundary(port_id.clone()))
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
            .ok_or_else(|| KernelCompositeError::StaleChild(child.clone()))?
            .close_boundary(port_id)
            .map_err(|reason| execution(child, reason))
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
            .ok_or_else(|| KernelCompositeError::StaleChild(child.clone()))?
            .close_boundary_abnormal(port_id, terminal)
            .map_err(|reason| execution(child, reason))
    }

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
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .boundary_output(port_id)
            .map_err(|reason| execution(&route.child, reason))
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
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .boundary_output_into(port_id, output)
            .map_err(|reason| execution(&route.child, reason))
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
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .deliver_boundary(port_id, sequence)
            .map_err(|reason| execution(&route.child, reason))
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
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
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
        for (child, kernel) in &mut self.children {
            kernel.step().map_err(|reason| execution(child, reason))?;
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
        for (child, kernel) in &mut self.children {
            if let Err(reason) = kernel.cancel() {
                self.cancellation_failures.push((child.clone(), reason));
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

    pub fn cancellation_failures(&self) -> &[(HostId, String)] {
        &self.cancellation_failures
    }

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
                .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                .remote_offer_into(link.source_endpoint, link.source_cord, &mut link.transfer)
                .map_err(|reason| KernelCompositeError::InternalTransport {
                    link: link_index,
                    reason,
                })?;
            if let Some(sequence) = offer {
                let accepted = children
                    .get_mut(&link.sink_child)
                    .ok_or_else(|| KernelCompositeError::StaleChild(link.sink_child.clone()))?
                    .remote_admit(link.sink_endpoint, link.sink_cord, sequence, &link.transfer)
                    .map_err(|reason| KernelCompositeError::InternalTransport {
                        link: link_index,
                        reason,
                    })?;
                if matches!(accepted, RemoteIngressOutcome::Accepted { .. }) {
                    children
                        .get_mut(&link.source_child)
                        .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                        .remote_delivered(link.source_endpoint, link.source_cord, sequence)
                        .map_err(|reason| KernelCompositeError::InternalTransport {
                            link: link_index,
                            reason,
                        })?;
                }
            } else {
                let terminal = children
                    .get(&link.source_child)
                    .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                    .remote_terminal_disposition(link.source_endpoint, link.source_cord)
                    .map_err(|reason| KernelCompositeError::InternalTransport {
                        link: link_index,
                        reason,
                    })?;
                match terminal {
                    Some(RemoteTerminalDisposition::NormalClose) => {
                        children
                            .get_mut(&link.sink_child)
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.sink_child.clone())
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
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.source_child.clone())
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
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.sink_child.clone())
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

fn execution(child: &HostId, reason: String) -> KernelCompositeError {
    KernelCompositeError::Execution {
        child: child.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests;
