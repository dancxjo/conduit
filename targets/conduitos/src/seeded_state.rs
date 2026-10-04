//! Retained exact native offers for generic Source-seeded state cells.
use alloc::{boxed::Box, format, string::String, vec::Vec};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory, SeededStateBack};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub const IMPLEMENTATION: &str = "conduitos/seeded-state-finite@1";
pub const PROFILE: &str = "conduitos/seeded-state-finite-prepared@1";
pub const ARTIFACT: &str = "conduit-composite/seeded-state@1";
pub const MAXIMUM_BYTES: u32 = 4096;
pub const MAXIMUM_SPECIALIZATIONS: usize = 16;

struct OfferedState {
    schema: StructuredInfoType,
    value: CheckedValueContract,
    offer: CapabilityOffer,
}

pub struct SeededStateOperationFactory {
    implementation: ImplementationId,
    states: Vec<OfferedState>,
}

impl Default for SeededStateOperationFactory {
    fn default() -> Self {
        Self {
            implementation: ImplementationId::from(IMPLEMENTATION),
            states: Vec::new(),
        }
    }
}

impl SeededStateOperationFactory {
    /// Retain the schema that actually supplied this Back before planning.
    /// Failed installation never changes the initialized offer set.
    pub fn install(
        &mut self,
        value: &CheckedValueContract,
        schema: &StructuredInfoType,
    ) -> Result<CapabilityOffer, &'static str> {
        self.install_mode(value, schema, false)
    }

    /// Retain a closing observation Flow with exactly one item per generation.
    pub fn install_flow(
        &mut self,
        value: &CheckedValueContract,
        schema: &StructuredInfoType,
    ) -> Result<CapabilityOffer, &'static str> {
        self.install_mode(value, schema, true)
    }

    fn install_mode(
        &mut self,
        value: &CheckedValueContract,
        schema: &StructuredInfoType,
        flow: bool,
    ) -> Result<CapabilityOffer, &'static str> {
        if self.states.len() >= MAXIMUM_SPECIALIZATIONS || value.maximum_bytes > MAXIMUM_BYTES {
            return Err("native seeded state exceeds its finite prepared profile");
        }
        let kind = if flow {
            conduit_semantic_catalog::seeded_state_flow_semantic_contract(value, schema)?
        } else {
            conduit_semantic_catalog::seeded_state_semantic_contract(value, schema)?
        };
        let offer = BackOfferBuilder::new(
            kind,
            Back {
                capability_id: CapabilityId::from(format!(
                    "conduitos/{}/{}/{}@1",
                    if flow {
                        "seeded-state-flow-finite"
                    } else {
                        "seeded-state-finite"
                    },
                    value.value_kind.as_str(),
                    value.maximum_bytes
                )),
                execution_profile_id: ExecutionProfileId::from(PROFILE),
                implementation_id: ImplementationId::from(IMPLEMENTATION),
                artifact_id: ArtifactId::from(ARTIFACT),
                host_calls: Vec::new(),
                resource_requirements: Vec::new(),
                authority_requirements: Vec::new(),
            },
        )
        .build();
        if self
            .states
            .iter()
            .any(|state| state.offer.capability_id == offer.capability_id)
        {
            return Err("native seeded state specialization is already initialized");
        }
        self.states.push(OfferedState {
            schema: schema.clone(),
            value: value.clone(),
            offer: offer.clone(),
        });
        Ok(offer)
    }

    pub fn offers(&self) -> impl Iterator<Item = &CapabilityOffer> {
        self.states.iter().map(|state| &state.offer)
    }

    pub fn validate_plan(&self, plan: &Plan) -> Result<(), String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("native seeded state requires one exact sealed fragment".into());
        }
        for gear in &plan.fragments[0].placements {
            if gear.implementation_id.as_str() == IMPLEMENTATION {
                self.selected(gear)?;
            }
        }
        Ok(())
    }

    fn selected(&self, gear: &PlannedGear) -> Result<&OfferedState, String> {
        let state = self
            .states
            .iter()
            .find(|state| state.offer.capability_id == gear.capability_id)
            .ok_or("native seeded state has no retained initialized schema owner")?;
        let expected = &state.offer;
        if gear.kind_id != expected.kind_id
            || gear.kind_contract_revision != expected.kind_contract_revision
            || gear.execution_profile_id != expected.implementation.execution_profile_id
            || gear.implementation_id != expected.implementation.implementation_id
            || gear.artifact_id != expected.implementation.artifact_id
            || gear.inputs != expected.inputs
            || gear.outputs != expected.outputs
            || gear.semantic_contract != expected.semantic_contract
            || gear.limits != expected.limits
            || !gear.configuration.is_empty()
            || !gear.host_calls.is_empty()
            || gear.base.is_some()
            || !gear.resources.is_empty()
            || !gear.authority.is_empty()
            || !gear.pool_references.is_empty()
        {
            return Err("native seeded state differs from its exact offered realization".into());
        }
        Ok(state)
    }
}

impl KernelOperationFactory for SeededStateOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }

    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let state = self.selected(gear)?;
        // One pool reference retains Current between observations. The Back
        // owns no private byte buffers or physical operations.
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: state.value.maximum_bytes,
            maximum_value_bytes: state.value.maximum_bytes.max(1),
            host_requests: 0,
            sign_items: 32,
        })
    }

    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        let state = self.selected(gear)?;
        let back = if state.offer.outputs[0].temporal == (PortTemporal::Flow { closes: true }) {
            SeededStateBack::prepare_flow(&state.value, &state.schema)
        } else {
            SeededStateBack::prepare(&state.value, &state.schema)
        }
        .map_err(String::from)?;
        Ok(Box::new(back))
    }
}

#[cfg(test)]
mod tests;
