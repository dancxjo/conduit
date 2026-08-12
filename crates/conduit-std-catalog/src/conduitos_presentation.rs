use alloc::{format, vec::Vec};
use conduit_core::{ArtifactId, CapabilityOffer, ExecutionProfileId, ImplementationId};

use crate::{
    graphics_offer_for, graphics_presentation_offer, layout_offer_for,
    presentation_composition_offer_for, text_presentation_offer, GRAPHICS_ICON_KIND,
    GRAPHICS_PRESENTATION_KIND, GRAPHICS_RECT_KIND, GRAPHICS_TEXT_KIND, LAYOUT_ALIGN_KIND,
    LAYOUT_COLUMN_KIND, LAYOUT_INSET_KIND, LAYOUT_ROW_KIND, LAYOUT_STACK_KIND,
    LAYOUT_VIEWPORT_KIND, PRESENTATION_BADGE_KIND, PRESENTATION_FRAME_KIND, PRESENTATION_ICON_KIND,
    TEXT_PRESENTATION_KIND,
};

pub const CONDUITOS_PRESENTATION_PROFILE: &str = "conduitos/framebuffer-presentation-kernel@1";
pub const CONDUITOS_PRESENTATION_ARTIFACT: &str = "conduitos/framebuffer-presentation@1";

pub fn conduitos_presentation_nucleus_offers() -> Vec<CapabilityOffer> {
    [
        LAYOUT_VIEWPORT_KIND,
        LAYOUT_INSET_KIND,
        LAYOUT_ROW_KIND,
        LAYOUT_COLUMN_KIND,
        LAYOUT_STACK_KIND,
        LAYOUT_ALIGN_KIND,
        PRESENTATION_ICON_KIND,
        PRESENTATION_FRAME_KIND,
        PRESENTATION_BADGE_KIND,
        TEXT_PRESENTATION_KIND,
        GRAPHICS_RECT_KIND,
        GRAPHICS_TEXT_KIND,
        GRAPHICS_ICON_KIND,
        GRAPHICS_PRESENTATION_KIND,
    ]
    .into_iter()
    .map(|kind| {
        let mut offer = layout_offer_for(kind)
            .or_else(|| presentation_composition_offer_for(kind))
            .or_else(|| graphics_offer_for(kind))
            .or_else(|| (kind == GRAPHICS_PRESENTATION_KIND).then(graphics_presentation_offer))
            .or_else(|| (kind == TEXT_PRESENTATION_KIND).then(text_presentation_offer))
            .expect("accepted ConduitOS presentation Kind has one canonical offer");
        offer.capability_id =
            conduit_core::CapabilityId::from(format!("conduitos/{kind}-capability@1").as_str());
        offer.implementation.execution_profile_id =
            ExecutionProfileId::from(CONDUITOS_PRESENTATION_PROFILE);
        offer.implementation.implementation_id =
            ImplementationId::from(format!("conduitos/{kind}-implementation@1").as_str());
        offer.implementation.artifact_id = ArtifactId::from(CONDUITOS_PRESENTATION_ARTIFACT);
        offer
    })
    .collect()
}
