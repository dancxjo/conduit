#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
use alloc::format;
use alloc::{vec, vec::Vec};
#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
use conduit_core::{ArtifactId, ExecutionProfileId, ImplementationId};
use conduit_core::{
    CapabilityOffer, HostOperationContractId, HostOperationRequirement,
    PRESENTATION_RESOURCE_CLASS, kind_id, present_host_operation_requirement, resource_requirement,
};
use conduit_presentation::{
    BITMAP_PRESENTATION_KIND, MAX_GRAPHICS_SCENE_BYTES, MAX_LAYOUT_FRAME_BYTES,
    MAX_PRESENTATION_COMPOSITION_BYTES,
};

pub const CONDUITOS_PRESENTATION_PROFILE: &str = "conduitos/framebuffer-presentation-kernel@1";
pub const CONDUITOS_PRESENTATION_ARTIFACT: &str = "conduitos/framebuffer-presentation@1";

#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
pub fn presentation_nucleus_offers() -> Vec<CapabilityOffer> {
    let mut offers: Vec<_> = [
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
        conduit_std_catalog::GRAPHICS_PRESENTATION_KIND,
        BITMAP_PRESENTATION_KIND,
        conduit_std_catalog::BOOL_PRESENTATION_KIND,
        conduit_std_catalog::COUNT_PRESENTATION_KIND,
    ]
    .into_iter()
    .map(|kind| {
        let mut offer = presentation_offer_for(kind)
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
    .collect();
    offers.extend(
        conduit_std_catalog::patchbay_presentation_offers()
            .into_iter()
            .filter(|offer| offer.kind_id.as_str() != conduit_std_catalog::PATCHBAY_GEAR_FACE_KIND)
            .map(bind_conduitos_presentation),
    );
    offers
}

pub(crate) fn presentation_offer_for(kind: &str) -> Option<CapabilityOffer> {
    portable_offer(kind).or_else(|| sink_offer(kind))
}

fn portable_offer(kind: &str) -> Option<CapabilityOffer> {
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
            capability: "conduitos/portable-presentation-face",
            execution_profile: CONDUITOS_PRESENTATION_PROFILE,
            implementation: "conduitos/portable-presentation-face@1",
            artifact: CONDUITOS_PRESENTATION_ARTIFACT,
        },
        host_operations,
        Vec::new(),
        Vec::new(),
    ))
}

fn sink_offer(kind: &str) -> Option<CapabilityOffer> {
    let (contract, revision, target, maximum_input_bytes) = match kind {
        conduit_std_catalog::TICK_PRESENTATION_KIND => (
            conduit_std_catalog::tick_presentation_contract(),
            conduit_std_catalog::TICK_PRESENTATION_CONTRACT_REVISION,
            "presentation/conduitos-tick",
            conduit_time::TICK_ENCODED_LEN,
        ),
        conduit_std_catalog::BOOL_PRESENTATION_KIND => (
            conduit_std_catalog::bool_presentation_contract(),
            conduit_std_catalog::BOOL_PRESENTATION_CONTRACT_REVISION,
            "presentation/conduitos-framebuffer-bool",
            conduit_core::BOOL_ENCODED_LEN as u32,
        ),
        conduit_std_catalog::TEXT_PRESENTATION_KIND => (
            conduit_std_catalog::text_presentation_contract(),
            conduit_std_catalog::TEXT_PRESENTATION_CONTRACT_REVISION,
            "presentation/conduitos-text",
            conduit_text::MAX_TEXT_BYTES,
        ),
        conduit_std_catalog::COUNT_PRESENTATION_KIND => (
            conduit_std_catalog::count_presentation_contract(),
            conduit_std_catalog::COUNT_PRESENTATION_CONTRACT_REVISION,
            "presentation/conduitos-count",
            conduit_std_catalog::COUNT_ENCODED_LEN,
        ),
        conduit_std_catalog::GRAPHICS_PRESENTATION_KIND => (
            conduit_std_catalog::graphics_presentation_contract(),
            conduit_std_catalog::GRAPHICS_PRESENTATION_REVISION,
            "presentation/conduitos-graphics-scene",
            MAX_GRAPHICS_SCENE_BYTES as u32,
        ),
        BITMAP_PRESENTATION_KIND => (
            conduit_std_catalog::bitmap_presentation_contract(),
            conduit_presentation::BITMAP_PRESENTATION_REVISION,
            "presentation/conduitos-bitmap-gray8",
            conduit_presentation::MAX_GRAY8_BITMAP_BYTES as u32,
        ),
        _ => return None,
    };
    Some(conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability: "conduitos/presentation-sink",
            execution_profile: CONDUITOS_PRESENTATION_PROFILE,
            implementation: "conduitos/presentation-sink@1",
            artifact: CONDUITOS_PRESENTATION_ARTIFACT,
        },
        vec![present_host_operation_requirement(
            kind_id(target),
            maximum_input_bytes,
        )],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        Vec::new(),
    ))
}

#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
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
        let offers = presentation_nucleus_offers();
        for kind in [
            conduit_std_catalog::COUNT_PRESENTATION_KIND,
            BITMAP_PRESENTATION_KIND,
            conduit_std_catalog::PATCHBAY_PRESENTATION_KIND,
            conduit_std_catalog::PATCHBAY_PORT_KIND,
            conduit_std_catalog::PATCHBAY_CORD_KIND,
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
        assert!(offers.iter().all(|offer| {
            offer.kind_id.as_str() != conduit_std_catalog::PATCHBAY_GEAR_FACE_KIND
        }));
    }
}
