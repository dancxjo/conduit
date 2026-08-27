use conduit_core::{ArtifactId, CapabilityOffer, ExecutionProfileId, ImplementationId};

pub const BROWSER_PRESENTATION_PROFILE: &str = "browser/presentation-nucleus-kernel@1";
pub const BROWSER_PRESENTATION_ARTIFACT: &str = "conduit-browser-runtime/presentation-nucleus@1";

pub fn offers() -> Vec<CapabilityOffer> {
    [
        conduit_std_catalog::LAYOUT_VIEWPORT_KIND,
        conduit_std_catalog::LAYOUT_INSET_KIND,
        conduit_std_catalog::LAYOUT_ROW_KIND,
        conduit_std_catalog::LAYOUT_COLUMN_KIND,
        conduit_std_catalog::LAYOUT_STACK_KIND,
        conduit_std_catalog::LAYOUT_ALIGN_KIND,
        conduit_std_catalog::PRESENTATION_ICON_KIND,
        conduit_std_catalog::PRESENTATION_FRAME_KIND,
        conduit_std_catalog::PRESENTATION_BADGE_KIND,
        conduit_std_catalog::TEXT_PRESENTATION_KIND,
        conduit_std_catalog::GRAPHICS_RECT_KIND,
        conduit_std_catalog::GRAPHICS_TEXT_KIND,
        conduit_std_catalog::GRAPHICS_ICON_KIND,
    ]
    .into_iter()
    .map(|kind| {
        let mut offer = conduit_std_catalog::layout_offer_for(kind)
            .or_else(|| conduit_std_catalog::presentation_composition_offer_for(kind))
            .or_else(|| conduit_std_catalog::graphics_offer_for(kind))
            .or_else(|| {
                (kind == conduit_std_catalog::TEXT_PRESENTATION_KIND)
                    .then(conduit_std_catalog::text_presentation_offer)
            })
            .expect("accepted browser presentation Kind has one canonical offer");
        offer.capability_id =
            conduit_core::CapabilityId::from(format!("browser/{kind}-capability@1").as_str());
        offer.implementation.execution_profile_id =
            ExecutionProfileId::from(BROWSER_PRESENTATION_PROFILE);
        offer.implementation.implementation_id =
            ImplementationId::from(format!("browser/{kind}-implementation@1").as_str());
        offer.implementation.artifact_id = ArtifactId::from(BROWSER_PRESENTATION_ARTIFACT);
        offer
    })
    .collect()
}
