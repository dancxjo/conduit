//! Generic std-kernel preparation and remote-Cord lifecycle access.
//!
//! This owns no transport. An admitted Line driver moves the exact offered
//! bytes and reports acceptance/delivery through these bounded methods.

use super::{
    kernel_preparation::KernelTables, preparation, supports, InstalledScheduler, MAX_CORDS,
    MAX_NODES, MAX_QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS,
};
use crate::remote_cord_sessions::RemoteCordSessions;
use conduit_core::{
    bind_active_play, kind_id, HostAdvertisement, HostOperationContractId, PlanFragment,
};
use conduit_kernel::scheduler::{HostOperationRequest, RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, CordId, HostOperationOutcome, HostedSignLog, HostedValueStore,
    RemoteEndpointId,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection,
};
use conduit_wire::SessionMessage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteValueTransfer {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteHostWork {
    pub request: HostOperationRequest,
    pub contract_id: HostOperationContractId,
    pub maximum_output_bytes: u32,
    pub input: Vec<u8>,
}

pub struct InstalledRemoteFragment {
    scheduler: InstalledScheduler,
    lowered: LoweredPlanFragment,
    sessions: RemoteCordSessions,
    text_output_buffer: Vec<u8>,
}

impl InstalledRemoteFragment {
    pub fn prepare(
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        play_sequence: u64,
    ) -> Result<Self, String> {
        if advertisement.host_id != fragment.host_id || advertisement.boot_id != fragment.boot_id {
            return Err("remote fragment preparation requires its exact Host and Boot".into());
        }
        if !supports(fragment) {
            return Err("remote fragment contains an uninstalled std implementation".into());
        }
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| format!("lower remote std fragment: {error:?}"))?;
        validate_profile(&lowered)?;
        let sessions = RemoteCordSessions::prepare(fragment, &lowered)?;

        let mut value_items = 0_u16;
        let mut value_bytes = 0_u32;
        let mut maximum_value_bytes = super::TICK_ENCODED_LEN;
        let mut sign_items = 32_u16;
        for placement in &fragment.placements {
            let budget = preparation::operation_budget(placement)?;
            value_items = value_items
                .checked_add(budget.value_items)
                .ok_or_else(|| "remote fragment value item budget overflow".to_string())?;
            value_bytes = value_bytes
                .checked_add(budget.value_bytes)
                .ok_or_else(|| "remote fragment value byte budget overflow".to_string())?;
            sign_items = sign_items
                .checked_add(budget.sign_items)
                .ok_or_else(|| "remote fragment Sign item budget overflow".to_string())?;
            maximum_value_bytes = maximum_value_bytes.max(budget.maximum_value_bytes);
        }
        for endpoint in &lowered.remote_endpoints {
            if endpoint.direction == RemoteCordDirection::Ingress {
                let cord = exact_cord(&lowered, endpoint.cord)?;
                value_items = value_items
                    .checked_add(cord.item_capacity)
                    .ok_or_else(|| "remote ingress value item budget overflow".to_string())?;
                value_bytes = value_bytes
                    .checked_add(cord.byte_capacity)
                    .ok_or_else(|| "remote ingress value byte budget overflow".to_string())?;
                maximum_value_bytes = maximum_value_bytes.max(cord.byte_capacity);
            }
        }
        let mut values =
            HostedValueStore::new(value_items.max(1), maximum_value_bytes, value_bytes.max(1))
                .map_err(|error| format!("remote fragment value store: {error:?}"))?;
        let play = bind_active_play(
            &fragment.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let drivers =
            preparation::prepare_operations(fragment, &lowered, &mut values, &play, None)?;
        let tables = KernelTables::prepare(&[&lowered])?;
        let sign_bytes = u32::from(sign_items)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "remote fragment Sign byte budget overflow".to_string())?;
        let remote_sign_items = remote_sign_capacity(&lowered)?;
        let remote_sign_bytes = conduit_kernel::remote_sign_storage_bytes(remote_sign_items)
            .ok_or_else(|| "remote fragment remote Sign byte budget overflow".to_string())?;
        let signs = HostedSignLog::new_with_remote_storage(
            sign_items,
            sign_bytes,
            remote_sign_items,
            remote_sign_bytes,
        )
        .map_err(|error| format!("remote fragment Sign store: {error:?}"))?;
        let scheduler = tables.install(drivers, values, signs)?;
        Ok(Self {
            scheduler,
            lowered,
            sessions,
            text_output_buffer: Vec::with_capacity(super::contract::MAX_TEXT_BYTES as usize),
        })
    }

    pub fn sessions(&self) -> &RemoteCordSessions {
        &self.sessions
    }
    pub fn sessions_mut(&mut self) -> &mut RemoteCordSessions {
        &mut self.sessions
    }
    pub fn step(&mut self) -> Result<SchedulerStatus, String> {
        self.scheduler
            .step()
            .map_err(|error| format!("step remote std fragment: {error:?}"))
    }
    pub fn next_host_request(&mut self) -> Option<HostOperationRequest> {
        self.scheduler.next_host_request()
    }
    pub fn describe_host_request(
        &self,
        request: HostOperationRequest,
    ) -> Result<RemoteHostWork, String> {
        let operation = self
            .lowered
            .host_operations
            .iter()
            .find(|operation| {
                operation.node == request.node && operation.operation == request.operation
            })
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote std host input: {error:?}"))?
            .to_vec();
        Ok(RemoteHostWork {
            request,
            contract_id: operation.contract_id.clone(),
            maximum_output_bytes: operation.binding.maximum_output_bytes,
            input,
        })
    }
    pub fn complete_host_operation(
        &mut self,
        request: HostOperationRequest,
        outcome: HostOperationOutcome,
    ) -> Result<(), String> {
        self.scheduler
            .complete_host_operation(request.node, request.request, outcome)
            .map_err(|error| format!("complete remote std host operation: {error:?}"))
    }
    pub fn complete_pure_text_host_operation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<bool, String> {
        let operation = self
            .lowered
            .host_operations
            .iter()
            .find(|operation| {
                operation.node == request.node && operation.operation == request.operation
            })
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        if operation.contract_id.as_str() != conduit_std_offers::TEXT_UPPER_HOST_OPERATION_CONTRACT
            || operation.target_kind.as_ref()
                != Some(&kind_id(
                    conduit_std_offers::TEXT_UPPER_HOST_OPERATION_TARGET,
                ))
        {
            return Ok(false);
        }
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote std text input: {error:?}"))?;
        super::text_operations::uppercase_utf8(input, &mut self.text_output_buffer)?;
        let value = self
            .scheduler
            .store_host_value(&self.text_output_buffer)
            .map_err(|error| format!("store remote uppercase text output: {error:?}"))?;
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                super::text_operations::completed_with_output(value),
            )
            .map_err(|error| format!("complete remote text/upper host operation: {error:?}"))?;
        Ok(true)
    }
    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<BoundedValueRef, String> {
        let value = self
            .scheduler
            .store_host_value(bytes)
            .map_err(|error| format!("store remote std host value: {error:?}"))?;
        let byte_len = u32::try_from(bytes.len())
            .map_err(|_| "remote std host value length overflow".to_string())?;
        BoundedValueRef::new(value, byte_len)
            .map_err(|error| format!("bound remote std host value: {error:?}"))
    }
    pub fn next_egress(
        &mut self,
        endpoint: RemoteEndpointId,
    ) -> Result<Option<RemoteValueTransfer>, String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Egress)?;
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(endpoint, cord)
            .map_err(|error| format!("offer remote std value: {error:?}"))?
        else {
            return Ok(None);
        };
        let bytes = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read remote std value: {error:?}"))?
            .to_vec();
        Ok(Some(RemoteValueTransfer {
            endpoint,
            cord,
            sequence: offer.sequence,
            bytes,
        }))
    }
    pub fn accept_egress(&mut self, transfer: &RemoteValueTransfer) -> Result<(), String> {
        self.scheduler
            .remote_egress_accept(transfer.endpoint, transfer.cord, transfer.sequence)
            .map_err(|error| format!("accept remote std value: {error:?}"))
    }
    pub fn deliver_egress(&mut self, transfer: &RemoteValueTransfer) -> Result<(), String> {
        self.scheduler
            .remote_egress_delivered(transfer.endpoint, transfer.cord, transfer.sequence)
            .map_err(|error| format!("deliver remote std value: {error:?}"))
    }
    pub fn admit_ingress(
        &mut self,
        endpoint: RemoteEndpointId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Ingress)?;
        self.scheduler
            .admit_remote_input(endpoint, cord, sequence, bytes)
            .map_err(|error| format!("admit remote std value: {error:?}"))
    }
    pub fn close_ingress(&mut self, endpoint: RemoteEndpointId) -> Result<(), String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Ingress)?;
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(|error| format!("close remote std input: {error:?}"))
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        self.scheduler
            .cancel()
            .map_err(|error| format!("cancel remote std fragment: {error:?}"))
    }

    /// Records an exact admitted Line failure before cancelling kernel work.
    /// A malformed zero code leaves both the session and kernel unchanged.
    pub fn fail_remote_line(
        &mut self,
        endpoint: RemoteEndpointId,
        code: u16,
    ) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(endpoint)
            .ok_or_else(|| "remote Line failure names no admitted endpoint".to_string())?;
        let binding = session.binding().clone();
        session
            .machine_mut()
            .admit_outbound(binding.frame(SessionMessage::Failed { code }))
            .map_err(|error| format!("record remote Line failure: {error:?}"))?;
        self.cancel()
    }
    fn endpoint_cord(
        &self,
        endpoint: RemoteEndpointId,
        direction: RemoteCordDirection,
    ) -> Result<CordId, String> {
        self.lowered
            .remote_endpoints
            .iter()
            .find(|candidate| candidate.endpoint == endpoint && candidate.direction == direction)
            .map(|candidate| candidate.cord)
            .ok_or_else(|| "remote endpoint direction or identity mismatch".into())
    }
}

