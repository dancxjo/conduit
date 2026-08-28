//! Hosted std direct-Patchbay Presenter offers.

use conduit_core::{
    kind_id, present_host_operation_requirement, resource_requirement, CapabilityOffer,
    PRESENTATION_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

pub fn patchbay_presentation_offers() -> [CapabilityOffer; 4] {
    conduit_semantic_catalog::patchbay_presentation_contracts().map(|contract| {
        let kind = contract.kind_id.as_str().to_owned();
        let capability = format!("patchbay/{kind}-direct@1");
        let implementation = format!("patchbay/direct/{}@1", kind.replace('/', "-"));
        realization_offer(
            contract,
            conduit_semantic_catalog::PATCHBAY_PRESENTATION_REVISION,
            RealizationOfferIdentity {
                capability: &capability,
                execution_profile: "patchbay/presenter-kernel-hosted@1",
                implementation: &implementation,
                artifact: "patchbay-model/direct-presentation@1",
            },
            vec![present_host_operation_requirement(
                kind_id("presentation/patchbay-surface@1"),
                conduit_semantic_catalog::MAX_PATCHBAY_PRESENTATION_BYTES,
            )],
            vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
            vec![],
        )
    })
}
