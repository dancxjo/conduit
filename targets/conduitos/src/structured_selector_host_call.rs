//! Native checked selection; unmatched Flow values follow the authored drop law.
use alloc::{format, vec, vec::Vec};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::{LoweredPlanFragment, lower_plan_fragment};
use conduit_plot::maximum_prepared_canonical_value_bytes;

pub const IMPLEMENTATION: &str = "conduitos/kernel-structured-selector@1";
pub const PROFILE: &str = "conduitos/structured-selector-cooperative-bounded@1";
pub const ARTIFACT: &str = "conduit-core/structured-selector@1";
pub const CALL: &str = "conduit.host/structured-selector@1";

pub fn offer(
    selector: &StructuredSelector,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, StructuredInfoRefusal> {
    let contract = conduit_semantic_catalog::structured_selector_contract(selector, temporal);
    let kind = contract.kind_id.clone();
    let bound = |ty| {
        maximum_prepared_canonical_value_bytes(ty)
            .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)
    };
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("conduitos/{}", kind.as_str())),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(CALL),
                target_kind: Some(kind),
                maximum_in_flight: 1,
                maximum_input_bytes: bound(selector.input_type())?,
                maximum_output_bytes: bound(selector.output_type())?,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

#[derive(Debug, PartialEq, Eq)]
pub enum SelectorCallRefusal {
    WrongBinding,
    InvalidSelector,
    InvalidInput,
    StaleRequest,
    SequenceExhausted,
    Cancelled,
    Selection(StructuredSelectorRefusal),
}
pub struct SelectorHostCall {
    selector: StructuredSelector,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
    output: Vec<u8>,
    node: NodeId,
    next_request: u32,
    maximum_input_bytes: u32,
    cancelled: bool,
}
impl SelectorHostCall {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
        placement: &PlacementId,
    ) -> Result<Self, SelectorCallRefusal> {
        use SelectorCallRefusal as Refusal;
        if !verify_plan_fragment(fragment)
            || active.plan_id != fragment.plan_id
            || active.host_id != fragment.host_id
            || active.boot_id != fragment.boot_id
            || bind_active_play(
                &fragment.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                active.play_sequence,
            ) != *active
            || lower_plan_fragment(fragment).map_err(|_| Refusal::WrongBinding)? != *lowered
        {
            return Err(Refusal::WrongBinding);
        }
        let mut gears = fragment
            .placements
            .iter()
            .filter(|gear| &gear.placement_id == placement);
        let gear = gears.next().ok_or(Refusal::WrongBinding)?;
        if gears.next().is_some()
            || gear.host_id != fragment.host_id
            || gear.boot_id != fragment.boot_id
        {
            return Err(Refusal::WrongBinding);
        }
        let selector = selected_selector(gear)?;
        let mut nodes = lowered
            .identity
            .placements
            .iter()
            .filter(|(_, id)| id == placement);
        let node = nodes.next().ok_or(Refusal::WrongBinding)?.0;
        if nodes.next().is_some() {
            return Err(Refusal::WrongBinding);
        }
        let bound =
            |ty| maximum_prepared_canonical_value_bytes(ty).map_err(|_| Refusal::InvalidSelector);
        let maximum_input_bytes = bound(selector.input_type())?;
        let output = Vec::with_capacity(bound(selector.output_type())? as usize);
        Ok(Self {
            input_type: selector
                .input_type()
                .canonical_bytes()
                .map_err(|_| Refusal::InvalidSelector)?,
            output_type: selector
                .output_type()
                .canonical_bytes()
                .map_err(|_| Refusal::InvalidSelector)?,
            selector,
            output,
            node,
            next_request: 0,
            maximum_input_bytes,
            cancelled: false,
        })
    }
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<Option<&[u8]>, SelectorCallRefusal> {
        use SelectorCallRefusal as Refusal;
        self.check_binding(node, call)?;
        if self.cancelled {
            return Err(Refusal::Cancelled);
        }
        if request != RequestId(self.next_request) {
            return Err(Refusal::StaleRequest);
        }
        if input.len() > self.maximum_input_bytes as usize {
            return Err(Refusal::InvalidInput);
        }
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(Refusal::SequenceExhausted)?;
        match self
            .selector
            .select_canonical_into(input, &self.input_type, &self.output_type, &mut self.output)
            .map_err(Refusal::Selection)?
        {
            StructuredCanonicalSelection::Matched => Ok(Some(&self.output)),
            StructuredCanonicalSelection::Unmatched(UnmatchedVariantDisposition::Drop) => Ok(None),
            StructuredCanonicalSelection::Unmatched(UnmatchedVariantDisposition::Refuse) => Err(
                Refusal::Selection(StructuredSelectorRefusal::UnmatchedVariant),
            ),
        }
    }
    pub fn cancel(&mut self, node: NodeId, call: HostCallId) -> Result<(), SelectorCallRefusal> {
        self.check_binding(node, call)?;
        self.cancelled = true;
        Ok(())
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), SelectorCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            Err(SelectorCallRefusal::WrongBinding)
        } else {
            Ok(())
        }
    }
}
fn selected_selector(gear: &PlannedGear) -> Result<StructuredSelector, SelectorCallRefusal> {
    use SelectorCallRefusal as Refusal;
    let [entry] = gear.configuration.as_slice() else {
        return Err(Refusal::InvalidSelector);
    };
    let ("selector", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err(Refusal::InvalidSelector);
    };
    let selector =
        StructuredSelector::from_canonical_hex(encoded).map_err(|_| Refusal::InvalidSelector)?;
    let temporal = gear.inputs.first().ok_or(Refusal::WrongBinding)?.temporal;
    let expected = offer(&selector, temporal).map_err(|_| Refusal::InvalidSelector)?;
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.artifact_id != expected.implementation.artifact_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.host_calls != expected.host_calls
        || gear.limits != expected.limits
        || gear.semantic_contract != expected.semantic_contract
        || gear.base.is_some()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
    {
        return Err(Refusal::WrongBinding);
    }
    Ok(selector)
}
mod factory;
pub use factory::SelectorOperationFactory;
