//! Exact pure bytes/literal factory; the existing emitter treats ValueRef opaquely.
use crate::text_kernel_backs::{LiteralBack, LiteralState};
use alloc::{boxed::Box, format, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{CapabilityOffer, ConfigurationValue, ImplementationId, PlannedGear};
use conduit_kernel::{HostedValueStore, ValueStorage, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub const IMPLEMENTATION: &str = "conduitos/kernel-bytes-literal@1";
pub struct BytesLiteralOperationFactory {
    offer: CapabilityOffer,
}
impl Default for BytesLiteralOperationFactory {
    fn default() -> Self {
        Self { offer: offer() }
    }
}
impl BytesLiteralOperationFactory {
    /// The existing pure offer, without manufacturing Host Calls or Base authority.
    pub fn offer(&self) -> &CapabilityOffer {
        &self.offer
    }
    fn selected<'a>(&self, gear: &'a PlannedGear) -> Result<&'a str, String> {
        let expected = &self.offer;
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
            || !gear.realization_properties.is_empty()
            || !gear.terminal_transductions.is_empty()
            || !gear.host_calls.is_empty()
            || gear.base.is_some()
            || !gear.resources.is_empty()
            || !gear.authority.is_empty()
            || !gear.pool_references.is_empty()
        {
            return Err("native byte literal differs from its exact pure realization".into());
        }
        let [entry] = gear.configuration.as_slice() else {
            return Err("native byte literal requires exactly one hex configuration".into());
        };
        let ConfigurationValue::Text(value) = &entry.value else {
            return Err("native byte literal hex must be Text".into());
        };
        if entry.key != "hex" {
            return Err("native byte literal configuration requires hex".into());
        }
        conduit_data::bytes_literal::validate_hex(value)
            .map_err(|error| format!("byte literal: {error:?}"))?;
        Ok(value)
    }
}
impl KernelOperationFactory for BytesLiteralOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.offer.implementation.implementation_id
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let bytes = (self.selected(gear)?.len() as u32 / 2).max(1);
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: bytes,
            maximum_value_bytes: bytes,
            host_requests: 0,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        let decoded = conduit_data::bytes_literal::decode_hex(self.selected(gear)?)
            .map_err(|error| format!("byte literal: {error:?}"))?;
        let text = values
            .store(&decoded)
            .map_err(|error| format!("{error:?}"))?;
        Ok(Box::new(LiteralBack {
            text,
            state: LiteralState::Emitting,
        }))
    }
}

pub fn offer() -> CapabilityOffer {
    use conduit_core::{ArtifactId, Back, BackOfferBuilder, CapabilityId, ExecutionProfileId};
    BackOfferBuilder::new(
        conduit_data::bytes_literal::definition(),
        Back {
            capability_id: CapabilityId::from("conduitos-bytes-literal-v1"),
            execution_profile_id: ExecutionProfileId::from(
                crate::functional_offers::FUNCTIONAL_KERNEL_PROFILE,
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(crate::functional_offers::FUNCTIONAL_KERNEL_ARTIFACT),
            host_calls: alloc::vec![],
            resource_requirements: alloc::vec![],
            authority_requirements: alloc::vec![],
        },
    )
    .build()
}
#[cfg(test)]
mod tests;
