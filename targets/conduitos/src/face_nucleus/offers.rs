#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
use alloc::{format, string::String};
use alloc::{vec, vec::Vec};
#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
use conduit_core::{ArtifactId, ExecutionProfileId, ImplementationId};
use conduit_core::{
    CapabilityOffer, FACE_RESOURCE_CLASS, HostCallContractId, HostCallRequirement, kind_id,
    present_host_call_requirement, resource_requirement,
};
use conduit_face::{
    BITMAP_FACE_KIND, MAX_FACE_COMPOSITION_BYTES, MAX_GRAPHICS_SCENE_BYTES, MAX_LAYOUT_FRAME_BYTES,
};

pub const CONDUITOS_FACE_PROFILE: &str = "conduitos/framebuffer-face-kernel@1";
pub const CONDUITOS_FACE_ARTIFACT: &str = "conduitos/framebuffer-face@1";

#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
pub fn face_nucleus_offers() -> Vec<CapabilityOffer> {
    let mut offers: Vec<_> = [
        conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND,
        conduit_semantic_catalog::LAYOUT_INSET_KIND,
        conduit_semantic_catalog::LAYOUT_ROW_KIND,
        conduit_semantic_catalog::LAYOUT_COLUMN_KIND,
        conduit_semantic_catalog::LAYOUT_STACK_KIND,
        conduit_semantic_catalog::LAYOUT_ALIGN_KIND,
        conduit_semantic_catalog::FACE_ICON_KIND,
        conduit_semantic_catalog::FACE_FRAME_KIND,
        conduit_semantic_catalog::FACE_BADGE_KIND,
        conduit_semantic_catalog::TEXT_FACE_KIND,
        conduit_semantic_catalog::GRAPHICS_RECT_KIND,
        conduit_semantic_catalog::GRAPHICS_TEXT_KIND,
        conduit_semantic_catalog::GRAPHICS_ICON_KIND,
        conduit_semantic_catalog::GRAPHICS_FACE_KIND,
        BITMAP_FACE_KIND,
        conduit_semantic_catalog::BOOL_FACE_KIND,
        conduit_semantic_catalog::COUNT_FACE_KIND,
    ]
    .into_iter()
    .map(|kind| {
        let mut offer =
            face_offer_for(kind).expect("accepted ConduitOS face Kind has one canonical offer");
        offer.capability_id =
            conduit_core::CapabilityId::from(format!("conduitos/{kind}-capability@1").as_str());
        offer.implementation.execution_profile_id =
            ExecutionProfileId::from(CONDUITOS_FACE_PROFILE);
        offer.implementation.implementation_id =
            ImplementationId::from(format!("conduitos/{kind}-implementation@1").as_str());
        offer.implementation.artifact_id = ArtifactId::from(CONDUITOS_FACE_ARTIFACT);
        offer
    })
    .collect();
    offers.extend(
        conduit_semantic_catalog::patchbay_face_contracts()
            .into_iter()
            .filter(|contract| {
                contract.kind_id.as_str() != conduit_semantic_catalog::PATCHBAY_GEAR_FACE_KIND
            })
            .map(conduitos_patchbay_offer),
    );
    offers
}

pub(crate) fn face_offer_for(kind: &str) -> Option<CapabilityOffer> {
    portable_offer(kind).or_else(|| sink_offer(kind))
}

fn portable_offer(kind: &str) -> Option<CapabilityOffer> {
    let (contract, revision, operation) = if let Some(contract) =
        conduit_semantic_catalog::layout_contract_for(kind)
    {
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
    } else if let Some(contract) = conduit_semantic_catalog::face_composition_contract_for(kind) {
        let operation = (kind != conduit_semantic_catalog::FACE_ICON_KIND).then_some((
            "conduit.host/face-composition-transform@1",
            MAX_FACE_COMPOSITION_BYTES as u32,
            MAX_FACE_COMPOSITION_BYTES as u32,
        ));
        (
            contract,
            conduit_semantic_catalog::FACE_COMPOSITION_CONTRACT_REVISION,
            operation,
        )
    } else {
        let contract = conduit_semantic_catalog::graphics_contract_for(kind)?;
        (
            contract,
            conduit_semantic_catalog::GRAPHICS_SCENE_CONTRACT_REVISION,
            Some((
                "conduit.host/graphics-scene-transform@1",
                MAX_FACE_COMPOSITION_BYTES as u32,
                MAX_GRAPHICS_SCENE_BYTES as u32,
            )),
        )
    };
    let host_calls = operation
        .map(|(id, input, output)| HostCallRequirement {
            contract_id: HostCallContractId::from(id),
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
            capability: "conduitos/portable-face-front",
            execution_profile: CONDUITOS_FACE_PROFILE,
            implementation: "conduitos/portable-face-front@1",
            artifact: CONDUITOS_FACE_ARTIFACT,
        },
        host_calls,
        Vec::new(),
        Vec::new(),
    ))
}

