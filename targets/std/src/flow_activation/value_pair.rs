//! Prepared schema custody for the generic one-shot Value pair child Back.
use conduit_composite::{KernelOperationBudget, KernelOperationFactory, ValuePairBack};
use conduit_core::{
    CapabilityOffer, CheckedValueContract, FrontValueLocation, ImplementationId, PlannedGear,
    StructuredInfoType,
};
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

/// One exact semantic pair specialization per child registry. All preparation
/// is explicit; the Back never discovers a schema or opens referenced content.
pub struct PreparedValuePairFactory {
    offer: CapabilityOffer,
    left: StructuredInfoType,
    right: StructuredInfoType,
    maximum_left: u32,
    maximum_right: u32,
    maximum_output: u32,
}
impl PreparedValuePairFactory {
    pub fn new(
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
    ) -> Result<Self, String> {
        let offer = conduit_std_offers::value_pair_offer(left, left_type, right, right_type)
            .map_err(str::to_string)?;
        let maximum_output = offer
            .semantic_contract
            .value_contracts()
            .iter()
            .find(|v| matches!(&v.location,FrontValueLocation::Output(id) if id.as_str()=="paired"))
            .ok_or("Value pair output contract missing")?
            .contract
            .maximum_bytes;
        Ok(Self {
            offer,
            left: left_type.clone(),
            right: right_type.clone(),
            maximum_left: left.maximum_bytes,
            maximum_right: right.maximum_bytes,
            maximum_output,
        })
    }
    pub fn offer(&self) -> &CapabilityOffer {
        &self.offer
    }
    fn validate(&self, p: &PlannedGear) -> Result<(), String> {
        let expected = &self.offer;
        if p.kind_id != expected.kind_id
            || p.kind_contract_revision != expected.kind_contract_revision
            || p.capability_id != expected.capability_id
            || p.execution_profile_id != expected.implementation.execution_profile_id
            || p.implementation_id != expected.implementation.implementation_id
            || p.artifact_id != expected.implementation.artifact_id
            || p.inputs != expected.inputs
            || p.outputs != expected.outputs
            || p.semantic_contract != expected.semantic_contract
            || p.limits != expected.limits
            || !p.configuration.is_empty()
            || p.base.is_some()
            || !p.host_calls.is_empty()
            || !p.resources.is_empty()
            || !p.authority.is_empty()
            || !p.pool_references.is_empty()
            || !p.realization_characteristics.is_empty()
            || !p.realization_properties.is_empty()
            || !p.terminal_transductions.is_empty()
        {
            return Err(
                "Value pair placement differs from its retained exact schema profile".into(),
            );
        }
        Ok(())
    }
}
impl KernelOperationFactory for PreparedValuePairFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.offer.implementation.implementation_id
    }
    fn budget(&self, p: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.validate(p)?;
        Ok(KernelOperationBudget {
            value_items: 3,
            value_bytes: self
                .maximum_left
                .checked_add(self.maximum_right)
                .and_then(|n| n.checked_add(self.maximum_output))
                .ok_or("Value pair budget overflow")?,
            maximum_value_bytes: self
                .maximum_output
                .max(self.maximum_left)
                .max(self.maximum_right),
            host_requests: 0,
            sign_items: 4,
        })
    }
    fn prepare(
        &self,
        p: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        self.validate(p)?;
        Ok(Box::new(
            ValuePairBack::prepare(
                self.left.clone(),
                self.maximum_left,
                self.right.clone(),
                self.maximum_right,
            )
            .map_err(|e| format!("prepare Value pair: {e:?}"))?,
        ))
    }
}
