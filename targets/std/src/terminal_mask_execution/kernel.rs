//! Finite storage comes from the exact lowered Plan, allocated before execution.
use super::*;
use conduit_face::{FACE_INTERACTION_KIND, FACE_TEE_KIND, RENDERER_KIND};
use conduit_kernel::scheduler::FixedScheduler;
use conduit_kernel::scheduler::*;
use conduit_kernel::*;
use conduit_kernel::{FixedHostCallBindings, FixedRoutes, HostedSignLog, HostedValueStore};
use conduit_plan_lowering::lowering::LoweredPlanFragment;
type Profile<const N: usize, const C: usize, const R: usize, const H: usize> =
    FixedScheduler<backs::MaskBack, HostedValueStore, HostedSignLog, N, C, PORTS, C, R, C, H, N>;
pub(super) enum Scheduler {
    Ordinary(Box<Profile<3, 6, { 3 * PORTS }, 2>>),
    Parsec(Box<Profile<5, 8, { 5 * PORTS }, 4>>),
}

pub(super) fn prepare(
    fragment: &conduit_core::PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Scheduler, TerminalError> {
    if fragment.placements.len() == 3 {
        Ok(Scheduler::Ordinary(Box::new(prepare_profile::<
            3,
            6,
            { 3 * PORTS },
            2,
        >(fragment, lowered)?)))
    } else if fragment.placements.len() == 5
        && fragment.placements.iter().any(|p| {
            p.kind_id.as_str() == crate::flow_activation::parsec_instruction_source_offer::KIND
        })
    {
        Ok(Scheduler::Parsec(Box::new(prepare_profile::<
            5,
            8,
            { 5 * PORTS },
            4,
        >(fragment, lowered)?)))
    } else {
        Err(error("unexpected terminal Mask Plan shape"))
    }
}
fn prepare_profile<const N: usize, const C: usize, const R: usize, const H: usize>(
    fragment: &conduit_core::PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Profile<N, C, R, H>, TerminalError> {
    for placement in &fragment.placements {
        if matches!(
            placement.kind_id.as_str(),
            FACE_TEE_KIND | RENDERER_KIND | FACE_INTERACTION_KIND
        ) {
            conduit_core::face_data_label(&placement.configuration).map_err(error)?;
        }
    }
    let read_only = fragment.placements.iter().any(|placement| {
        placement.implementation_id.as_str() == planning::READ_ONLY_INTERACTION_IMPLEMENTATION
    });
    if fragment.placements.len() != N
        || lowered.cords.len() != C
        || lowered.host_calls.len() != if read_only { 1 } else { H }
        || usize::from(lowered.cord_value_slots) != C
    {
        return Err(error("unexpected terminal Mask Plan shape"));
    }
    let mut routes = FixedRoutes::<R, C>::new(PORTS as u16);
    for r in &lowered.routes {
        routes
            .install(r.source_node, r.source_port, r.range, &r.targets)
            .map_err(debug_error)?;
    }
    routes.seal().map_err(debug_error)?;
    let mut bindings = FixedHostCallBindings::<H>::new(1);
    let mut bytes = lowered.cord_value_bytes;
    for call in &lowered.host_calls {
        if call.maximum_in_flight != 1
            || call.binding.maximum_input_bytes > MAX_TERMINAL_VALUE_BYTES
        {
            return Err(error("terminal Host Call exceeds admitted bound"));
        }
        let maximum = match call.contract_id.as_str() {
            PRESENT_CALL => MAX_TERMINAL_VALUE_BYTES,
            INTERACTION_CALL => conduit_face::MAX_FACE_INTERACTION_BYTES as u32,
            crate::flow_activation::parsec_instruction_source_offer::CONTRACT => {
                crate::flow_activation::parsec_instruction_source_offer::MAXIMUM_INSTRUCTION_BYTES
            }
            super::parsec_instruction_effect::CONTRACT => 0,
            _ => return Err(error("unknown terminal Host Call")),
        };
        if call.binding.maximum_output_bytes > maximum {
            return Err(error("terminal Host Call output bound"));
        }
        bytes = bytes
            .checked_add(call.binding.maximum_output_bytes)
            .ok_or_else(|| error("terminal storage overflow"))?;
        bindings
            .install(call.node, call.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let drivers = fragment
        .placements
        .iter()
        .map(|p| match p.kind_id.as_str() {
            FACE_TEE_KIND => Ok(backs::MaskBack::Tee),
            RENDERER_KIND => Ok(backs::MaskBack::Renderer {
                pending: false,
                emitted: false,
            }),
            FACE_INTERACTION_KIND => {
                if p.implementation_id.as_str() == planning::READ_ONLY_INTERACTION_IMPLEMENTATION {
                    Ok(backs::MaskBack::InteractionClose { seen_face: false })
                } else {
                    Ok(backs::MaskBack::Interaction {
                        seen_face: false,
                        pending: false,
                    })
                }
            }
            crate::flow_activation::parsec_instruction_source_offer::KIND =>
                crate::installed_std::parsec_instruction_source_back::ParsecInstructionSourceBack::prepare(p)
                    .map(backs::MaskBack::InstructionSource).map_err(error),
            super::parsec_instruction_effect::KIND => Ok(backs::MaskBack::InstructionEffect(super::parsec_instruction_effect::InstructionEffectBack::new())),
            _ => Err(error("unknown terminal back")),
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| error("terminal back count"))?;
    // Exact admitted one-item queues plus each profile's Host Call result slots.
    let values = HostedValueStore::new((C + H) as u16, MAX_TERMINAL_VALUE_BYTES, bytes)
        .map_err(debug_error)?;
    let signs = HostedSignLog::new_with_remote_storage(
        96,
        (96 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
        32,
        conduit_kernel::remote_sign_storage_bytes(32)
            .ok_or_else(|| error("terminal sign bound"))?,
    )
    .map_err(debug_error)?;
    Profile::<N, C, R, H>::new_with_host_calls(
        lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| error("terminal node count"))?,
        lowered
            .cords
            .iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| error("terminal cord count"))?,
        routes,
        bindings,
        drivers,
        values,
        signs,
    )
    .map_err(debug_error)
}

impl Scheduler {
    pub(super) fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        match self {
            Self::Ordinary(s) => s.step(),
            Self::Parsec(s) => s.step(),
        }
    }
    pub(super) fn next_host_request(&mut self) -> Option<HostCallRequest> {
        match self {
            Self::Ordinary(s) => s.next_host_request(),
            Self::Parsec(s) => s.next_host_request(),
        }
    }
    pub(super) fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        match self {
            Self::Ordinary(s) => s.store_host_value(bytes),
            Self::Parsec(s) => s.store_host_value(bytes),
        }
    }
    pub(super) fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        match self {
            Self::Ordinary(s) => s.host_value(value),
            Self::Parsec(s) => s.host_value(value),
        }
    }
    pub(super) fn remote_egress_offer(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<RemoteValueOffer>, SchedulerError> {
        match self {
            Self::Ordinary(s) => s.remote_egress_offer(endpoint, cord),
            Self::Parsec(s) => s.remote_egress_offer(endpoint, cord),
        }
    }
    pub(super) fn remote_egress_accept(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Ordinary(s) => s.remote_egress_accept(endpoint, cord, sequence),
            Self::Parsec(s) => s.remote_egress_accept(endpoint, cord, sequence),
        }
    }
    pub(super) fn remote_egress_delivered(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Ordinary(s) => s.remote_egress_delivered(endpoint, cord, sequence),
            Self::Parsec(s) => s.remote_egress_delivered(endpoint, cord, sequence),
        }
    }
    pub(super) fn remote_egress_terminal_disposition(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<RemoteTerminalDisposition>, SchedulerError> {
        match self {
            Self::Ordinary(s) => s.remote_egress_terminal_disposition(endpoint, cord),
            Self::Parsec(s) => s.remote_egress_terminal_disposition(endpoint, cord),
        }
    }
    pub(super) fn complete_host_call(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Ordinary(s) => s.complete_host_call(node, request, outcome),
            Self::Parsec(s) => s.complete_host_call(node, request, outcome),
        }
    }
    pub(super) fn admit_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        match self {
            Self::Ordinary(s) => s.admit_remote_input(endpoint, cord, sequence, bytes),
            Self::Parsec(s) => s.admit_remote_input(endpoint, cord, sequence, bytes),
        }
    }
    pub(super) fn close_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Ordinary(s) => s.close_remote_input(endpoint, cord),
            Self::Parsec(s) => s.close_remote_input(endpoint, cord),
        }
    }
    pub(super) fn cancel(&mut self) -> Result<(), SchedulerError> {
        match self {
            Self::Ordinary(s) => s.cancel(),
            Self::Parsec(s) => s.cancel(),
        }
    }
}

