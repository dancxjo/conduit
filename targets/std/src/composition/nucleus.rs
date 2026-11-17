//! Exact portable-nucleus inventory derived from the existing reference composition.
use super::{reference_advertisement, StdHostConfig};

/// The exact offers installed for the portable supported-nucleus contracts by
/// the broad hosted reference composition.
pub fn supported_nucleus_offers() -> Vec<conduit_core::CapabilityOffer> {
    let advertisement = reference_advertisement(StdHostConfig {
        host_id: conduit_core::HostId::from("std-inventory"),
        boot_id: conduit_core::BootId::from("std-inventory/boot"),
        offer_generation: conduit_core::OfferGeneration(1),
    });

    let contracts = conduit_semantic_catalog::supported_nucleus_contracts();
    let missing = contracts
        .iter()
        .filter(|contract| {
            !advertisement.capabilities.iter().any(|offer| {
                offer.kind_id == contract.kind_id
                    && offer.inputs == contract.inputs
                    && offer.outputs == contract.outputs
                    && offer.limits == contract.limits
            })
        })
        .map(|contract| contract.kind_id.as_str())
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "std reference composition does not install exact contracts: {missing:?}"
    );

    contracts
        .into_iter()
        .map(|contract| {
            advertisement
                .capabilities
                .iter()
                .find(|offer| {
                    offer.kind_id == contract.kind_id
                        && offer.inputs == contract.inputs
                        && offer.outputs == contract.outputs
                        && offer.limits == contract.limits
                })
                .expect("missing contracts were checked above")
                .clone()
        })
        .collect()
}
