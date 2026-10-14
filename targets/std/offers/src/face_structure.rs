//! Hosted std realizations of portable layout and scene-composition contracts.

use conduit_core::{CapabilityOffer, HostCallContractId, HostCallRequirement};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity, StandardKindContract};

pub const LAYOUT_HOST_CALL: &str = "conduit.host/layout-frame-transform@1";
pub const LAYOUT_VIEWPORT_IMPLEMENTATION: &str = "std/layout/viewport-implementation@1";
pub const LAYOUT_INSET_IMPLEMENTATION: &str = "std/layout/inset-implementation@1";
pub const LAYOUT_ROW_IMPLEMENTATION: &str = "std/layout/row-implementation@1";
pub const LAYOUT_COLUMN_IMPLEMENTATION: &str = "std/layout/column-implementation@1";
pub const LAYOUT_STACK_IMPLEMENTATION: &str = "std/layout/stack-implementation@1";
pub const LAYOUT_ALIGN_IMPLEMENTATION: &str = "std/layout/align-implementation@1";
pub const FACE_ICON_IMPLEMENTATION: &str = "std/face/icon-implementation@1";
pub const FACE_FRAME_IMPLEMENTATION: &str = "std/face/frame-implementation@1";
pub const FACE_BADGE_IMPLEMENTATION: &str = "std/face/badge-implementation@1";
pub const FACE_COMPOSITION_HOST_CALL: &str = "conduit.host/face-composition-transform@1";
pub const GRAPHICS_RECT_IMPLEMENTATION: &str = "std/graphics/rect-implementation@1";
pub const GRAPHICS_TEXT_IMPLEMENTATION: &str = "std/graphics/text-implementation@1";
pub const GRAPHICS_ICON_IMPLEMENTATION: &str = "std/graphics/icon-implementation@1";
pub const GRAPHICS_HOST_CALL: &str = "conduit.host/graphics-scene-transform@1";

pub fn layout_viewport_offer() -> CapabilityOffer {
    layout_offer(conduit_semantic_catalog::layout_viewport_contract())
}
pub fn layout_inset_offer() -> CapabilityOffer {
    layout_offer(conduit_semantic_catalog::layout_inset_contract())
}
pub fn layout_row_offer() -> CapabilityOffer {
    layout_offer(conduit_semantic_catalog::layout_row_contract())
}
pub fn layout_column_offer() -> CapabilityOffer {
    layout_offer(conduit_semantic_catalog::layout_column_contract())
}
pub fn layout_stack_offer() -> CapabilityOffer {
    layout_offer(conduit_semantic_catalog::layout_stack_contract())
}
pub fn layout_align_offer() -> CapabilityOffer {
    layout_offer(conduit_semantic_catalog::layout_align_contract())
}

pub fn layout_offer_for(kind: &str) -> Option<CapabilityOffer> {
    conduit_semantic_catalog::layout_contract_for(kind).map(layout_offer)
}

pub fn face_icon_offer() -> CapabilityOffer {
    face_composition_offer(conduit_semantic_catalog::face_icon_contract())
}
pub fn face_frame_offer() -> CapabilityOffer {
    face_composition_offer(conduit_semantic_catalog::face_frame_contract())
}
pub fn face_badge_offer() -> CapabilityOffer {
    face_composition_offer(conduit_semantic_catalog::face_badge_contract())
}

pub fn face_composition_offer_for(kind: &str) -> Option<CapabilityOffer> {
    conduit_semantic_catalog::face_composition_contract_for(kind).map(face_composition_offer)
}

pub fn graphics_rect_offer() -> CapabilityOffer {
    graphics_offer(conduit_semantic_catalog::graphics_rect_contract())
}
pub fn graphics_text_offer() -> CapabilityOffer {
    graphics_offer(conduit_semantic_catalog::graphics_text_contract())
}
pub fn graphics_icon_offer() -> CapabilityOffer {
    graphics_offer(conduit_semantic_catalog::graphics_icon_contract())
}

pub fn graphics_offer_for(kind: &str) -> Option<CapabilityOffer> {
    conduit_semantic_catalog::graphics_contract_for(kind).map(graphics_offer)
}