impl Scheduler {
    pub(super) fn complete_instruction(
        &mut self,
        admission: &mut crate::installed_std::parsec_instruction_source_admission::ParsecInstructionSourceAdmission,
        request: HostCallRequest,
        qualified: &crate::installed_std::body_kernel::QualifiedParsecInstruction,
        host: &HostAdvertisement,
    ) -> Result<ValueRef, TerminalError> {
        match self {
            Self::Parsec(s) => admission
                .complete_on_scheduler(
                    request,
                    qualified,
                    conduit_face::ParsecInputInstructionLook::Linear,
                    host,
                    s.as_mut(),
                )
                .map_err(error),
            Self::Ordinary(_) => Err(error("ordinary Mask has no instruction Source")),
        }
    }
}

impl Scheduler {
    pub(super) fn instruction_effect_acknowledged(
        &self,
        node: NodeId,
    ) -> Result<bool, TerminalError> {
        match self {
            Self::Parsec(s) => match s.drivers().get(usize::from(node.0)) {
                Some(backs::MaskBack::InstructionEffect(back)) => Ok(back.acknowledged()),
                _ => Err(error(
                    "instruction acknowledgement belongs to a different Back",
                )),
            },
            Self::Ordinary(_) => Err(error("ordinary Mask has no supplementary effect")),
        }
    }
}
