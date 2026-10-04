//! Native finite pairing retains the exact schemas and Back offered before planning.
use alloc::{boxed::Box, collections::BTreeMap, format, string::String, vec::Vec};
use conduit_composite::{FlowZipBack, KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub const IMPLEMENTATION: &str = "conduitos/flow-zip-finite@1";
pub const PROFILE: &str = "conduitos/flow-zip-finite-prepared@1";
pub const ARTIFACT: &str = "conduit-composite/flow-zip@1";
pub const MAXIMUM_INPUT_BYTES: u32 = 4096;
pub const MAXIMUM_PAIR_BYTES: u32 = 16_384;
pub const MAXIMUM_SPECIALIZATIONS: usize = 16;

fn offer(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
    feedback: bool,
) -> Result<CapabilityOffer, String> {
    if left.maximum_bytes > MAXIMUM_INPUT_BYTES || right.maximum_bytes > MAXIMUM_INPUT_BYTES {
        return Err("native finite zip input exceeds its prepared profile".into());
    }
    let semantic = if feedback {
        conduit_semantic_catalog::flow_zip_feedback_semantic_contract
    } else {
        conduit_semantic_catalog::flow_zip_finite_semantic_contract
    };
    let kind = semantic(left, left_type, right, right_type).map_err(String::from)?;
    let encoder = PreparedTypedTuplePairEncoder::new(
        left_type.clone(),
        left.maximum_bytes,
        right_type.clone(),
        right.maximum_bytes,
    )
    .map_err(|error| format!("{error:?}"))?;
    if encoder.maximum_bytes() > MAXIMUM_PAIR_BYTES {
        return Err("native finite zip pair exceeds its prepared profile".into());
    }
    let kind_name = if feedback {
        "flow-zip-feedback"
    } else {
        "flow-zip-finite"
    };
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!(
                "conduitos/{}/{}/{}/{}/{}@1",
                kind_name,
                left.value_kind.as_str(),
                left.maximum_bytes,
                right.value_kind.as_str(),
                right.maximum_bytes
            )),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

struct OfferedPair {
    left: StructuredInfoType,
    right: StructuredInfoType,
    offer: CapabilityOffer,
}

/// Finite schema owners installed while preparing the host advertisement.
/// Selected Plans must match one of these exact retained offers before Play.
pub struct FlowZipOperationFactory {
    implementation: ImplementationId,
    pairs: BTreeMap<CapabilityId, OfferedPair>,
}
impl Default for FlowZipOperationFactory {
    fn default() -> Self {
        Self {
            implementation: ImplementationId::from(IMPLEMENTATION),
            pairs: BTreeMap::new(),
        }
    }
}
impl FlowZipOperationFactory {
    pub fn install(
        &mut self,
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
    ) -> Result<CapabilityOffer, String> {
        self.install_mode(left, left_type, right, right_type, false)
    }

    pub fn install_feedback(
        &mut self,
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
    ) -> Result<CapabilityOffer, String> {
        self.install_mode(left, left_type, right, right_type, true)
    }

    fn install_mode(
        &mut self,
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
        feedback: bool,
    ) -> Result<CapabilityOffer, String> {
        if self.pairs.len() == MAXIMUM_SPECIALIZATIONS {
            return Err("native finite zip exceeds its admitted offer count".into());
        }
        let offer = offer(left, left_type, right, right_type, feedback)?;
        if self.pairs.contains_key(&offer.capability_id) {
            return Err("native finite zip specialization is already installed".into());
        }
        self.pairs.insert(
            offer.capability_id.clone(),
            OfferedPair {
                left: left_type.clone(),
                right: right_type.clone(),
                offer: offer.clone(),
            },
        );
        Ok(offer)
    }

    pub fn offers(&self) -> impl Iterator<Item = &CapabilityOffer> {
        self.pairs.values().map(|pair| &pair.offer)
    }

    pub fn validate_plan(&self, plan: &Plan) -> Result<(), String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("native finite zip requires one exact sealed fragment".into());
        }
        for gear in plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.implementation_id.as_str() == IMPLEMENTATION)
        {
            self.budget(gear)?;
        }
        Ok(())
    }

    fn selected<'a>(
        &'a self,
        gear: &'a PlannedGear,
    ) -> Result<
        (
            &'a OfferedPair,
            &'a CheckedValueContract,
            &'a CheckedValueContract,
            u32,
        ),
        String,
    > {
        let types = self
            .pairs
            .get(&gear.capability_id)
            .ok_or("native finite zip has no exact retained offer")?;
        let expected = &types.offer;
        let contract_at = |name| {
            expected
                .semantic_contract
                .value_contracts()
                .iter()
                .find(|entry| entry.location == FrontValueLocation::Input(port_id(name)))
                .map(|entry| &entry.contract)
        };
        let left = contract_at("left").ok_or("native finite zip left contract is absent")?;
        let right = contract_at("right").ok_or("native finite zip right contract is absent")?;
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
            return Err("native finite zip differs from its exact selected realization".into());
        }
        let pair = expected
            .semantic_contract
            .value_contracts()
            .iter()
            .find(|entry| entry.location == FrontValueLocation::Output(port_id("paired")))
            .ok_or("native finite zip paired contract is absent")?
            .contract
            .maximum_bytes;
        Ok((types, left, right, pair))
    }
}
impl KernelOperationFactory for FlowZipOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let (_, left, right, pair) = self.selected(gear)?;
        Ok(KernelOperationBudget {
            value_items: 3,
            value_bytes: left
                .maximum_bytes
                .checked_add(right.maximum_bytes)
                .and_then(|bytes| bytes.checked_add(pair))
                .ok_or("native finite zip value budget overflows")?,
            maximum_value_bytes: pair.max(left.maximum_bytes).max(right.maximum_bytes).max(1),
            host_requests: 0,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        let (types, left, right, _) = self.selected(gear)?;
        let prepare = if gear.kind_id.as_str() == conduit_semantic_catalog::FLOW_ZIP_FEEDBACK_KIND {
            FlowZipBack::prepare_typed_feedback
        } else {
            FlowZipBack::prepare_typed_finite
        };
        Ok(Box::new(
            prepare(left, types.left.clone(), right, types.right.clone())
                .map_err(|error| format!("{error:?}"))?,
        ))
    }
}

#[cfg(test)]
mod tests;