fn layout_offer(contract: StandardKindContract) -> CapabilityOffer {
    let kind = contract.kind_id.as_str().to_owned();
    let implementation = match kind.as_str() {
        conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND => LAYOUT_VIEWPORT_IMPLEMENTATION,
        conduit_semantic_catalog::LAYOUT_INSET_KIND => LAYOUT_INSET_IMPLEMENTATION,
        conduit_semantic_catalog::LAYOUT_ROW_KIND => LAYOUT_ROW_IMPLEMENTATION,
        conduit_semantic_catalog::LAYOUT_COLUMN_KIND => LAYOUT_COLUMN_IMPLEMENTATION,
        conduit_semantic_catalog::LAYOUT_STACK_KIND => LAYOUT_STACK_IMPLEMENTATION,
        conduit_semantic_catalog::LAYOUT_ALIGN_KIND => LAYOUT_ALIGN_IMPLEMENTATION,
        _ => unreachable!("layout contract mapper admits only layout Kinds"),
    };
    let operations = if kind == conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND {
        Vec::new()
    } else {
        vec![operation(
            LAYOUT_HOST_CALL,
            &contract,
            conduit_face::MAX_LAYOUT_FRAME_BYTES as u32,
            conduit_face::MAX_LAYOUT_FRAME_BYTES as u32,
        )]
    };
    offer(
        contract,
        conduit_semantic_catalog::LAYOUT_CONTRACT_REVISION,
        &format!("std/{kind}-capability@1"),
        "conduit.std/layout-frame-kernel@1",
        implementation,
        "conduit-std-host/layout-frame@1",
        operations,
    )
}

fn face_composition_offer(contract: StandardKindContract) -> CapabilityOffer {
    let kind = contract.kind_id.as_str().to_owned();
    let implementation = match kind.as_str() {
        conduit_semantic_catalog::FACE_ICON_KIND => FACE_ICON_IMPLEMENTATION,
        conduit_semantic_catalog::FACE_FRAME_KIND => FACE_FRAME_IMPLEMENTATION,
        conduit_semantic_catalog::FACE_BADGE_KIND => FACE_BADGE_IMPLEMENTATION,
        _ => unreachable!("face contract mapper admits only composition Kinds"),
    };
    let operations = if kind == conduit_semantic_catalog::FACE_ICON_KIND {
        Vec::new()
    } else {
        vec![operation(
            FACE_COMPOSITION_HOST_CALL,
            &contract,
            conduit_face::MAX_FACE_COMPOSITION_BYTES as u32,
            conduit_face::MAX_FACE_COMPOSITION_BYTES as u32,
        )]
    };
    offer(
        contract,
        conduit_semantic_catalog::FACE_COMPOSITION_CONTRACT_REVISION,
        &format!("std/{kind}-capability@1"),
        "conduit.std/face-composition-kernel@1",
        implementation,
        "conduit-std-host/face-composition@1",
        operations,
    )
}

fn graphics_offer(contract: StandardKindContract) -> CapabilityOffer {
    let kind = contract.kind_id.as_str().to_owned();
    let implementation = match kind.as_str() {
        conduit_semantic_catalog::GRAPHICS_RECT_KIND => GRAPHICS_RECT_IMPLEMENTATION,
        conduit_semantic_catalog::GRAPHICS_TEXT_KIND => GRAPHICS_TEXT_IMPLEMENTATION,
        conduit_semantic_catalog::GRAPHICS_ICON_KIND => GRAPHICS_ICON_IMPLEMENTATION,
        _ => unreachable!("graphics contract mapper admits only graphics Kinds"),
    };
    let operations = vec![operation(
        GRAPHICS_HOST_CALL,
        &contract,
        conduit_face::MAX_FACE_COMPOSITION_BYTES as u32,
        conduit_face::MAX_GRAPHICS_SCENE_BYTES as u32,
    )];
    offer(
        contract,
        conduit_semantic_catalog::GRAPHICS_SCENE_CONTRACT_REVISION,
        &format!("std/{kind}-capability@1"),
        "conduit.std/graphics-scene-kernel@1",
        implementation,
        "conduit-std-host/graphics-scene@1",
        operations,
    )
}

fn operation(
    id: &str,
    contract: &StandardKindContract,
    input: u32,
    output: u32,
) -> HostCallRequirement {
    HostCallRequirement {
        contract_id: HostCallContractId::from(id),
        target_kind: Some(contract.kind_id.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    }
}

#[allow(clippy::too_many_arguments)]
fn offer(
    contract: StandardKindContract,
    revision: &str,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    operations: Vec<HostCallRequirement>,
) -> CapabilityOffer {
    realization_offer(
        contract,
        revision,
        RealizationOfferIdentity {
            capability,
            execution_profile: profile,
            implementation,
            artifact,
        },
        operations,
        Vec::new(),
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_twelve_structural_offers_preserve_portable_contracts() {
        let offers = [
            layout_viewport_offer(),
            layout_inset_offer(),
            layout_row_offer(),
            layout_column_offer(),
            layout_stack_offer(),
            layout_align_offer(),
            face_icon_offer(),
            face_frame_offer(),
            face_badge_offer(),
            graphics_rect_offer(),
            graphics_text_offer(),
            graphics_icon_offer(),
        ];
        assert_eq!(offers.len(), 12);
        assert!(offers.iter().all(|offer| offer.limits.max_queue_items == 1));
        assert!(layout_viewport_offer().host_calls.is_empty());
        assert!(face_icon_offer().host_calls.is_empty());
        assert!(offers[1..6].iter().all(|offer| offer.host_calls.len() == 1));
        assert!(offers[7..].iter().all(|offer| offer.host_calls.len() == 1));
    }
}
