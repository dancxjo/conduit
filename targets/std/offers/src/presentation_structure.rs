//! Hosted std realizations of portable layout and scene-composition contracts.

use conduit_core::{CapabilityOffer, HostOperationContractId, HostOperationRequirement};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity, StandardKindContract};

pub const LAYOUT_HOST_OPERATION: &str = "conduit.host/layout-frame-transform@1";
pub const LAYOUT_VIEWPORT_IMPLEMENTATION: &str = "std/layout/viewport-implementation@1";
pub const LAYOUT_INSET_IMPLEMENTATION: &str = "std/layout/inset-implementation@1";
pub const LAYOUT_ROW_IMPLEMENTATION: &str = "std/layout/row-implementation@1";
pub const LAYOUT_COLUMN_IMPLEMENTATION: &str = "std/layout/column-implementation@1";
pub const LAYOUT_STACK_IMPLEMENTATION: &str = "std/layout/stack-implementation@1";
pub const LAYOUT_ALIGN_IMPLEMENTATION: &str = "std/layout/align-implementation@1";
pub const PRESENTATION_ICON_IMPLEMENTATION: &str = "std/presentation/icon-implementation@1";
pub const PRESENTATION_FRAME_IMPLEMENTATION: &str = "std/presentation/frame-implementation@1";
pub const PRESENTATION_BADGE_IMPLEMENTATION: &str = "std/presentation/badge-implementation@1";
pub const PRESENTATION_COMPOSITION_HOST_OPERATION: &str =
    "conduit.host/presentation-composition-transform@1";
pub const GRAPHICS_RECT_IMPLEMENTATION: &str = "std/graphics/rect-implementation@1";
pub const GRAPHICS_TEXT_IMPLEMENTATION: &str = "std/graphics/text-implementation@1";
pub const GRAPHICS_ICON_IMPLEMENTATION: &str = "std/graphics/icon-implementation@1";
pub const GRAPHICS_HOST_OPERATION: &str = "conduit.host/graphics-scene-transform@1";

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

pub fn presentation_icon_offer() -> CapabilityOffer {
    presentation_composition_offer(conduit_semantic_catalog::presentation_icon_contract())
}
pub fn presentation_frame_offer() -> CapabilityOffer {
    presentation_composition_offer(conduit_semantic_catalog::presentation_frame_contract())
}
pub fn presentation_badge_offer() -> CapabilityOffer {
    presentation_composition_offer(conduit_semantic_catalog::presentation_badge_contract())
}

pub fn presentation_composition_offer_for(kind: &str) -> Option<CapabilityOffer> {
    conduit_semantic_catalog::presentation_composition_contract_for(kind)
        .map(presentation_composition_offer)
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
            LAYOUT_HOST_OPERATION,
            &contract,
            conduit_presentation::MAX_LAYOUT_FRAME_BYTES as u32,
            conduit_presentation::MAX_LAYOUT_FRAME_BYTES as u32,
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

fn presentation_composition_offer(contract: StandardKindContract) -> CapabilityOffer {
    let kind = contract.kind_id.as_str().to_owned();
    let implementation = match kind.as_str() {
        conduit_semantic_catalog::PRESENTATION_ICON_KIND => PRESENTATION_ICON_IMPLEMENTATION,
        conduit_semantic_catalog::PRESENTATION_FRAME_KIND => PRESENTATION_FRAME_IMPLEMENTATION,
        conduit_semantic_catalog::PRESENTATION_BADGE_KIND => PRESENTATION_BADGE_IMPLEMENTATION,
        _ => unreachable!("presentation contract mapper admits only composition Kinds"),
    };
    let operations = if kind == conduit_semantic_catalog::PRESENTATION_ICON_KIND {
        Vec::new()
    } else {
        vec![operation(
            PRESENTATION_COMPOSITION_HOST_OPERATION,
            &contract,
            conduit_presentation::MAX_PRESENTATION_COMPOSITION_BYTES as u32,
            conduit_presentation::MAX_PRESENTATION_COMPOSITION_BYTES as u32,
        )]
    };
    offer(
        contract,
        conduit_semantic_catalog::PRESENTATION_COMPOSITION_CONTRACT_REVISION,
        &format!("std/{kind}-capability@1"),
        "conduit.std/presentation-composition-kernel@1",
        implementation,
        "conduit-std-host/presentation-composition@1",
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
        GRAPHICS_HOST_OPERATION,
        &contract,
        conduit_presentation::MAX_PRESENTATION_COMPOSITION_BYTES as u32,
        conduit_presentation::MAX_GRAPHICS_SCENE_BYTES as u32,
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
) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(id),
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
    operations: Vec<HostOperationRequirement>,
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
            presentation_icon_offer(),
            presentation_frame_offer(),
            presentation_badge_offer(),
            graphics_rect_offer(),
            graphics_text_offer(),
            graphics_icon_offer(),
        ];
        assert_eq!(offers.len(), 12);
        assert!(offers.iter().all(|offer| offer.limits.max_queue_items == 1));
        assert!(layout_viewport_offer().host_operations.is_empty());
        assert!(presentation_icon_offer().host_operations.is_empty());
        assert!(offers[1..6]
            .iter()
            .all(|offer| offer.host_operations.len() == 1));
        assert!(offers[7..]
            .iter()
            .all(|offer| offer.host_operations.len() == 1));
    }
}
