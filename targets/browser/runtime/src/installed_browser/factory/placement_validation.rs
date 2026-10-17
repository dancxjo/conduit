//! Validate exact planned facts against one installed browser Back.
use conduit_core::CapabilityOffer;

pub(in super::super) fn validate_placement(
    placement: &conduit_core::PlannedGear,
    offer: &CapabilityOffer,
) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.realization_properties != offer.realization_properties
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
    {
        return Err("planned browser Gear does not match its installed capability".into());
    }
    conduit_semantic_catalog::validate_declared_data_labels(
        &placement.configuration,
        &offer.semantic_contract.configuration,
    )?;
    if conduit_semantic_catalog::face_data_label_required(placement.kind_id.as_str()) {
        conduit_semantic_catalog::face_data_label(&placement.configuration)?;
    }
    Ok(())
}

#[cfg(test)]
mod label_tests {
    use super::*;
    use conduit_core::{ConfigurationEntry, ConfigurationValue, PlannedGear};

    fn placement(offer: &CapabilityOffer, label: &str) -> PlannedGear {
        let configuration = offer
            .semantic_contract
            .configuration
            .iter()
            .map(|field| ConfigurationEntry {
                key: field.key.clone(),
                value: if field.key == "label" {
                    ConfigurationValue::Text(label.into())
                } else {
                    field.default_value.clone()
                },
            })
            .collect();
        conduit_core::planned_gear_from_parts! {
            realization_properties: offer.realization_properties.clone(),
            semantic_contract: offer.semantic_contract.clone(),
            placement_id: "browser-label-bound".into(), gear_id: "named-frame".into(),
            kind_id: offer.kind_id.clone(), kind_contract_revision: offer.kind_contract_revision.clone(),
            execution_profile_id: offer.implementation.execution_profile_id.clone(), configuration: configuration,
            host_id: "browser-label-host".into(), boot_id: "browser-label-boot".into(), offer_generation: conduit_core::OfferGeneration(1),
            capability_id: offer.capability_id.clone(), implementation_id: offer.implementation.implementation_id.clone(),
            artifact_id: offer.implementation.artifact_id.clone(), base: None, realization_characteristics: Vec::new(),
            limits: offer.limits.clone(), inputs: offer.inputs.clone(), outputs: offer.outputs.clone(),
            terminal_transductions: Vec::new(), host_calls: offer.host_calls.clone(), resources: Vec::new(), authority: Vec::new(), pool_references: Vec::new(),
        }
    }

    #[test]
    fn declared_label_bound_is_checked_against_installed_offer_before_retention() {
        let offers = crate::face_nucleus::offers();
        for kind in [
            conduit_semantic_catalog::FACE_FRAME_KIND,
            conduit_semantic_catalog::FACE_BADGE_KIND,
        ] {
            let offer = offers
                .iter()
                .find(|offer| offer.kind_id.as_str() == kind)
                .unwrap();
            let valid = placement(offer, &"x".repeat(64));
            assert!(validate_placement(&valid, offer).is_ok());
            assert!(
                crate::face_nucleus::validate_nucleus_data_labels(std::slice::from_ref(&valid))
                    .is_ok()
            );
            let mut forged = placement(offer, &"x".repeat(65));
            assert!(conduit_core::face_data_label(&forged.configuration).is_ok());
            forged.semantic_contract.configuration[0].rule =
                conduit_core::KindConfigurationRule::DataLabel { maximum: 128 };
            assert!(validate_placement(&forged, offer).is_err());
            assert!(crate::face_nucleus::validate_nucleus_data_labels(&[forged]).is_err());
        }
    }
}
