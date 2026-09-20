use conduit_core::{
    kind_id, present_host_operation_requirement, resource_requirement, CapabilityOffer,
    HostOperationContractId, HostOperationRequirement, PRESENTATION_RESOURCE_CLASS,
};
use conduit_presentation::{
    MAX_GRAPHICS_SCENE_BYTES, MAX_LAYOUT_FRAME_BYTES, MAX_PRESENTATION_COMPOSITION_BYTES,
};

pub const BROWSER_PRESENTATION_PROFILE: &str = "browser/presentation-nucleus-kernel@1";
pub const BROWSER_PRESENTATION_ARTIFACT: &str = "conduit-browser-runtime/presentation-nucleus@1";

pub fn offers() -> Vec<CapabilityOffer> {
    [
        conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND,
        conduit_semantic_catalog::LAYOUT_INSET_KIND,
        conduit_semantic_catalog::LAYOUT_ROW_KIND,
        conduit_semantic_catalog::LAYOUT_COLUMN_KIND,
        conduit_semantic_catalog::LAYOUT_STACK_KIND,
        conduit_semantic_catalog::LAYOUT_ALIGN_KIND,
        conduit_semantic_catalog::PRESENTATION_ICON_KIND,
        conduit_semantic_catalog::PRESENTATION_FRAME_KIND,
        conduit_semantic_catalog::PRESENTATION_BADGE_KIND,
        conduit_semantic_catalog::TEXT_PRESENTATION_KIND,
        conduit_semantic_catalog::GRAPHICS_RECT_KIND,
        conduit_semantic_catalog::GRAPHICS_TEXT_KIND,
        conduit_semantic_catalog::GRAPHICS_ICON_KIND,
    ]
    .into_iter()
    .map(|kind| {
        let capability = format!("browser/{kind}-capability@1");
        let implementation = format!("browser/{kind}-implementation@1");
        portable_offer_for(kind, &capability, &implementation)
            .or_else(|| {
                (kind == conduit_semantic_catalog::TEXT_PRESENTATION_KIND)
                    .then(|| text_offer_for(&capability, &implementation))
            })
            .expect("accepted browser presentation Kind has one canonical offer")
    })
    .collect()
}

#[cfg(test)]
pub(super) fn text_offer() -> CapabilityOffer {
    text_offer_for(
        "browser-text-presentation-v1",
        "browser/presentation-text-implementation@1",
    )
}

fn text_offer_for(capability: &str, implementation: &str) -> CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::text_presentation_contract(),
        conduit_semantic_catalog::TEXT_PRESENTATION_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: BROWSER_PRESENTATION_PROFILE,
            implementation,
            artifact: BROWSER_PRESENTATION_ARTIFACT,
        },
        vec![present_host_operation_requirement(
            kind_id("presentation/browser-text"),
            conduit_text::MAX_TEXT_BYTES,
        )],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

#[cfg(test)]
pub(super) fn portable_offer(kind: &str) -> Option<CapabilityOffer> {
    portable_offer_for(
        kind,
        "browser/portable-presentation-front",
        "browser/portable-presentation-front@1",
    )
}

fn portable_offer_for(
    kind: &str,
    capability: &str,
    implementation: &str,
) -> Option<CapabilityOffer> {
    let (contract, revision, operation) =
        if let Some(contract) = conduit_semantic_catalog::layout_contract_for(kind) {
            let operation = (kind != conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND).then_some((
                "conduit.host/layout-frame-transform@1",
                MAX_LAYOUT_FRAME_BYTES as u32,
                MAX_LAYOUT_FRAME_BYTES as u32,
            ));
            (
                contract,
                conduit_semantic_catalog::LAYOUT_CONTRACT_REVISION,
                operation,
            )
        } else if let Some(contract) =
            conduit_semantic_catalog::presentation_composition_contract_for(kind)
        {
            let operation = (kind != conduit_semantic_catalog::PRESENTATION_ICON_KIND).then_some((
                "conduit.host/presentation-composition-transform@1",
                MAX_PRESENTATION_COMPOSITION_BYTES as u32,
                MAX_PRESENTATION_COMPOSITION_BYTES as u32,
            ));
            (
                contract,
                conduit_semantic_catalog::PRESENTATION_COMPOSITION_CONTRACT_REVISION,
                operation,
            )
        } else {
            let contract = conduit_semantic_catalog::graphics_contract_for(kind)?;
            (
                contract,
                conduit_semantic_catalog::GRAPHICS_SCENE_CONTRACT_REVISION,
                Some((
                    "conduit.host/graphics-scene-transform@1",
                    MAX_PRESENTATION_COMPOSITION_BYTES as u32,
                    MAX_GRAPHICS_SCENE_BYTES as u32,
                )),
            )
        };
    let host_operations = operation
        .map(|(id, input, output)| HostOperationRequirement {
            contract_id: HostOperationContractId::from(id),
            target_kind: Some(contract.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: input,
            maximum_output_bytes: output,
        })
        .into_iter()
        .collect();
    Some(conduit_semantic_catalog::realization_offer(
        contract,
        revision,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: BROWSER_PRESENTATION_PROFILE,
            implementation,
            artifact: BROWSER_PRESENTATION_ARTIFACT,
        },
        host_operations,
        Vec::new(),
        Vec::new(),
    ))
}
