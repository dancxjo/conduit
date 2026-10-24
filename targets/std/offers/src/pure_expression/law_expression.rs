//! Distinct installed identity for complete Source-law capsules.
use conduit_core::{CapabilityOffer, PortTemporal};
use conduit_plot::{LawExpressionMaterial, LawExpressionRefusal};

pub const LAW_EXPRESSION_STD_IMPLEMENTATION: &str = "std/kernel-law-expression@1";
pub const LAW_FILTER_STD_IMPLEMENTATION: &str = "std/kernel-law-filter@1";
pub const LAW_EXPRESSION_STD_PROFILE: &str = "std/checked-native-law-expression@1";
pub const LAW_FILTER_STD_PROFILE: &str = "std/checked-native-law-filter@1";
pub const LAW_EXPRESSION_STD_ARTIFACT: &str = "conduit-plot/checked-native-law-expression@1";
pub const LAW_FILTER_STD_ARTIFACT: &str = "conduit-plot/checked-native-law-filter@1";

/// Describes realization requirements, without granting Source custody. The
/// owning Host must independently verify and prepare the complete capsule.
pub fn law_expression_std_offer(
    material: &LawExpressionMaterial,
    temporal: PortTemporal,
    filter: bool,
) -> Result<CapabilityOffer, LawExpressionRefusal> {
    let definition = if filter {
        conduit_plot::law_filter_definition(material, temporal)?
    } else {
        conduit_plot::law_expression_definition(material, temporal)?
    };
    let mut offer = if filter {
        super::pure_filter_std_offer(&material.program, temporal)
    } else {
        super::pure_expression_std_offer(&material.program, temporal)
    }
    .map_err(|_| LawExpressionRefusal::Encoding)?;
    offer.kind_id = definition.kind_id;
    offer.kind_contract_revision = definition.kind_contract_revision;
    offer.inputs = definition.inputs;
    offer.outputs = definition.outputs;
    offer.semantic_contract.configuration = definition.configuration;
    offer.startup_parameters[0].name = "law-program".into();
    offer.capability_id = format!("std/{}", offer.kind_id.as_str()).into();
    for call in &mut offer.host_calls {
        call.target_kind = Some(offer.kind_id.clone());
    }
    let (implementation, profile, artifact) = if filter {
        (
            LAW_FILTER_STD_IMPLEMENTATION,
            LAW_FILTER_STD_PROFILE,
            LAW_FILTER_STD_ARTIFACT,
        )
    } else {
        (
            LAW_EXPRESSION_STD_IMPLEMENTATION,
            LAW_EXPRESSION_STD_PROFILE,
            LAW_EXPRESSION_STD_ARTIFACT,
        )
    };
    offer.implementation.implementation_id = implementation.into();
    offer.implementation.execution_profile_id = profile.into();
    offer.implementation.artifact_id = artifact.into();
    Ok(offer)
}
