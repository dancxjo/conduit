//! Exact installed Crèche factory. This selects a finite Back only; it neither
//! advertises an encounter nor manufactures Host authority or platform effects.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_body_make::{deployment_runtime::*, deployment_runtime_catalog::*};
use conduit_core::PlannedGear;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: CRECHE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let kind = creche_kind();
    let offer = creche_std_offer();
    if placement.capability_id != offer.capability_id
        || placement.kind_id != kind.kind_id
        || placement.kind_contract_revision != kind.kind_contract_revision
        || placement.execution_profile_id.as_str() != CRECHE_PROFILE
        || placement.implementation_id.as_str() != CRECHE_IMPLEMENTATION
        || placement.artifact_id.as_str() != CRECHE_ARTIFACT
        || placement.inputs != kind.inputs
        || placement.outputs != kind.outputs
        || placement.semantic_contract != kind.semantic_contract()
        || placement.limits != offer.limits
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.configuration.is_empty()
        || !placement.terminal_transductions.is_empty()
        || !placement.realization_properties.is_empty()
        || placement.host_calls != vec![creche_host_call_requirement()]
        || placement.resources.len() != 1
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned Creche differs from its exact installed encounter contract".into());
    }
    // Structural selection only. The installed Host owner must additionally
    // prove this exact pool belongs to its admitted encounter before effects.
    let resource = &placement.resources[0];
    if resource.class_id.as_str() != CRECHE_RESOURCE
        || resource.pool_id.as_str().trim().is_empty()
        || resource.units != 1
        || resource.protected.is_some()
        || resource.compute.is_some()
        || resource.content.is_some()
    {
        return Err("planned Creche requires one exact admitted encounter resource slot".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (CRECHE_REQUEST_BYTES + CRECHE_OUTCOME_BYTES) as u32,
        maximum_value_bytes: CRECHE_OUTCOME_BYTES as u32,
        host_requests: 1,
        sign_items: 32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    // Actual Body/Plan/Play binding is validated by the selected Host owner.
    // It cannot be invented by a factory that only has PlannedGear.
    Ok(InstalledBack::Creche(CrecheBack::for_admitted_host(
        conduit_kernel::HostCallId(0),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::*;
    fn placement() -> PlannedGear {
        let offer = creche_std_offer();
        PlannedGear {
            placement_id: "placement/creche".into(),
            gear_id: "gear/creche".into(),
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            source_span: None,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![],
            host_id: "host/test".into(),
            boot_id: "boot/test".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offer.capability_id,
            implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id,
            base: None,
            realization_characteristics: vec![],
            realization_properties: vec![],
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            semantic_contract: offer.semantic_contract,
            terminal_transductions: vec![],
            host_calls: offer.host_calls,
            resources: vec![ResourceBinding {
                pool_id: "encounter/test".into(),
                class_id: CRECHE_RESOURCE.into(),
                units: 1,
                protected: None,
                compute: None,
                content: None,
            }],
            authority: vec![],
            pool_references: vec![],
        }
    }
    #[test]
    fn exact_installed_factory_retains_finite_call_and_value_budgets() {
        let placement = placement();
        let factory = super::super::catalog::factory(&placement.implementation_id).unwrap();
        let budget = (factory.budget)(&placement).unwrap();
        assert_eq!(
            (
                budget.host_requests,
                budget.value_items,
                budget.value_bytes,
                budget.maximum_value_bytes
            ),
            (1, 2, 97, 65)
        );
        let mut store = conduit_kernel::HostedValueStore::new(2, 65, 97).unwrap();
        assert!(matches!(
            (factory.prepare)(&placement, &mut store).unwrap(),
            InstalledBack::Creche(_)
        ));
    }
    #[test]
    fn missing_resource_and_changed_call_cannot_select_installed_back() {
        let mut missing = placement();
        missing.resources.clear();
        assert!(validate(&missing).is_err());
        let mut wrong_call = placement();
        wrong_call.host_calls[0].maximum_output_bytes += 1;
        assert!(validate(&wrong_call).is_err());
        let mut wrong_slot = placement();
        wrong_slot.resources[0].class_id = "other/resource".into();
        assert!(validate(&wrong_slot).is_err());
    }
}
