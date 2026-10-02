//! Dynamic browser realization of exact bounded Flow collection.

use super::factory::BrowserInstallation;
use super::{BrowserBack, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    kind_id, validate_primitive_info, ArtifactId, Back, BackOfferBuilder,
    BoundedCollectSemanticLaw, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, ImplementationId, PlannedGear, UNIT_INFO_ID,
};
use conduit_kernel::CanonicalValue;

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-flow-collect@1";
const PROFILE: &str = "browser/flow-collect-prepared@1";
const ARTIFACT: &str = "conduit-browser-runtime/flow-collect@1";
const MAXIMUM_ITEMS: u16 = 256;

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: None,
};

pub(crate) fn offer_for_expanded(
    gear: &conduit_plot::CheckedGear,
) -> Result<CapabilityOffer, String> {
    let law = gear
        .semantic_contract
        .bounded_collect()
        .ok_or("expanded flow/collect Gear has no exact bounded collection law")?;
    offer(&law.element, law.maximum_items)
}

fn offer(element: &CheckedValueContract, maximum_items: u16) -> Result<CapabilityOffer, String> {
    let overflow = CheckedValueContract::new(kind_id(UNIT_INFO_ID), 0, Vec::new())
        .map_err(|_| "browser flow/collect Unit overflow contract is invalid")?;
    let contract =
        conduit_semantic_catalog::flow_collect_semantic_contract(element, maximum_items, &overflow)
            .map_err(str::to_string)?;
    let law = contract
        .bounded_collect()
        .ok_or("browser flow/collect contract lost its exact law")?;
    if maximum_items > MAXIMUM_ITEMS
        || element.maximum_bytes > MAXIMUM_BROWSER_VALUE_BYTES as u32
        || law.collection.maximum_bytes > MAXIMUM_BROWSER_VALUE_BYTES as u32
    {
        return Err("browser flow/collect specialization exceeds its prepared bounds".into());
    }
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "browser/flow-collect-{}-{}-{}",
                element.value_kind.as_str(),
                element.maximum_bytes,
                maximum_items
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

fn exact_law(placement: &PlannedGear) -> Result<&BoundedCollectSemanticLaw, String> {
    placement
        .semantic_contract
        .bounded_collect()
        .ok_or_else(|| "browser flow/collect placement has no exact bounded collection law".into())
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    let law = exact_law(placement)?;
    let exact = offer(&law.element, law.maximum_items)?;
    if law.overflow_disposition.value_kind.as_str() != UNIT_INFO_ID
        || law.overflow_disposition.maximum_bytes != 0
        || validate_primitive_info(law.overflow_disposition.value_kind.as_str(), &[]).is_err()
        || placement.kind_id != exact.kind_id
        || placement.kind_contract_revision != exact.kind_contract_revision
        || placement.capability_id != exact.capability_id
        || placement.execution_profile_id != exact.implementation.execution_profile_id
        || placement.implementation_id != exact.implementation.implementation_id
        || placement.artifact_id != exact.implementation.artifact_id
        || placement.inputs != exact.inputs
        || placement.outputs != exact.outputs
        || placement.semantic_contract != exact.semantic_contract
        || placement.limits != exact.limits
        || placement.host_calls != exact.host_calls
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned flow/collect differs from its exact browser realization".into());
    }
    let overflow = CanonicalValue::new(&[])
        .map_err(|error| format!("prepare browser flow/collect Unit overflow: {error:?}"))?;
    let operation = conduit_data::FlowCollectBack::prepare(
        &law.element,
        law.maximum_items,
        law.collection.maximum_bytes,
        overflow,
    )
    .map_err(|error| format!("prepare browser flow/collect storage: {error:?}"))?;
    Ok(BrowserBack::installed_step(operation))
}

fn unreachable_offer() -> CapabilityOffer {
    panic!("dynamic flow/collect offers must be derived from checked expansion")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{KindSemanticLaw, OfferGeneration};

    fn placement() -> PlannedGear {
        let element = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
        let offered = offer(&element, 4).unwrap();
        conduit_core::planned_gear_from_parts! {
            semantic_contract: offered.semantic_contract,
            placement_id: "browser-collect-placement".into(),
            gear_id: "collect".into(),
            kind_id: offered.kind_id,
            kind_contract_revision: offered.kind_contract_revision,
            execution_profile_id: offered.implementation.execution_profile_id,
            configuration: vec![],
            host_id: "browser/collect".into(),
            boot_id: "browser-collect-boot".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offered.capability_id,
            implementation_id: offered.implementation.implementation_id,
            artifact_id: offered.implementation.artifact_id,
            base: None,
            realization_characteristics: vec![],
            limits: offered.limits,
            inputs: offered.inputs,
            outputs: offered.outputs,
            terminal_transductions: vec![],
            host_calls: offered.host_calls,
            resources: vec![],
            authority: vec![],
            pool_references: vec![],
        }
    }

    #[test]
    fn exact_unit_overflow_offer_prepares_the_shared_back() {
        let planned = placement();
        let law = exact_law(&planned).unwrap();
        assert_eq!(law.overflow_disposition.value_kind.as_str(), UNIT_INFO_ID);
        let mut values = conduit_kernel::HostedValueStore::new(8, 128, 1024).unwrap();
        assert!(prepare(&planned, &mut values).is_ok());
    }

    #[test]
    fn preparation_refuses_identity_law_bound_and_overflow_drift() {
        let mut wrong = placement();
        wrong.artifact_id = "foreign".into();
        let mut values = conduit_kernel::HostedValueStore::new(8, 128, 1024).unwrap();
        assert!(prepare(&wrong, &mut values).is_err());

        wrong = placement();
        let law = wrong
            .semantic_contract
            .laws
            .iter_mut()
            .find_map(|law| match law {
                KindSemanticLaw::BoundedCollect(law) => Some(law),
                _ => None,
            })
            .unwrap();
        law.maximum_items += 1;
        assert!(prepare(&wrong, &mut values).is_err());

        wrong = placement();
        let law = wrong
            .semantic_contract
            .laws
            .iter_mut()
            .find_map(|law| match law {
                KindSemanticLaw::BoundedCollect(law) => Some(law),
                _ => None,
            })
            .unwrap();
        law.overflow_disposition =
            CheckedValueContract::new(kind_id("value/text"), 8, vec![]).unwrap();
        assert!(prepare(&wrong, &mut values).is_err());
    }
}
