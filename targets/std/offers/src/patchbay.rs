//! Hosted std direct-Patchbay Presenter offers.

use conduit_core::{
    kind_id, present_host_call_requirement, resource_requirement, CapabilityOffer,
    FACE_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

pub fn patchbay_face_offers() -> [CapabilityOffer; 4] {
    conduit_semantic_catalog::patchbay_face_contracts().map(|contract| {
        let kind = contract.kind_id.as_str().to_owned();
        let capability = format!("patchbay/{kind}-direct@1");
        let implementation = format!("patchbay/direct/{}@1", kind.replace('/', "-"));
        realization_offer(
            contract,
            conduit_semantic_catalog::PATCHBAY_FACE_REVISION,
            RealizationOfferIdentity {
                capability: &capability,
                execution_profile: "patchbay/presenter-kernel-hosted@1",
                implementation: &implementation,
                artifact: "conduit-patchbay-workbench/direct-face@1",
            },
            vec![present_host_call_requirement(
                kind_id("face/patchbay-surface@1"),
                conduit_semantic_catalog::MAX_PATCHBAY_FACE_BYTES,
            )],
            vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
            vec![],
        )
    })
}
