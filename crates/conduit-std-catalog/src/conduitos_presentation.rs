use alloc::{format, vec::Vec};
use conduit_core::{ArtifactId, CapabilityOffer, ExecutionProfileId, ImplementationId};
use conduit_presentation::BITMAP_PRESENTATION_KIND;

use crate::{
    bitmap_presentation_offer, bool_presentation_std_offer, graphics_offer_for,
    graphics_presentation_offer, layout_offer_for, patchbay_presentation_offers,
    presentation_composition_offer_for, text_presentation_offer, BOOL_PRESENTATION_KIND,
    COUNT_PRESENTATION_KIND, GRAPHICS_ICON_KIND, GRAPHICS_PRESENTATION_KIND, GRAPHICS_RECT_KIND,
    GRAPHICS_TEXT_KIND, LAYOUT_ALIGN_KIND, LAYOUT_COLUMN_KIND, LAYOUT_INSET_KIND, LAYOUT_ROW_KIND,
    LAYOUT_STACK_KIND, LAYOUT_VIEWPORT_KIND, PRESENTATION_BADGE_KIND, PRESENTATION_FRAME_KIND,
    PRESENTATION_ICON_KIND, TEXT_PRESENTATION_KIND,
};

pub const CONDUITOS_PRESENTATION_PROFILE: &str = "conduitos/framebuffer-presentation-kernel@1";
pub const CONDUITOS_PRESENTATION_ARTIFACT: &str = "conduitos/framebuffer-presentation@1";

pub fn conduitos_presentation_nucleus_offers() -> Vec<CapabilityOffer> {
    let mut offers: Vec<_> = [
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
        BITMAP_PRESENTATION_KIND,
        BOOL_PRESENTATION_KIND,
        COUNT_PRESENTATION_KIND,
    ]
    .into_iter()
    .map(|kind| {
        let mut offer = layout_offer_for(kind)
            .or_else(|| presentation_composition_offer_for(kind))
            .or_else(|| graphics_offer_for(kind))
            .or_else(|| (kind == GRAPHICS_PRESENTATION_KIND).then(graphics_presentation_offer))
            .or_else(|| (kind == BITMAP_PRESENTATION_KIND).then(bitmap_presentation_offer))
            .or_else(|| (kind == TEXT_PRESENTATION_KIND).then(text_presentation_offer))
            .or_else(|| (kind == BOOL_PRESENTATION_KIND).then(bool_presentation_std_offer))
            .or_else(|| (kind == COUNT_PRESENTATION_KIND).then(crate::count_presentation_offer))
            .expect("accepted ConduitOS presentation Kind has one canonical offer");
        offer.capability_id =
            conduit_core::CapabilityId::from(format!("conduitos/{kind}-capability@1").as_str());
        offer.implementation.execution_profile_id =
            ExecutionProfileId::from(CONDUITOS_PRESENTATION_PROFILE);
        offer.implementation.implementation_id =
            ImplementationId::from(format!("conduitos/{kind}-implementation@1").as_str());
        offer.implementation.artifact_id = ArtifactId::from(CONDUITOS_PRESENTATION_ARTIFACT);
        if kind == BOOL_PRESENTATION_KIND {
            offer.host_operations[0].target_kind = Some(conduit_core::kind_id(
                "presentation/conduitos-framebuffer-bool",
            ));
        }
        offer
    })
    .collect();
    offers.extend(
        patchbay_presentation_offers()
            .into_iter()
            .filter(|offer| offer.kind_id.as_str() != crate::PATCHBAY_GEAR_FACE_KIND)
            .map(bind_conduitos_presentation),
    );
    offers
}

fn bind_conduitos_presentation(mut offer: CapabilityOffer) -> CapabilityOffer {
    let kind = offer.kind_id.as_str();
    offer.capability_id =
        conduit_core::CapabilityId::from(format!("conduitos/{kind}-capability@1").as_str());
    offer.implementation.execution_profile_id =
        ExecutionProfileId::from(CONDUITOS_PRESENTATION_PROFILE);
    offer.implementation.implementation_id =
        ImplementationId::from(format!("conduitos/{kind}-implementation@1").as_str());
    offer.implementation.artifact_id = ArtifactId::from(CONDUITOS_PRESENTATION_ARTIFACT);
    offer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_count_and_the_complete_patchbay_waist() {
        let offers = conduitos_presentation_nucleus_offers();
        for kind in [
            COUNT_PRESENTATION_KIND,
            BITMAP_PRESENTATION_KIND,
            crate::PATCHBAY_PRESENTATION_KIND,
            crate::PATCHBAY_PORT_KIND,
            crate::PATCHBAY_CORD_KIND,
        ] {
            let offer = offers
                .iter()
                .find(|offer| offer.kind_id.as_str() == kind)
                .expect("ConduitOS presentation nucleus must contain canonical Kind");
            assert_eq!(
                offer.implementation.execution_profile_id.as_str(),
                CONDUITOS_PRESENTATION_PROFILE
            );
        }
        assert!(offers
            .iter()
            .all(|offer| offer.kind_id.as_str() != crate::PATCHBAY_GEAR_FACE_KIND));
    }
}
