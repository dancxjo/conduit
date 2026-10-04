//! Exact native binding of the shared prepared current-value sampler.
use alloc::{boxed::Box, format, string::String, vec::Vec};
use conduit_composite::{CurrentSampleBack, KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub const IMPLEMENTATION: &str = "conduitos/current-sample-finite@1";
pub const PROFILE: &str = "conduitos/current-sample-finite-prepared@1";
pub const ARTIFACT: &str = "conduit-composite/current-sample@1";
pub const MAXIMUM_BYTES: u32 = 4096;

pub fn offer(
    value: &CheckedValueContract,
    trigger: &CheckedValueContract,
) -> Result<CapabilityOffer, &'static str> {
    if value.maximum_bytes > MAXIMUM_BYTES || trigger.maximum_bytes > MAXIMUM_BYTES {
        return Err("native current/sample envelope exceeds its admitted profile");
    }
    let kind = conduit_semantic_catalog::current_sample_finite_semantic_contract(value, trigger)?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!(
                "conduitos/current-sample-finite/{}/{}/{}/{}@1",
                value.value_kind.as_str(),
                value.maximum_bytes,
                trigger.value_kind.as_str(),
                trigger.maximum_bytes,
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

fn validate(gear: &PlannedGear) -> Result<u32, String> {
    let contracts = gear.semantic_contract.value_contracts();
    let at = |name| {
        contracts
            .iter()
            .find(|entry| entry.location == FrontValueLocation::Input(port_id(name)))
            .map(|entry| &entry.contract)
    };
    let value = at("current").ok_or("native sampler has no current contract")?;
    let trigger = at("trigger").ok_or("native sampler has no trigger contract")?;
    let expected = offer(value, trigger).map_err(String::from)?;
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
        return Err("native sampler differs from its exact selected realization".into());
    }
    Ok(value.maximum_bytes)
}

pub struct CurrentSampleOperationFactory {
    implementation: ImplementationId,
}
impl Default for CurrentSampleOperationFactory {
    fn default() -> Self {
        Self {
            implementation: ImplementationId::from(IMPLEMENTATION),
        }
    }
}
impl KernelOperationFactory for CurrentSampleOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let maximum = validate(gear)?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: maximum,
            maximum_value_bytes: maximum,
            host_requests: 0,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        Ok(Box::new(CurrentSampleBack::prepare_finite(validate(gear)?)))
    }
}
