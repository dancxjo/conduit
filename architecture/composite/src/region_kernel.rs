//! Prepared pure region instances reuse the existing composite child kernel.
mod memory;
pub use memory::RegionMemoryReceipt;

use crate::{
    child::ChildKernel, prelude::*, ChildExecutionError, ChildTransportError,
    KernelCompositeSignStorage, KernelOperationRegistry,
};
use conduit_core::{
    derive_planned_transformation_eligibility, ExecutionRegionId, FragmentId, PlanId, PortTemporal,
    WorkTransformation,
};
use conduit_kernel::{
    scheduler::{RemoteIngressOutcome, SchedulerError, SchedulerStatus},
    CordId, NodeId, RemoteEndpointId,
};
use conduit_plan_lowering::execution_regions::PreparedExecutionRegions;

/// An owned kernel instance that can move to an explicitly admitted worker.
/// The containing Host must reserve lane/memory and complete worker footprint
/// admission before calling `step`. Preparation itself starts no thread or Play.
pub struct PreparedRegionKernel {
    kernel: ChildKernel,
    plan_id: PlanId,
    fragment_id: FragmentId,
    region_id: ExecutionRegionId,
    worker_storage: conduit_core::ExecutionWorkerStorage,
    scratch: Box<[u8]>,
    memory: RegionMemoryReceipt,
    maximum_transfer_bytes: usize,
    original_nodes: Vec<NodeId>,
    original_cords: Vec<CordId>,
}
impl PreparedRegionKernel {
    pub fn prepare(
        prepared: &PreparedExecutionRegions<'_>,
        region_index: usize,
        registry: &KernelOperationRegistry,
        signs: KernelCompositeSignStorage,
    ) -> Result<Self, String> {
        let region = prepared
            .regions()
            .get(region_index)
            .ok_or("unknown execution region")?
            .admitted();
        if region.scheduling.worker_storage().is_none()
            || region.preemption_required
            || region.isolation_required
        {
            return Err(
                "region worker requires its bounded memory profile without preemption or isolation"
                    .into(),
            );
        }
        let storage = region
            .scheduling
            .worker_storage()
            .ok_or("missing worker storage")?;
        let stack =
            usize::try_from(storage.stack_bytes).map_err(|_| "worker stack capacity overflow")?;
        let scratch = usize::try_from(storage.scratch_bytes)
            .map_err(|_| "worker scratch capacity overflow")?;
        let capacity = usize::try_from(storage.binding.units)
            .map_err(|_| "worker memory capacity overflow")?;
        let floor = ChildKernel::fixed_storage_floor_bytes()
            .checked_add(stack)
            .and_then(|bytes| bytes.checked_add(scratch))
            .ok_or("worker storage floor overflow")?;
        if floor > capacity {
            return Err("worker memory cannot hold the fixed kernel, stack and scratch".into());
        }
        let tables = prepared
            .prepare_kernel_tables(region_index)
            .map_err(|e| format!("region tables: {e:?}"))?;
        let mut placements = Vec::with_capacity(tables.original_nodes().len());
        for node in tables.original_nodes() {
            let id = prepared
                .lowered()
                .identity
                .placement_for_node(*node)
                .ok_or("region placement identity missing")?;
            let placement = prepared
                .fragment()
                .placements
                .iter()
                .find(|p| &p.placement_id == id)
                .ok_or("region placement disappeared")?;
            derive_planned_transformation_eligibility(placement)
                .and_then(|facts| facts.require(WorkTransformation::Move))
                .map_err(|e| format!("region movement is ineligible: {e:?}"))?;
            if placement.base.is_some()
                || placement
                    .inputs
                    .iter()
                    .chain(&placement.outputs)
                    .any(|p| p.temporal != PortTemporal::Value)
            {
                return Err("worker region requires pure Value ports without Base effects".into());
            }
            placements.push(placement);
        }
        let kernel = ChildKernel::prepare_region(&tables, &placements, registry, signs)?;
        let mut instance = Self {
            kernel,
            plan_id: prepared.plan().plan_id.clone(),
            fragment_id: prepared.fragment().fragment_id.clone(),
            region_id: region.region_id.clone(),
            worker_storage: storage.clone(),
            scratch: vec![0u8; scratch].into_boxed_slice(),
            memory: RegionMemoryReceipt::default(),
            maximum_transfer_bytes: tables
                .cords()
                .iter()
                .filter(|cord| {
                    matches!(cord.source, conduit_kernel::CordEndpoint::Remote(_))
                        || matches!(cord.sink, conduit_kernel::CordEndpoint::Remote(_))
                })
                .map(|cord| cord.maximum_value_bytes as usize)
                .max()
                .unwrap_or(0),
            original_nodes: tables.original_nodes().to_vec(),
            original_cords: tables.original_cords().to_vec(),
        };
        instance.memory = memory::admit(&instance)?;
        Ok(instance)
    }
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    pub fn fragment_id(&self) -> &FragmentId {
        &self.fragment_id
    }
    pub fn region_id(&self) -> &ExecutionRegionId {
        &self.region_id
    }
    pub fn worker_storage(&self) -> &conduit_core::ExecutionWorkerStorage {
        &self.worker_storage
    }
    pub fn maximum_transfer_bytes(&self) -> usize {
        self.maximum_transfer_bytes
    }
    pub fn memory_receipt(&self) -> RegionMemoryReceipt {
        self.memory
    }
    pub fn scratch_mut(&mut self) -> &mut [u8] {
        &mut self.scratch
    }
    pub fn original_cords(&self) -> &[CordId] {
        &self.original_cords
    }
    pub fn original_nodes(&self) -> &[NodeId] {
        &self.original_nodes
    }
    pub fn step(&mut self) -> Result<SchedulerStatus, ChildExecutionError> {
        self.kernel.step()
    }
    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        self.kernel.cancel()
    }
    fn local_cord(&self, original: CordId) -> Result<CordId, ChildTransportError> {
        self.original_cords
            .iter()
            .position(|c| *c == original)
            .and_then(|i| u16::try_from(i).ok())
            .map(CordId)
            .ok_or(ChildTransportError::Scheduler(
                SchedulerError::InvalidRemoteCordAccess,
            ))
    }
    pub fn offer_into(
        &mut self,
        original: CordId,
        buffer: &mut Vec<u8>,
    ) -> Result<Option<u64>, ChildTransportError> {
        let local = self.local_cord(original)?;
        self.kernel
            .remote_offer_into(RemoteEndpointId(original.0), local, buffer)
    }
    pub fn admit(
        &mut self,
        original: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, ChildTransportError> {
        let local = self.local_cord(original)?;
        self.kernel
            .remote_admit(RemoteEndpointId(original.0), local, sequence, bytes)
    }
    pub fn delivered(
        &mut self,
        original: CordId,
        sequence: u64,
    ) -> Result<(), ChildTransportError> {
        let local = self.local_cord(original)?;
        self.kernel
            .remote_delivered(RemoteEndpointId(original.0), local, sequence)
    }
    pub fn terminal(
        &self,
        original: CordId,
    ) -> Result<Option<conduit_kernel::RemoteTerminalDisposition>, ChildTransportError> {
        let local = self.local_cord(original)?;
        self.kernel
            .remote_terminal_disposition(RemoteEndpointId(original.0), local)
    }
    pub fn abnormal_terminal(
        &self,
        original: CordId,
    ) -> Result<Option<conduit_kernel::CanonicalValue>, ChildTransportError> {
        let local = self.local_cord(original)?;
        self.kernel
            .remote_abnormal_terminal(RemoteEndpointId(original.0), local)
    }
    pub fn close(&mut self, original: CordId) -> Result<(), ChildTransportError> {
        let local = self.local_cord(original)?;
        self.kernel
            .remote_close(RemoteEndpointId(original.0), local)
    }
    pub fn close_abnormal(
        &mut self,
        original: CordId,
        value: conduit_kernel::CanonicalValue,
    ) -> Result<(), ChildTransportError> {
        let local = self.local_cord(original)?;
        self.kernel
            .remote_close_abnormal(RemoteEndpointId(original.0), local, value)
    }
    /// Evidence extraction allocates; use outside the bounded execution loop.
    /// Remote lifecycle evidence extraction also belongs outside Play.
    pub fn remote_signs(&self) -> Vec<(u32, conduit_kernel::RemoteLifecycleIdentity)> {
        self.kernel.remote_signs()
    }
    pub fn signs(&self) -> Vec<conduit_kernel::KernelEvent> {
        self.kernel.signs()
    }
}
