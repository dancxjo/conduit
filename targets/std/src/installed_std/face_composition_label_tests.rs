use super::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue};

fn placement(kind: &str, label: &str) -> PlannedGear {
    let offer = conduit_std_offers::face_composition_offer_for(kind).unwrap();
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
        realization_properties: offer.realization_properties,
        semantic_contract: offer.semantic_contract,
        placement_id: "label-bound-placement".into(), gear_id: "named-composition".into(),
        kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: configuration,
        host_id: "label-bound-host".into(), boot_id: "label-bound-boot".into(), offer_generation: conduit_core::OfferGeneration(1),
        capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        base: None, realization_characteristics: Vec::new(), limits: offer.limits,
        inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: Vec::new(), host_calls: offer.host_calls,
        resources: Vec::new(), authority: Vec::new(), pool_references: Vec::new(),
    }
}

#[test]
fn declared_composition_label_bound_refuses_forged_plan_before_budget_or_prepare() {
    for kind in [
        conduit_semantic_catalog::FACE_FRAME_KIND,
        conduit_semantic_catalog::FACE_BADGE_KIND,
    ] {
        let valid = placement(kind, &"x".repeat(64));
        let mut values = conduit_kernel::HostedValueStore::new(
            2,
            MAX_FACE_COMPOSITION_BYTES as u32,
            (2 * MAX_FACE_COMPOSITION_BYTES) as u32,
        )
        .unwrap();
        assert!(budget(&valid).is_ok());
        assert!(prepare(&valid, &mut values).is_ok());
        let input = FaceComposition::icon("type", "Original item").unwrap();
        let transformed = conduit_semantic_catalog::execute_face_transform(&valid, input).unwrap();
        assert_eq!(transformed.items()[0], input.items()[0]);
        assert_eq!(transformed.items().last().unwrap().name(), "x".repeat(64));

        let mut forged = placement(kind, &"x".repeat(65));
        assert!(conduit_core::face_data_label(&forged.configuration).is_ok());
        // The copied Plan metadata cannot widen the installed contract.
        forged.semantic_contract.configuration[0].rule =
            conduit_core::KindConfigurationRule::DataLabel { maximum: 128 };
        assert!(budget(&forged).is_err());
        assert!(prepare(&forged, &mut values).is_err());
    }
}
