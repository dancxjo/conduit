//! Native admission of an exact ordinary Plot and its retained physical owner.
use crate::{
    current_sample::CurrentSampleOperationFactory,
    expression_host_call::ExpressionOperationFactory,
    i2c_base::{I2cOperationFactory, I2cProvider, installation::ReadyI2cBase},
    protocol_host_calls::{PreparedProtocolCalls, ProtocolCallRefusal},
    structured_selector_host_call::SelectorOperationFactory,
};
use conduit_composite::{
    KernelCompositeDefinition, KernelCompositeHost, KernelCompositeStatus, KernelOperationRegistry,
};
use conduit_core::*;

mod timed;
pub use timed::{ClockAdmission, PreparedTimedProtocolPlay};

pub struct I2cAdmission<P> {
    pub ready: ReadyI2cBase<P>,
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}

/// Preparation owns the admitted kernel and the actual native owners together.
/// The caller supplies the selected Plan and opaque possession; no grant is minted here.
pub struct PreparedProtocolPlay<P> {
    kernel: KernelCompositeHost,
    calls: PreparedProtocolCalls<P>,
}
impl<P: I2cProvider> PreparedProtocolPlay<P> {
    pub fn prepare(
        definition: KernelCompositeDefinition,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_with_joins(
            definition,
            ready,
            table,
            handle,
            claim,
            crate::flow_zip::FlowZipOperationFactory::default(),
        )
    }

    /// Consume the exact typed join owners retained when their Backs were offered.
    pub fn prepare_with_joins(
        definition: KernelCompositeDefinition,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        zip: crate::flow_zip::FlowZipOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_with_operations(
            definition,
            ready,
            table,
            handle,
            claim,
            crate::protocol_operations::ProtocolOperations {
                joins: zip,
                states: crate::seeded_state::SeededStateOperationFactory::default(),
                concats: crate::flow_concat_finite::FlowConcatFiniteOperationFactory::default(),
                merges: crate::flow_merge_finite::FlowMergeFiniteOperationFactory::default(),
            },
        )
    }

    /// Consume retained Source state owners without supplying join specializations.
    pub fn prepare_with_seeded_state(
        definition: KernelCompositeDefinition,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        states: crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_with_operations(
            definition,
            ready,
            table,
            handle,
            claim,
            crate::protocol_operations::ProtocolOperations {
                joins: crate::flow_zip::FlowZipOperationFactory::default(),
                states,
                concats: crate::flow_concat_finite::FlowConcatFiniteOperationFactory::default(),
                merges: crate::flow_merge_finite::FlowMergeFiniteOperationFactory::default(),
            },
        )
    }

    pub fn prepare_with_operations(
        definition: KernelCompositeDefinition,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        operations: crate::protocol_operations::ProtocolOperations,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_mode(definition, ready, table, handle, claim, operations, false)
    }

    fn prepare_mode(
        definition: KernelCompositeDefinition,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        operations: crate::protocol_operations::ProtocolOperations,
        allow_clock: bool,
    ) -> Result<Self, ProtocolCallRefusal> {
        if !allow_clock
            && definition
                .internal_plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .any(|gear| {
                    gear.implementation_id.as_str()
                        == crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION
                })
        {
            return Err(ProtocolCallRefusal::Unsupported);
        }
        let crate::protocol_operations::ProtocolOperations {
            joins: zip,
            states,
            merges,
            concats,
        } = operations;
        validate_fore(&definition)?;
        zip.validate_plan(&definition.internal_plan)
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        states
            .validate_plan(&definition.internal_plan)
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        concats
            .validate_plan(&definition.internal_plan)
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        merges
            .validate_plan(&definition.internal_plan)
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        let calls = PreparedProtocolCalls::prepare_with_operations(
            &definition.internal_plan,
            ready,
            table,
            handle,
            claim,
            &zip,
            &states,
        )?;
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(ExpressionOperationFactory::default())
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        registry
            .install(
                I2cOperationFactory::prepare_contract()
                    .map_err(|_| ProtocolCallRefusal::InvalidPlan)?,
            )
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        registry
            .install(SelectorOperationFactory::default())
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        registry
            .install(zip)
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        registry
            .install(states)
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        registry
            .install(CurrentSampleOperationFactory::default())
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        registry
            .install(concats)
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        registry
            .install(merges)
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        if allow_clock {
            registry
                .install(
                    crate::monotonic_clock::factory::ClockOperationFactory::prepare_contract()
                        .map_err(|_| ProtocolCallRefusal::InvalidPlan)?,
                )
                .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        }
        let kernel = KernelCompositeHost::prepare(definition, &registry)
            .map_err(ProtocolCallRefusal::Kernel)?;
        Ok(Self { kernel, calls })
    }

