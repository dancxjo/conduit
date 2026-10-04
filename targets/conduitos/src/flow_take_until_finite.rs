//! Retained schema specializations for the generic finite context merge.
use alloc::{boxed::Box, collections::BTreeMap, format, string::String, vec::Vec};
use conduit_composite::{FlowTakeUntilFiniteBack, KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub const IMPLEMENTATION: &str = "conduitos/flow-take-until-finite@1";
pub const PROFILE: &str = "conduitos/flow-take-until-finite-prepared@1";
const ARTIFACT: &str = "conduit-composite/flow-take-until-finite@1";
const MAXIMUM_BYTES: u32 = 4096;
const MAXIMUM_SPECIALIZATIONS: usize = 16;
struct OfferedTakeUntil {
    schema: StructuredInfoType,
    offer: CapabilityOffer,
}
pub struct FlowTakeUntilFiniteOperationFactory {
    implementation: ImplementationId,
    offers: BTreeMap<CapabilityId, OfferedTakeUntil>,
}
impl Default for FlowTakeUntilFiniteOperationFactory {
    fn default() -> Self {
        Self {
            implementation: IMPLEMENTATION.into(),
            offers: BTreeMap::new(),
        }
    }
}
impl FlowTakeUntilFiniteOperationFactory {
    pub fn install(
        &mut self,
        value: &CheckedValueContract,
        schema: &StructuredInfoType,
    ) -> Result<CapabilityOffer, String> {
        if value.maximum_bytes > MAXIMUM_BYTES || self.offers.len() == MAXIMUM_SPECIALIZATIONS {
            return Err("native finite take-until exceeds its prepared offer envelope".into());
        }
        let kind =
            conduit_semantic_catalog::flow_take_until_finite_semantic_contract(value, schema)
                .map_err(String::from)?;
        let offer = BackOfferBuilder::new(
            kind,
            Back {
                capability_id: format!(
                    "conduitos/flow-take-until-finite/{}/{}@1",
                    value.value_kind.as_str(),
                    value.maximum_bytes
                )
                .into(),
                execution_profile_id: PROFILE.into(),
                implementation_id: IMPLEMENTATION.into(),
                artifact_id: ARTIFACT.into(),
                host_calls: Vec::new(),
                resource_requirements: Vec::new(),
                authority_requirements: Vec::new(),
            },
        )
        .build();
        if self.offers.contains_key(&offer.capability_id) {
            return Err("native finite take-until specialization already installed".into());
        }
        self.offers.insert(
            offer.capability_id.clone(),
            OfferedTakeUntil {
                schema: schema.clone(),
                offer: offer.clone(),
            },
        );
        Ok(offer)
    }
    pub fn validate_plan(&self, plan: &Plan) -> Result<(), String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("native finite take-until requires one sealed fragment".into());
        }
        for gear in plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.implementation_id.as_str() == IMPLEMENTATION)
        {
            self.selected(gear)?;
        }
        Ok(())
    }
    fn selected<'a>(&'a self, gear: &PlannedGear) -> Result<(&'a OfferedTakeUntil, u32), String> {
        let retained = self
            .offers
            .get(&gear.capability_id)
            .ok_or("native finite take-until has no retained offer")?;
        let expected = &retained.offer;
        if gear.kind_id != expected.kind_id
            || gear.kind_contract_revision != expected.kind_contract_revision
            || gear.capability_id != expected.capability_id
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
            return Err("native finite take-until differs from its selected realization".into());
        }
        let maximum = expected
            .semantic_contract
            .value_contracts()
            .iter()
            .find(|entry| entry.location == FrontValueLocation::Output(port_id("taken")))
            .ok_or("native finite take-until output contract missing")?
            .contract
            .maximum_bytes;
        Ok((retained, maximum))
    }
}
impl KernelOperationFactory for FlowTakeUntilFiniteOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let (_, maximum) = self.selected(gear)?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: maximum.max(1),
            maximum_value_bytes: maximum.max(1),
            host_requests: 0,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        let (retained, maximum) = self.selected(gear)?;
        Ok(Box::new(
            FlowTakeUntilFiniteBack::prepare(&retained.schema, maximum)
                .map_err(|error| format!("{error:?}"))?,
        ))
    }
}
