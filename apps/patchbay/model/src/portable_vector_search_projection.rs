//! Vector-search-specific disclosure over exact generic Plan facts.

use conduit_core::PlannedGear;
use conduit_presentation::PresentationPropertyValue;

use crate::portable_projection::ContentBuilder;
use crate::PatchbayGear;

pub(super) fn append_vector_search_realization(
    content: &mut ContentBuilder,
    subject: &str,
    gear: &PatchbayGear,
    placement: &PlannedGear,
) {
    if gear.kind_id.as_str() != conduit_ai::VECTOR_SEARCH_KIND {
        return;
    }

    let proof_class =
        if placement.implementation_id.as_str() == conduit_ai::EXACT_VECTOR_SEARCH_IMPLEMENTATION {
            Some("deterministic-exact")
        } else if placement.implementation_id.as_str()
            == conduit_std_host::hosted_vector_index::HOSTED_HNSW_IMPLEMENTATION_ID
        {
            Some("approximate")
        } else {
            None
        };
    if let Some(proof_class) = proof_class {
        content.property(
            subject,
            "vector-search-proof-class",
            PresentationPropertyValue::Text(proof_class.into()),
        );
    }

    for resource in placement
        .resources
        .iter()
        .filter(|resource| resource.class_id.as_str() == conduit_ai::VECTOR_INDEX_RESOURCE_CLASS)
    {
        content.property(
            subject,
            "vector-index-resource",
            PresentationPropertyValue::Text(format!(
                "pool={} class={} units={}",
                resource.pool_id.as_str(),
                resource.class_id.as_str(),
                resource.units
            )),
        );
    }
}
