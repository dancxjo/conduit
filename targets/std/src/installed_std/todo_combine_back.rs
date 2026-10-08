//! Production std selection of the exact bounded Todo combine Back.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TODO_COMBINE_IMPLEMENTATION,
    budget,
    prepare,
};

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let kind = conduit_todo_plot::todo_combine_kind();
    if placement.kind_id != kind.kind_id
        || placement.kind_contract_revision != kind.kind_contract_revision
        || placement.execution_profile_id.as_str() != conduit_std_offers::TODO_COMBINE_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::TODO_COMBINE_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::TODO_COMBINE_ARTIFACT
        || placement.inputs != kind.inputs
        || placement.outputs != kind.outputs
        || placement.semantic_contract != kind.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned Todo combine differs from its exact bounded Back".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: (2 * conduit_todo_plot::STATE_MAX_BYTES + conduit_todo_plot::COMMAND_MAX_BYTES)
            as u32,
        maximum_value_bytes: conduit_todo_plot::STATE_MAX_BYTES as u32,
        host_requests: 0,
        sign_items: 3,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::TodoCombine(
        conduit_todo_plot::TodoCombineBack::new(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::OfferGeneration;

    fn placement() -> PlannedGear {
        let offer = conduit_std_offers::todo_combine_offer();
        conduit_core::planned_gear_from_parts! {
            semantic_contract: offer.semantic_contract,
            placement_id: "todo-combine-placement".into(),
            gear_id: "todo-combine".into(),
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![],
            host_id: "std-host".into(),
            boot_id: "std-boot".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offer.capability_id,
            implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id,
            base: None,
            realization_characteristics: vec![],
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            terminal_transductions: vec![],
            host_calls: offer.host_calls,
            resources: vec![],
            authority: vec![],
            pool_references: vec![],
        }
    }

    #[test]
    fn production_catalog_prepares_exact_todo_combine() {
        let planned = placement();
        assert!(super::super::catalog::factory(&planned.implementation_id).is_some());
        let admitted = budget(&planned).unwrap();
        assert_eq!(
            admitted.maximum_value_bytes,
            conduit_todo_plot::STATE_MAX_BYTES as u32
        );
        assert_eq!(admitted.host_requests, 0);
        let mut values = conduit_kernel::HostedValueStore::new(
            4,
            admitted.maximum_value_bytes,
            admitted.value_bytes,
        )
        .unwrap();
        assert!(matches!(
            prepare(&planned, &mut values),
            Ok(InstalledBack::TodoCombine(_))
        ));
    }

    #[test]
    fn production_preparation_refuses_identity_drift() {
        let mut planned = placement();
        planned.kind_contract_revision = "todo/combine/foreign@1".into();
        assert!(budget(&planned).is_err());
    }
}
