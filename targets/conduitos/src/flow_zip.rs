//! Native finite pairing retains the exact schemas and Back offered before planning.
use alloc::{boxed::Box, format, string::String, vec::Vec};
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
pub const FRAME16K_IMPLEMENTATION: &str = "conduitos/flow-zip-finite-frame16k@1";
pub const FRAME16K_PROFILE: &str = "conduitos/flow-zip-finite-frame16k-prepared@1";

#[derive(Clone, Copy)]
struct PairProfile {
    implementation: &'static str,
    execution: &'static str,
    maximum_input: u32,
    capability_prefix: &'static str,
}
const DEFAULT_PAIR_PROFILE: PairProfile = PairProfile {
    implementation: IMPLEMENTATION,
    execution: PROFILE,
    maximum_input: MAXIMUM_INPUT_BYTES,
    capability_prefix: "conduitos",
};
const FRAME16K_PAIR_PROFILE: PairProfile = PairProfile {
    implementation: FRAME16K_IMPLEMENTATION,
    execution: FRAME16K_PROFILE,
    maximum_input: MAXIMUM_PAIR_BYTES,
    capability_prefix: "conduitos/frame16k",
};

fn offer(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
    feedback: bool,
    specialized: bool,
    selected: PairProfile,
) -> Result<CapabilityOffer, String> {
    if left.maximum_bytes > selected.maximum_input || right.maximum_bytes > selected.maximum_input {
        return Err("native finite zip input exceeds its prepared profile".into());
    }
    let semantic = match (feedback, specialized) {
        (false, false) => conduit_semantic_catalog::flow_zip_finite_semantic_contract,
        (true, false) => conduit_semantic_catalog::flow_zip_feedback_semantic_contract,
        (false, true) => conduit_semantic_catalog::flow_zip_finite_specialized_semantic_contract,
        (true, true) => conduit_semantic_catalog::flow_zip_feedback_specialized_semantic_contract,
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
    let capability = if specialized {
        format!("{}/{}", selected.capability_prefix, kind.kind_id.as_str())
    } else {
        format!(
            "{}/{}/{}/{}/{}/{}@1",
            selected.capability_prefix,
            kind_name,
            left.value_kind.as_str(),
            left.maximum_bytes,
            right.value_kind.as_str(),
            right.maximum_bytes
        )
    };
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(selected.execution),
            implementation_id: ImplementationId::from(selected.implementation),
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
    feedback: bool,
}

/// Finite schema owners installed while preparing the host advertisement.
/// Selected Plans must match one of these exact retained offers before Play.
pub struct FlowZipOperationFactory {
    implementation: ImplementationId,
    profile: PairProfile,
    pairs: Vec<(CapabilityId, OfferedPair)>,
}
impl Default for FlowZipOperationFactory {
    fn default() -> Self {
        Self {
            implementation: ImplementationId::from(IMPLEMENTATION),
            profile: DEFAULT_PAIR_PROFILE,
            pairs: Vec::new(),
        }
    }
}
impl FlowZipOperationFactory {
    /// Explicit larger-input realization for finite frames. The complete pair
    /// still must fit 16 KiB; default protocol owners retain their 4 KiB bound.
    /// Each selected placement retains its exact schemas and storage budget.
    pub fn frame16k() -> Self {
        Self {
            implementation: ImplementationId::from(FRAME16K_IMPLEMENTATION),
            profile: FRAME16K_PAIR_PROFILE,
            pairs: Vec::new(),
        }
    }

    /// Reserves the unchanged finite16-specialization array before installation.
    /// This quota covers the selected array only, not Type/offer construction.
    pub fn frame16k_with_selection_storage_limit(
        maximum_requested_bytes: usize,
    ) -> Result<Self, &'static str> {
        let bytes = MAXIMUM_SPECIALIZATIONS
            .checked_mul(core::mem::size_of::<(CapabilityId, OfferedPair)>())
            .ok_or("zip selection storage overflow")?;
        if bytes > maximum_requested_bytes {
            return Err("zip selection storage capacity");
        }
        Ok(Self {
            implementation: ImplementationId::from(FRAME16K_IMPLEMENTATION),
            profile: FRAME16K_PAIR_PROFILE,
            pairs: Vec::with_capacity(MAXIMUM_SPECIALIZATIONS),
        })
    }
    pub fn selection_array_capacity_bytes(&self) -> usize {
        self.pairs
            .capacity()
            .saturating_mul(core::mem::size_of::<(CapabilityId, OfferedPair)>())
    }

    pub fn install(
        &mut self,
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
    ) -> Result<CapabilityOffer, String> {
        self.install_mode(left, left_type, right, right_type, false, false)
    }

    pub fn install_feedback(
        &mut self,
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
    ) -> Result<CapabilityOffer, String> {
        self.install_mode(left, left_type, right, right_type, true, false)
    }

    pub fn install_specialized(
        &mut self,
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
    ) -> Result<CapabilityOffer, String> {
        self.install_mode(left, left_type, right, right_type, false, true)
    }

    pub fn install_feedback_specialized(
        &mut self,
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
    ) -> Result<CapabilityOffer, String> {
        self.install_mode(left, left_type, right, right_type, true, true)
    }

    fn install_mode(
        &mut self,
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
        feedback: bool,
        specialized: bool,
    ) -> Result<CapabilityOffer, String> {
        if self.pairs.len() == MAXIMUM_SPECIALIZATIONS {
            return Err("native finite zip exceeds its admitted offer count".into());
        }
        let offer = offer(
            left,
            left_type,
            right,
            right_type,
            feedback,
            specialized,
            self.profile,
        )?;
        if self
            .pairs
            .binary_search_by(|(id, _)| id.cmp(&offer.capability_id))
            .is_ok()
        {
            return Err("native finite zip specialization is already installed".into());
        }
        let position = self
            .pairs
            .binary_search_by(|(id, _)| id.cmp(&offer.capability_id))
            .unwrap_err();
        self.pairs.insert(
            position,
            (
                offer.capability_id.clone(),
                OfferedPair {
                    left: left_type.clone(),
                    right: right_type.clone(),
                    offer: offer.clone(),
                    feedback,
                },
            ),
        );
        Ok(offer)
    }

    pub fn offers(&self) -> impl Iterator<Item = &CapabilityOffer> {
        self.pairs.iter().map(|(_, pair)| &pair.offer)
    }

    pub fn validate_plan(&self, plan: &Plan) -> Result<(), String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("native finite zip requires one exact sealed fragment".into());
        }
        for gear in plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.implementation_id == self.implementation)
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
            .binary_search_by(|(id, _)| id.cmp(&gear.capability_id))
            .ok()
            .map(|index| &self.pairs[index].1)
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
        let prepare = if types.feedback {
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