fn exact_cord(
    lowered: &LoweredPlanFragment,
    cord: CordId,
) -> Result<conduit_kernel::scheduler::CordSpec, String> {
    lowered
        .cords
        .iter()
        .find(|candidate| candidate.spec.cord == cord)
        .map(|candidate| candidate.spec)
        .ok_or_else(|| "remote endpoint has no exact lowered Cord".into())
}

fn validate_profile(lowered: &LoweredPlanFragment) -> Result<(), String> {
    let route_targets = lowered
        .routes
        .iter()
        .map(|route| route.targets.len())
        .sum::<usize>();
    if lowered.nodes.is_empty()
        || lowered.nodes.len() > MAX_NODES
        || lowered.cords.is_empty()
        || lowered.cords.len() > MAX_CORDS
        || lowered.cord_value_slots as usize > MAX_QUEUE_SLOTS
        || lowered.routes.len() > ROUTE_SLOTS
        || route_targets > ROUTE_TARGETS
    {
        return Err("remote fragment exceeds the installed std kernel profile".into());
    }
    Ok(())
}

fn remote_sign_capacity(lowered: &LoweredPlanFragment) -> Result<u16, String> {
    lowered
        .remote_endpoints
        .iter()
        .try_fold(0_u16, |total, endpoint| {
            let items = exact_cord(lowered, endpoint.cord)?.item_capacity;
            let events = match endpoint.direction {
                RemoteCordDirection::Ingress => items.checked_add(1),
                RemoteCordDirection::Egress => {
                    items.checked_mul(3).and_then(|count| count.checked_add(1))
                }
            }
            .ok_or_else(|| "remote lifecycle Sign capacity overflow".to_string())?;
            total
                .checked_add(events)
                .ok_or_else(|| "remote lifecycle Sign capacity overflow".to_string())
        })
}
