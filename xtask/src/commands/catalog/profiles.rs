use super::{CatalogError, CatalogHost};
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use conduitos::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
};

pub(super) fn advertisement(host: CatalogHost) -> Result<HostAdvertisement, CatalogError> {
    match host {
        CatalogHost::Std => Ok(StdHost::new_with_composition(
            StdHostConfig {
                host_id: HostId::from("catalog-std-reference"),
                boot_id: BootId::from("catalog-static-not-a-boot"),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::reference(),
        )
        .advertisement()
        .clone()),
        CatalogHost::Browser => browser_advertisement(),
        CatalogHost::Pico => Ok(conduit_signal::pico_local_advertisement()),
        CatalogHost::Conduitos => conduitos_advertisement(),
        CatalogHost::PatchbayConstrained => patchbay_model::patchbay_presenter_plans()
            .map(|proof| proof.recursive_host)
            .map_err(|error| CatalogError::new("patchbay-recursive-profile-invalid", error)),
    }
}

fn browser_advertisement() -> Result<HostAdvertisement, CatalogError> {
    let mut browser = conduit_signal::distributed_browser_sink_advertisement();
    browser
        .capabilities
        .extend(conduit_std_catalog::browser_presentation_nucleus_offers());
    let proof = patchbay_model::patchbay_presenter_plans()
        .map_err(|error| CatalogError::new("patchbay-direct-profile-invalid", error))?;
    browser.capabilities.extend(
        proof
            .direct_host
            .capabilities
            .into_iter()
            .filter(|offer| offer.kind_id.as_str() == patchbay_model::PATCHBAY_PRESENTATION_KIND),
    );
    browser.resources.extend(proof.direct_host.resources);
    browser.resources.sort();
    browser
        .resources
        .dedup_by(|left, right| left.pool_id == right.pool_id);
    Ok(browser)
}

fn conduitos_advertisement() -> Result<HostAdvertisement, CatalogError> {
    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &ids,
        "catalog-static-artifact",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    offer
        .validate()
        .map_err(|error| CatalogError::new("conduitos-offer-invalid", error.as_str()))?;
    let mut advertisement = HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: HostId::from("catalog-conduitos-reference"),
        boot_id: BootId::from("catalog-static-not-a-boot"),
        offer_generation: OfferGeneration(offer.generation),
        profile: conduit_core::HostProfileId::from(offer.profile),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities: offer
            .capabilities
            .iter()
            .map(|capability| {
                let mut exact = conduit_std_catalog::supported_nucleus_offers()
                    .into_iter()
                    .find(|candidate| {
                        candidate.kind_id.as_str() == capability.kind
                            && candidate.kind_contract_revision.as_str()
                                == capability.contract_revision
                    })
                    .ok_or_else(|| {
                        CatalogError::new("conduitos-capability-not-in-catalog", capability.kind)
                    })?;
                exact.capability_id = conduit_core::CapabilityId::from(capability.implementation);
                exact.implementation.implementation_id =
                    conduit_core::ImplementationId::from(capability.implementation);
                exact.implementation.artifact_id =
                    conduit_core::ArtifactId::from(capability.artifact_build);
                Ok(exact)
            })
            .collect::<Result<Vec<_>, CatalogError>>()?,
    };
    let presentation = conduit_std_catalog::conduitos_presentation_nucleus_offers();
    advertisement.capabilities.retain(|capability| {
        !presentation
            .iter()
            .any(|offer| offer.kind_id == capability.kind_id)
    });
    advertisement.capabilities.extend(presentation);
    advertisement
        .capabilities
        .push(conduit_std_catalog::conduitos_logic_not_offer());
    Ok(advertisement)
}