fn sink_offer(kind: &str) -> Option<CapabilityOffer> {
    let (contract, revision, target, maximum_input_bytes) = match kind {
        conduit_semantic_catalog::TICK_FACE_KIND => (
            conduit_semantic_catalog::tick_face_contract(),
            conduit_semantic_catalog::TICK_FACE_CONTRACT_REVISION,
            "face/conduitos-tick",
            conduit_time::TICK_ENCODED_LEN,
        ),
        conduit_semantic_catalog::BOOL_FACE_KIND => (
            conduit_semantic_catalog::bool_face_contract(),
            conduit_semantic_catalog::BOOL_FACE_CONTRACT_REVISION,
            "face/conduitos-framebuffer-bool",
            conduit_core::BOOL_ENCODED_LEN as u32,
        ),
        conduit_semantic_catalog::TEXT_FACE_KIND => (
            conduit_semantic_catalog::text_face_contract(),
            conduit_semantic_catalog::TEXT_FACE_CONTRACT_REVISION,
            "face/conduitos-text",
            conduit_text::MAX_TEXT_BYTES,
        ),
        conduit_semantic_catalog::COUNT_FACE_KIND => (
            conduit_semantic_catalog::count_face_contract(),
            conduit_semantic_catalog::COUNT_FACE_CONTRACT_REVISION,
            "face/conduitos-count",
            conduit_semantic_catalog::COUNT_ENCODED_LEN,
        ),
        conduit_semantic_catalog::GRAPHICS_FACE_KIND => (
            conduit_semantic_catalog::graphics_face_contract(),
            conduit_semantic_catalog::GRAPHICS_FACE_REVISION,
            "face/conduitos-graphics-scene",
            MAX_GRAPHICS_SCENE_BYTES as u32,
        ),
        BITMAP_FACE_KIND => (
            conduit_semantic_catalog::bitmap_face_contract(),
            conduit_face::BITMAP_FACE_REVISION,
            "face/conduitos-bitmap-gray8",
            conduit_face::MAX_GRAY8_BITMAP_BYTES as u32,
        ),
        _ => return None,
    };
    Some(conduit_semantic_catalog::realization_offer(
        contract,
        revision,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: "conduitos/face-sink",
            execution_profile: CONDUITOS_FACE_PROFILE,
            implementation: "conduitos/face-sink@1",
            artifact: CONDUITOS_FACE_ARTIFACT,
        },
        vec![present_host_call_requirement(
            kind_id(target),
            maximum_input_bytes,
        )],
        vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
        Vec::new(),
    ))
}

#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
fn conduitos_patchbay_offer(
    contract: conduit_semantic_catalog::StandardKindContract,
) -> CapabilityOffer {
    let kind = String::from(contract.kind_id.as_str());
    let capability = format!("conduitos/{kind}-capability@1");
    let implementation = format!("conduitos/{kind}-implementation@1");
    conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::PATCHBAY_FACE_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: &capability,
            execution_profile: CONDUITOS_FACE_PROFILE,
            implementation: &implementation,
            artifact: CONDUITOS_FACE_ARTIFACT,
        },
        vec![present_host_call_requirement(
            kind_id("face/patchbay-surface@1"),
            conduit_semantic_catalog::MAX_PATCHBAY_FACE_BYTES,
        )],
        vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_count_and_the_complete_patchbay_waist() {
        let offers = face_nucleus_offers();
        for kind in [
            conduit_semantic_catalog::COUNT_FACE_KIND,
            BITMAP_FACE_KIND,
            conduit_semantic_catalog::PATCHBAY_FACE_KIND,
            conduit_semantic_catalog::PATCHBAY_PORT_KIND,
            conduit_semantic_catalog::PATCHBAY_CORD_KIND,
        ] {
            let offer = offers
                .iter()
                .find(|offer| offer.kind_id.as_str() == kind)
                .expect("ConduitOS face nucleus must contain canonical Kind");
            assert_eq!(
                offer.implementation.execution_profile_id.as_str(),
                CONDUITOS_FACE_PROFILE
            );
        }
        assert!(offers.iter().all(|offer| {
            offer.kind_id.as_str() != conduit_semantic_catalog::PATCHBAY_GEAR_FACE_KIND
        }));
    }
}