    pub fn start(&mut self) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .start()
            .map(|_| ())
            .map_err(ProtocolCallRefusal::Kernel)
    }

    pub fn admit_input(
        &mut self,
        port: &PortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<conduit_kernel::scheduler::RemoteIngressOutcome, ProtocolCallRefusal> {
        self.kernel
            .admit_input(port, sequence, value)
            .map_err(ProtocolCallRefusal::Kernel)
    }

    pub fn close_input(&mut self, port: &PortId) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .close_input(port)
            .map_err(ProtocolCallRefusal::Kernel)
    }

    pub fn output_into(
        &mut self,
        port: &PortId,
        output: &mut ValuePayload,
    ) -> Result<Option<u64>, ProtocolCallRefusal> {
        self.kernel
            .output_into(port, output)
            .map_err(ProtocolCallRefusal::Kernel)
    }

    pub fn complete_output(
        &mut self,
        port: &PortId,
        sequence: u64,
    ) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .complete_output(port, sequence)
            .map_err(ProtocolCallRefusal::Kernel)
    }

    /// Inspection cannot cancel or replace the admitted kernel behind the native owners.
    pub fn kernel(&self) -> &KernelCompositeHost {
        &self.kernel
    }

    /// Advance the sole kernel once and service at most one surfaced Host Call.
    pub fn step(&mut self) -> Result<KernelCompositeStatus, ProtocolCallRefusal> {
        let status = self.kernel.step().map_err(ProtocolCallRefusal::Kernel)?;
        if let Some(request) = self.kernel.next_host_request() {
            self.calls.dispatch(&mut self.kernel, &request)?;
        }
        Ok(status)
    }

    pub fn cancel(&mut self) -> Result<(), ProtocolCallRefusal> {
        self.calls.cancel(&mut self.kernel)
    }
}

pub(crate) fn validate_fore(
    definition: &KernelCompositeDefinition,
) -> Result<(), ProtocolCallRefusal> {
    let plan = &definition.internal_plan;
    if !verify_plan(plan) || plan.fragments.len() != 1 {
        return Err(ProtocolCallRefusal::InvalidPlan);
    }
    let fragment = &plan.fragments[0];
    if definition.host_id != fragment.host_id
        || definition.boot_id != fragment.boot_id
        || definition.offer_generation != fragment.offer_generation
        || fragment
            .fore_ports
            .iter()
            .map(|fore| (&fore.front_port_id, fore.direction == PortDirection::Input))
            .collect::<alloc::collections::BTreeSet<_>>()
            .len()
            != definition.boundary.input_fronts.len() + definition.boundary.output_fronts.len()
    {
        return Err(ProtocolCallRefusal::InvalidPlan);
    }
    for (fronts, direction, ports) in [
        (
            &definition.boundary.input_fronts,
            PortDirection::Input,
            &definition.external_capability.inputs,
        ),
        (
            &definition.boundary.output_fronts,
            PortDirection::Output,
            &definition.external_capability.outputs,
        ),
    ] {
        if fronts.len() != ports.len() {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        for front in fronts {
            let mut matching = fragment.fore_ports.iter().filter(|fore| {
                fore.front_port_id == front.external_port.port_id && fore.direction == direction
            });
            let fore = matching.next().ok_or(ProtocolCallRefusal::InvalidPlan)?;
            if (matching.clone().next().is_some() && direction != PortDirection::Input)
                || matching.any(|candidate| {
                    candidate.track != fore.track
                        || candidate.value_kind != fore.value_kind
                        || candidate.temporal != fore.temporal
                        || candidate.abnormal_kind != fore.abnormal_kind
                        || candidate.value_contract != fore.value_contract
                })
                || fore.track != ConnectionTrack::Payload
                || front.terminal != conduit_plot::CompositeFrontTerminal::Independent
                || front.internal_child != fragment.host_id
                || front.internal_placement_id != fore.placement_id
                || front.internal_port_id != fore.gear_port_id
                || front.external_port.direction != direction
                || front.external_port.value_kind != fore.value_kind
                || front.external_port.temporal != fore.temporal
                || front.external_port.abnormal_kind != fore.abnormal_kind
                || ports
                    .iter()
                    .filter(|port| **port == front.external_port)
                    .count()
                    != 1
                || fronts
                    .iter()
                    .filter(|candidate| candidate.external_port.port_id == fore.front_port_id)
                    .count()
                    != 1
            {
                return Err(ProtocolCallRefusal::InvalidPlan);
            }
        }
    }
    Ok(())
}
