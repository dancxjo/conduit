//! Exact source-evaluator-backed child operation for the finite Parsec scan.
use super::{parsec_combine_back::ParsecCombineBack, ParsecSourcePlans};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{
    kind_id, port_id, Back, BackOfferBuilder, CapabilityLimits, CapabilityOffer,
    CheckedValueContract, FrontValueContract, FrontValueLocation, ImplementationId, Kind,
    KindSemanticLaw, KindTerminalBehavior, PlannedGear, PlannedScanActivation, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_face::{ParsecMapCommand, ParsecMapTransitionResult, PARSEC_MAP_FRAME_BYTES};
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::sync::Arc;

pub const KIND: &str = "parsec/map-combine";
pub const REVISION: &str = "conduit.parsec/map-combine@1";
pub const IMPLEMENTATION: &str = "std/kernel-parsec-source-combine@1";
pub fn command_kind() -> Result<conduit_core::KindId, String> {
    Ok(ParsecMapCommand::semantic_type()
        .map_err(|e| format!("{e:?}"))?
        .profile()
        .map_err(|e| format!("{e:?}"))?
        .value_kind()
        .clone())
}
pub fn result_kind() -> Result<conduit_core::KindId, String> {
    Ok(ParsecMapTransitionResult::semantic_type()
        .map_err(|e| format!("{e:?}"))?
        .profile()
        .map_err(|e| format!("{e:?}"))?
        .value_kind()
        .clone())
}
pub fn parsec_combine_kind() -> Result<Kind, String> {
    let command = command_kind()?;
    let state = result_kind()?;
    let port = |name: &str, value_kind: &conduit_core::KindId, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value_kind.clone(),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let contract = |name,
                    value_kind: conduit_core::KindId,
                    input|
     -> Result<FrontValueContract, String> {
        Ok(FrontValueContract {
            location: if input {
                FrontValueLocation::Input(port_id(name))
            } else {
                FrontValueLocation::Output(port_id(name))
            },
            contract: CheckedValueContract::new(value_kind, PARSEC_MAP_FRAME_BYTES as u32, vec![])
                .map_err(|e| format!("{e:?}"))?,
        })
    };
    Ok(Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(KIND),
        kind_contract_revision: REVISION.into(),
        inputs: vec![
            port("accumulator", &state, PortDirection::Input),
            port("item", &command, PortDirection::Input),
        ],
        outputs: vec![port("combined", &state, PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                contract("accumulator", state.clone(), true)?,
                contract("item", command, true)?,
                contract("combined", state, false)?,
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: (3 * PARSEC_MAP_FRAME_BYTES) as u32,
        },
    })
}
pub fn parsec_combine_offer() -> Result<CapabilityOffer, String> {
    Ok(BackOfferBuilder::new(
        parsec_combine_kind()?,
        Back {
            capability_id: "std/parsec-source-combine@1".into(),
            execution_profile_id: "conduit.std/parsec-source-combine@1".into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/parsec-source-combine@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub struct ParsecCombineFactory {
    sources: Arc<ParsecSourcePlans>,
    implementation: ImplementationId,
}
impl ParsecCombineFactory {
    pub fn new(sources: Arc<ParsecSourcePlans>) -> Self {
        Self {
            sources,
            implementation: IMPLEMENTATION.into(),
        }
    }
    pub(super) fn validate(&self, placement: &PlannedGear) -> Result<(), String> {
        validate_placement(placement)?;
        let host = self.sources.host_identity();
        if placement.host_id != host.host_id
            || placement.boot_id != host.boot_id
            || placement.offer_generation != host.offer_generation
        {
            return Err(
                "Parsec child differs from computational producer Host/Boot/generation".into(),
            );
        }
        Ok(())
    }
}
pub(super) fn validate_placement(placement: &PlannedGear) -> Result<(), String> {
    let expected = parsec_combine_offer()?;
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.terminal_transductions.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("Parsec combine differs from exact installed source Back".into());
    }
    Ok(())
}
pub(super) fn maximum_scan_child_steps(planned: &PlannedScanActivation) -> Result<u32, String> {
    let plan = &planned.selected_plan;
    if !conduit_core::verify_plan(plan) || plan.fragments.len() != 1 {
        return Err("Parsec child requires one sealed local Plan".into());
    }
    let fragment = &plan.fragments[0];
    if fragment.placements.len() != 1
        || !fragment.connections.is_empty()
        || !fragment.states.is_empty()
        || !fragment.execution_regions.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.shared_pools.is_empty()
        || fragment.fore_ports.len() != 3
        || fragment.expected_terminals
            != [
                conduit_core::ExpectedTerminal::PlacementCompleted(
                    fragment.placements[0].placement_id.clone(),
                ),
                conduit_core::ExpectedTerminal::PlanCompleted,
            ]
        || fragment.terminal_policy
            != conduit_core::TerminalPolicy::RequireAllPlacementsAndConnections
        || fragment.cancellation_policy
            != conduit_core::CancellationPolicy::CancelAllAndRejectLateCompletion
    {
        return Err("Parsec child requires exact pure three-Fore combine".into());
    }
    validate_placement(&fragment.placements[0])?;
    for (expected, direction) in [
        (&planned.accumulator_input, PortDirection::Input),
        (&planned.item_input, PortDirection::Input),
        (&planned.output, PortDirection::Output),
    ] {
        if !fragment.fore_ports.iter().any(|port| {
            port.front_port_id == expected.front_port_id
                && port.direction == direction
                && port.placement_id == fragment.placements[0].placement_id
                && port.gear_port_id == expected.front_port_id
                && port.value_kind == expected.value_kind
                && port.abnormal_kind.is_none()
                && port.track == conduit_core::ConnectionTrack::Payload
                && port.temporal == PortTemporal::Value
                && port.item_capacity == 1
                && port.byte_capacity == PARSEC_MAP_FRAME_BYTES as u32
                && port.selected_line.is_none()
        }) {
            return Err("Parsec child differs from exact bounded Value Fore".into());
        }
    }
    u32::from(planned.limits.maximum_items)
        .checked_mul(2)
        .ok_or_else(|| "Parsec child step budget overflow".into())
}
impl KernelOperationFactory for ParsecCombineFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.validate(placement)?;
        Ok(KernelOperationBudget {
            value_items: 3,
            value_bytes: (3 * PARSEC_MAP_FRAME_BYTES) as u32,
            maximum_value_bytes: PARSEC_MAP_FRAME_BYTES as u32,
            host_requests: 0,
            sign_items: 3,
        })
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        self.validate(placement)?;
        Ok(Box::new(ParsecCombineBack::prepare(
            self.sources.prepare_evaluator()?,
        )?))
    }
}
