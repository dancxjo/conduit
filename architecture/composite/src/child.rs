//! Bounded child execution and exact composite boundary transport.
mod preparation;
use crate::BoxedKernelBack;
use conduit_core::{PortDirection, PortId as SemanticPortId, ValuePayload};
use conduit_kernel::scheduler::{
    FixedScheduler, HostCallRequest, RemoteIngressOutcome, SchedulerError, SchedulerStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildTransportError {
    Scheduler(SchedulerError),
    TransferBufferTooSmall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildTerminalError {
    UnknownFront,
    MissingAbnormalKind,
    MissingAbnormalValue,
    BufferContractMismatch,
    Scheduler(SchedulerError),
}
use conduit_kernel::{
    CanonicalValue, CordId, HostedSignLog, HostedValueStore, KernelEvent, NodeId, RemoteEndpointId,
    RemoteTerminalDisposition, ValueStorage,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::collections::BTreeMap;

pub(crate) const MAX_NODES: usize = 16;
pub(crate) const MAX_CORDS: usize = 32;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const MAX_QUEUE_SLOTS: usize = 256;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = MAX_CORDS;
const HOST_CALLS_PER_NODE: u16 = 8;
const HOST_BINDING_SLOTS: usize = MAX_NODES * HOST_CALLS_PER_NODE as usize;
const PENDING_REQUESTS: usize = MAX_NODES;

type ChildScheduler = FixedScheduler<
    BoxedKernelBack,
    HostedValueStore,
    HostedSignLog,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    MAX_QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundaryEndpoint {
    pub external_port_id: SemanticPortId,
    pub internal_port_id: SemanticPortId,
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub direction: PortDirection,
    pub value_kind: conduit_core::KindId,
    pub abnormal_kind: Option<conduit_core::KindId>,
    pub item_capacity: u16,
    pub byte_capacity: u32,
    pub already_lowered: bool,
}

pub(crate) struct ChildKernel {
    scheduler: ChildScheduler,
    boundaries: BTreeMap<SemanticPortId, BoundaryEndpoint>,
    status: SchedulerStatus,
}

impl ChildKernel {
    pub(crate) fn step(&mut self) -> Result<SchedulerStatus, String> {
        self.status = self.scheduler.step().map_err(debug)?;
        Ok(self.status)
    }

    pub(crate) fn status(&self) -> SchedulerStatus {
        self.status
    }

    pub(crate) fn next_host_request(&mut self) -> Option<HostCallRequest> {
        self.scheduler.next_host_request()
    }

    pub(crate) fn complete_host_call(
        &mut self,
        node: NodeId,
        request: conduit_kernel::RequestId,
        outcome: conduit_kernel::HostCallOutcome,
    ) -> Result<(), String> {
        self.scheduler
            .complete_host_call(node, request, outcome)
            .map_err(debug)
    }

    pub(crate) fn host_value(&self, value: conduit_kernel::ValueRef) -> Result<&[u8], String> {
        self.scheduler.host_value(value).map_err(debug)
    }

    pub(crate) fn store_host_value(
        &mut self,
        bytes: &[u8],
    ) -> Result<conduit_kernel::ValueRef, String> {
        self.scheduler.store_host_value(bytes).map_err(debug)
    }

    pub(crate) fn admit_boundary(
        &mut self,
        port_id: &SemanticPortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<RemoteIngressOutcome, String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Input)
            .ok_or_else(|| "unknown composite input front".to_string())?;
        if boundary.value_kind != value.value_kind {
            return Err("composite input value kind differs from its exact front".into());
        }
        self.scheduler
            .admit_remote_input(boundary.endpoint, boundary.cord, sequence, &value.encoded)
            .map_err(debug)
    }

    pub(crate) fn close_boundary(&mut self, port_id: &SemanticPortId) -> Result<(), String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Input)
            .ok_or_else(|| "unknown composite input front".to_string())?;
        self.scheduler
            .close_remote_input(boundary.endpoint, boundary.cord)
            .map_err(debug)
    }

    pub(crate) fn close_boundary_abnormal(
        &mut self,
        port_id: &SemanticPortId,
        terminal: &ValuePayload,
    ) -> Result<(), String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Input)
            .ok_or_else(|| "unknown composite input front".to_string())?;
        if boundary.abnormal_kind.as_ref() != Some(&terminal.value_kind) {
            return Err("composite abnormal input kind differs from its exact front".into());
        }
        let terminal = CanonicalValue::new(&terminal.encoded).map_err(debug)?;
        self.scheduler
            .close_remote_input_abnormal(boundary.endpoint, boundary.cord, terminal)
            .map_err(debug)
    }

    pub(crate) fn boundary_output(
        &mut self,
        port_id: &SemanticPortId,
    ) -> Result<Option<(u64, ValuePayload)>, String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Output)
            .ok_or_else(|| "unknown composite output front".to_string())?;
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(boundary.endpoint, boundary.cord)
            .map_err(debug)?
        else {
            return Ok(None);
        };
        let bytes = self
            .scheduler
            .values()
            .get(offer.value)
            .map_err(debug)?
            .to_vec();
        Ok(Some((
            offer.sequence,
            ValuePayload {
                value_kind: boundary.value_kind.clone(),
                encoded: bytes,
            },
        )))
    }

    pub(crate) fn boundary_output_into(
        &mut self,
        port_id: &SemanticPortId,
        output: &mut ValuePayload,
    ) -> Result<Option<u64>, String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Output)
            .ok_or_else(|| "unknown composite output front".to_string())?;
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(boundary.endpoint, boundary.cord)
            .map_err(debug)?
        else {
            return Ok(None);
        };
        let bytes = self.scheduler.values().get(offer.value).map_err(debug)?;
        if output.value_kind != boundary.value_kind || bytes.len() > output.encoded.capacity() {
            return Err("prepared composite output buffer differs from its exact front".into());
        }
        output.encoded.clear();
        output.encoded.extend_from_slice(bytes);
        Ok(Some(offer.sequence))
    }

    pub(crate) fn deliver_boundary(
        &mut self,
        port_id: &SemanticPortId,
        sequence: u64,
    ) -> Result<(), String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Output)
            .ok_or_else(|| "unknown composite output front".to_string())?;
        self.scheduler
            .remote_egress_accept(boundary.endpoint, boundary.cord, sequence)
            .and_then(|()| {
                self.scheduler
                    .remote_egress_delivered(boundary.endpoint, boundary.cord, sequence)
            })
            .map_err(debug)
    }

    pub(crate) fn boundary_terminal_into(
        &self,
        port_id: &SemanticPortId,
        abnormal: &mut ValuePayload,
    ) -> Result<Option<RemoteTerminalDisposition>, ChildTerminalError> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Output)
            .ok_or(ChildTerminalError::UnknownFront)?;
        match self
            .scheduler
            .remote_egress_terminal_disposition(boundary.endpoint, boundary.cord)
            .map_err(ChildTerminalError::Scheduler)?
        {
            None => Ok(None),
            Some(RemoteTerminalDisposition::NormalClose) => {
                Ok(Some(RemoteTerminalDisposition::NormalClose))
            }
            Some(RemoteTerminalDisposition::Abnormal) => {
                let kind = boundary
                    .abnormal_kind
                    .as_ref()
                    .ok_or(ChildTerminalError::MissingAbnormalKind)?;
                let terminal = self
                    .scheduler
                    .remote_egress_abnormal_terminal(boundary.endpoint, boundary.cord)
                    .map_err(ChildTerminalError::Scheduler)?
                    .ok_or(ChildTerminalError::MissingAbnormalValue)?;
                if &abnormal.value_kind != kind
                    || terminal.as_slice().len() > abnormal.encoded.capacity()
                {
                    return Err(ChildTerminalError::BufferContractMismatch);
                }
                abnormal.encoded.clear();
                abnormal.encoded.extend_from_slice(terminal.as_slice());
                Ok(Some(RemoteTerminalDisposition::Abnormal))
            }
        }
    }

    pub(crate) fn cancel(&mut self) -> Result<(), String> {
        self.scheduler.cancel().map_err(debug)?;
        self.status = SchedulerStatus::Cancelled;
        Ok(())
    }

    pub(crate) fn signs(&self) -> Vec<KernelEvent> {
        self.scheduler.signs().events().collect()
    }

    pub(crate) fn remote_offer_into(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        output: &mut Vec<u8>,
    ) -> Result<Option<u64>, ChildTransportError> {
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(endpoint, cord)
            .map_err(ChildTransportError::Scheduler)?
        else {
            return Ok(None);
        };
        let bytes = self
            .scheduler
            .values()
            .get(offer.value)
            .map_err(|error| ChildTransportError::Scheduler(error.into()))?;
        if bytes.len() > output.capacity() {
            return Err(ChildTransportError::TransferBufferTooSmall);
        }
        output.clear();
        output.extend_from_slice(bytes);
        Ok(Some(offer.sequence))
    }

    pub(crate) fn remote_delivered(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), ChildTransportError> {
        self.scheduler
            .remote_egress_accept(endpoint, cord, sequence)
            .and_then(|()| {
                self.scheduler
                    .remote_egress_delivered(endpoint, cord, sequence)
            })
            .map_err(ChildTransportError::Scheduler)
    }

    pub(crate) fn remote_admit(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, ChildTransportError> {
        self.scheduler
            .admit_remote_input(endpoint, cord, sequence, bytes)
            .map_err(ChildTransportError::Scheduler)
    }

    pub(crate) fn remote_terminal_disposition(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<RemoteTerminalDisposition>, ChildTransportError> {
        self.scheduler
            .remote_egress_terminal_disposition(endpoint, cord)
            .map_err(ChildTransportError::Scheduler)
    }

    pub(crate) fn remote_abnormal_terminal(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<CanonicalValue>, ChildTransportError> {
        self.scheduler
            .remote_egress_abnormal_terminal(endpoint, cord)
            .map_err(ChildTransportError::Scheduler)
    }

    pub(crate) fn remote_close(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<(), ChildTransportError> {
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(ChildTransportError::Scheduler)
    }

    pub(crate) fn remote_close_abnormal(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        terminal: CanonicalValue,
    ) -> Result<(), ChildTransportError> {
        self.scheduler
            .close_remote_input_abnormal(endpoint, cord, terminal)
            .map_err(ChildTransportError::Scheduler)
    }
}

fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
