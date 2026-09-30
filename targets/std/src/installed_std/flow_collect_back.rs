//! Std installation of the shared allocation-prepared `flow/collect` Back.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{BoundedCollectSemanticLaw, PlannedGear, UNIT_INFO_ID};
use conduit_kernel::CanonicalValue;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::FLOW_COLLECT_IMPLEMENTATION,
    budget,
    prepare,
};

fn exact_law(placement: &PlannedGear) -> Result<&BoundedCollectSemanticLaw, String> {
    placement
        .semantic_contract
        .bounded_collect()
        .ok_or_else(|| "flow/collect placement has no exact bounded collection law".into())
}

fn validate(placement: &PlannedGear) -> Result<&BoundedCollectSemanticLaw, String> {
    let law = exact_law(placement)?;
    let expected = conduit_semantic_catalog::flow_collect_semantic_contract(
        &law.element,
        law.maximum_items,
        &law.overflow_disposition,
    )
    .map_err(str::to_string)?;
    let offer = conduit_std_offers::flow_collect_offer(&law.element, law.maximum_items)
        .map_err(str::to_string)?;
    if law.overflow_disposition.value_kind.as_str() != UNIT_INFO_ID
        || law.overflow_disposition.maximum_bytes != 0
        || conduit_core::validate_primitive_info(law.overflow_disposition.value_kind.as_str(), &[])
            .is_err()
        || placement.kind_id.as_str() != conduit_semantic_catalog::FLOW_COLLECT_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::FLOW_COLLECT_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::FLOW_COLLECT_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::FLOW_COLLECT_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::FLOW_COLLECT_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || placement.host_calls != offer.host_calls
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned flow/collect identity differs from its exact specialization".into());
    }
    Ok(law)
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let law = validate(placement)?;
    Ok(BackBudget {
        value_items: law
            .maximum_items
            .checked_add(2)
            .ok_or("flow/collect prepared item budget overflows")?,
        value_bytes: placement.limits.max_queue_bytes,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: law.collection.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let law = validate(placement)?;
    let overflow = CanonicalValue::new(&[])
        .map_err(|error| format!("prepare flow/collect Unit overflow: {error:?}"))?;
    let operation = conduit_data::FlowCollectBack::prepare(
        &law.element,
        law.maximum_items,
        law.collection.maximum_bytes,
        overflow,
    )
    .map_err(|error| format!("prepare flow/collect storage: {error:?}"))?;
    Ok(InstalledBack::FlowCollect(operation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, CheckedValueContract, OfferGeneration};

    fn placement() -> PlannedGear {
        let element = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
        let offer = conduit_std_offers::flow_collect_offer(&element, 4).unwrap();
        conduit_core::planned_gear_from_parts! {
            semantic_contract: offer.semantic_contract,
            placement_id: "collect-placement".into(),
            gear_id: "collect".into(),
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
    fn exact_offer_prepares_the_shared_back_and_finite_budget() {
        let planned = placement();
        let admitted = budget(&planned).unwrap();
        let law = planned.semantic_contract.bounded_collect().unwrap();
        assert_eq!(admitted.value_items, law.maximum_items + 2);
        assert_eq!(admitted.maximum_value_bytes, law.collection.maximum_bytes);
        assert_eq!(admitted.host_requests, 0);
        let mut values = conduit_kernel::HostedValueStore::new(8, 128, 1024).unwrap();
        assert!(matches!(
            prepare(&planned, &mut values),
            Ok(InstalledBack::FlowCollect(_))
        ));
    }

    #[test]
    fn preparation_refuses_identity_law_and_bound_drift() {
        let mut wrong = placement();
        wrong.artifact_id = "foreign".into();
        assert!(budget(&wrong).is_err());

        wrong = placement();
        wrong
            .semantic_contract
            .laws
            .retain(|law| !matches!(law, conduit_core::KindSemanticLaw::BoundedCollect(_)));
        assert!(budget(&wrong).is_err());

        wrong = placement();
        let law = wrong
            .semantic_contract
            .laws
            .iter_mut()
            .find_map(|law| match law {
                conduit_core::KindSemanticLaw::BoundedCollect(law) => Some(law),
                _ => None,
            })
            .unwrap();
        law.maximum_items += 1;
        assert!(budget(&wrong).is_err());

        wrong = placement();
        let law = wrong
            .semantic_contract
            .laws
            .iter_mut()
            .find_map(|law| match law {
                conduit_core::KindSemanticLaw::BoundedCollect(law) => Some(law),
                _ => None,
            })
            .unwrap();
        law.overflow_disposition =
            CheckedValueContract::new(kind_id("value/text"), 8, vec![]).unwrap();
        assert!(budget(&wrong).is_err());
    }
}
