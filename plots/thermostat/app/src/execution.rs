//! Check authored meaning, select an exact Back, execute via the production kernel.
use conduit_composite::{
    KernelCompositeDefinition, KernelCompositeHost, KernelCompositeStatus, KernelOperationBudget,
    KernelOperationFactory, KernelOperationRegistry,
};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_plot::{parse, ProfileCatalog, StartupCatalog};
use conduit_thermostat_plot::*;

const IMPLEMENTATION: &str = "std/thermostat-combine@1";
fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        thermostat_kind(),
        Back {
            capability_id: "std/thermostat".into(),
            execution_profile_id: IMPLEMENTATION.into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: IMPLEMENTATION.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
struct Factory(ImplementationId);
impl KernelOperationFactory for Factory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.0
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let expected = offer();
        if placement.kind_id != expected.kind_id
            || placement.kind_contract_revision != expected.kind_contract_revision
            || placement.capability_id != expected.capability_id
            || placement.implementation_id != expected.implementation.implementation_id
            || placement.execution_profile_id != expected.implementation.execution_profile_id
            || placement.artifact_id != expected.implementation.artifact_id
            || placement.inputs != expected.inputs
            || placement.outputs != expected.outputs
            || placement.semantic_contract != expected.semantic_contract
            || placement.limits != expected.limits
            || !placement.configuration.is_empty()
            || !placement.host_calls.is_empty()
            || !placement.resources.is_empty()
            || !placement.authority.is_empty()
            || !placement.pool_references.is_empty()
            || placement.base.is_some()
            || !placement.realization_characteristics.is_empty()
            || !placement.realization_properties.is_empty()
            || !placement.terminal_transductions.is_empty()
        {
            return Err("thermostat Back identity or bounds differ from installed offer".into());
        }
        Ok(KernelOperationBudget {
            value_items: 3,
            value_bytes: (2 * STATE_BYTES + COMMAND_BYTES) as u32,
            maximum_value_bytes: STATE_BYTES as u32,
            host_requests: 0,
            sign_items: 4,
        })
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.budget(placement)?;
        Ok(Box::new(ThermostatBack::new()))
    }
}
pub struct Execution {
    definition: KernelCompositeDefinition,
    registry: KernelOperationRegistry,
    sequence: u64,
}
pub struct ResultState {
    pub state: ThermostatState,
    pub basis: conduit_presentation::PresentationContributionBasis,
}
fn error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
impl Execution {
    pub fn new() -> Result<Self, String> {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_catalogs(&mut startup, &mut profile)?;
        let source = include_str!("../../main.conduit");
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .map_err(error)?;
        let checked = parse(source, &profile).map_err(error)?;
        let boot = format!(
            "thermostat-boot/{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(error)?
                .as_nanos()
        );
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: "thermostat-local".into(),
            boot_id: boot.into(),
            offer_generation: OfferGeneration(1),
            profile: "thermostat-control-demo@1".into(),
            bases: vec![],
            resources: vec![],
            planner_capabilities: vec![],
            capabilities: vec![offer()],
        };
        let placements =
            conduit_planner::default_placements(&checked, core::slice::from_ref(&host))
                .map_err(error)?;
        let plan = conduit_planner::plan(&checked, core::slice::from_ref(&host), &placements, &[])
            .map_err(error)?;
        let definition = KernelCompositeDefinition::from_authored_export(
            host.host_id.clone(),
            host.boot_id.clone(),
            host.offer_generation,
            host.profile.clone(),
            "thermostat/demo@1".into(),
            "thermostat/demo@1".into(),
            &checked,
            &CapabilityId::from("thermostat/main"),
            plan,
            FailureReason::CompositeCapabilityFailed,
        )
        .map_err(error)?;
        let mut registry = KernelOperationRegistry::new();
        registry.install(Factory(IMPLEMENTATION.into()))?;
        Ok(Self {
            definition,
            registry,
            sequence: 0,
        })
    }
    pub fn execute(
        &mut self,
        state: ThermostatState,
        command: Command,
    ) -> Result<ResultState, String> {
        let plan = &self.definition.internal_plan;
        let parent = bind_active_play(
            &plan.plan_id,
            &plan.fragments[0].host_id,
            &plan.fragments[0].boot_id,
            self.sequence,
        );
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("thermostat Play identity exhausted")?;
        let mut kernel =
            KernelCompositeHost::prepare(self.definition.clone(), &self.registry).map_err(error)?;
        kernel
            .bind_parent_play(&parent.active_play_id, "thermostat-command", 0)
            .map_err(error)?;
        let input_values = [
            (
                port_id("accumulator"),
                ValuePayload {
                    value_kind: kind_id(STATE_KIND),
                    encoded: state.encode().map_err(error)?.to_vec(),
                },
            ),
            (
                port_id("item"),
                ValuePayload {
                    value_kind: kind_id(COMMAND_KIND),
                    encoded: command.encode().to_vec(),
                },
            ),
        ];
        let output_port = port_id("combined");
        let mut output = ValuePayload {
            value_kind: kind_id(STATE_KIND),
            encoded: Vec::with_capacity(STATE_BYTES),
        };
        kernel.start().map_err(error)?;
        for (port, value) in &input_values {
            let admitted = kernel.admit_input(port, 0, value).map_err(error)?;
            if !matches!(
                admitted,
                conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
            ) {
                return Err(format!("thermostat ingress refused: {admitted:?}"));
            }
            kernel.close_input(port).map_err(error)?;
        }
        let mut next = None;
        for _ in 0..16 {
            let status = kernel.step().map_err(error)?;
            if let Some(sequence) = kernel
                .output_into(&output_port, &mut output)
                .map_err(error)?
            {
                next = Some(ThermostatState::decode(&output.encoded).map_err(error)?);
                kernel
                    .complete_output(&output_port, sequence)
                    .map_err(error)?;
            }
            if status == KernelCompositeStatus::Complete {
                return Ok(ResultState {
                    state: next.ok_or("thermostat completed without a state")?,
                    basis: conduit_presentation::PresentationContributionBasis {
                        checked_plot_id: plan.checked_plot_id.clone(),
                        plan_id: plan.plan_id.clone(),
                        active_play_id: kernel
                            .active_plays()
                            .values()
                            .next()
                            .ok_or("missing thermostat Play")?
                            .clone(),
                        required_interaction_context: None,
                    },
                });
            }
        }
        kernel.cancel().map_err(error)?;
        Err("thermostat exceeded its admitted step bound".into())
    }
}
