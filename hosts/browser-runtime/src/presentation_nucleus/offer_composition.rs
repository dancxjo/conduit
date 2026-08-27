use conduit_core::{
    ArtifactId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId,
};
use conduit_presentation::{
    MAX_GRAPHICS_SCENE_BYTES, MAX_LAYOUT_FRAME_BYTES, MAX_PRESENTATION_COMPOSITION_BYTES,
};

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
        let mut offer = portable_offer(kind)
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

pub(super) fn portable_offer(kind: &str) -> Option<CapabilityOffer> {
    let (contract, revision, operation) = if let Some(contract) =
        conduit_std_catalog::layout_contract_for(kind)
    {
        let operation = (kind != conduit_std_catalog::LAYOUT_VIEWPORT_KIND).then_some((
            "conduit.host/layout-frame-transform@1",
            MAX_LAYOUT_FRAME_BYTES as u32,
            MAX_LAYOUT_FRAME_BYTES as u32,
        ));
        (
            contract,
            conduit_std_catalog::LAYOUT_CONTRACT_REVISION,
            operation,
        )
    } else if let Some(contract) = conduit_std_catalog::presentation_composition_contract_for(kind)
    {
        let operation = (kind != conduit_std_catalog::PRESENTATION_ICON_KIND).then_some((
            "conduit.host/presentation-composition-transform@1",
            MAX_PRESENTATION_COMPOSITION_BYTES as u32,
            MAX_PRESENTATION_COMPOSITION_BYTES as u32,
        ));
        (
            contract,
            conduit_std_catalog::PRESENTATION_COMPOSITION_CONTRACT_REVISION,
            operation,
        )
    } else {
        let contract = conduit_std_catalog::graphics_contract_for(kind)?;
        (
            contract,
            conduit_std_catalog::GRAPHICS_SCENE_CONTRACT_REVISION,
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
    Some(conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability: "browser/portable-presentation-face",
            execution_profile: BROWSER_PRESENTATION_PROFILE,
            implementation: "browser/portable-presentation-face@1",
            artifact: BROWSER_PRESENTATION_ARTIFACT,
        },
        host_operations,
        Vec::new(),
        Vec::new(),
    ))
}
