use super::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue, PlannedGear};

fn placement(offer: CapabilityOffer) -> PlannedGear {
    conduit_core::planned_gear_from_parts! {
        realization_properties: offer.realization_properties,
        semantic_contract: Default::default(),
        placement_id: "label-placement".into(),
        gear_id: "labelled-data".into(),
        kind_id: offer.kind_id,
        kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry {
            key: "label".into(),
            value: ConfigurationValue::Text("Declared data name".into()),
        }],
        host_id: "browser/labels".into(),
        boot_id: "browser-boot/labels".into(),
        offer_generation: OfferGeneration(1),
        capability_id: offer.capability_id,
        implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        base: None,
        realization_characteristics: Vec::new(),
        limits: offer.limits,
        inputs: offer.inputs,
        outputs: offer.outputs,
        terminal_transductions: Vec::new(),
        host_calls: offer.host_calls,
        resources: Vec::new(),
        authority: Vec::new(),
        pool_references: Vec::new(),
    }
}

#[test]
fn every_installed_face_requires_its_declared_name_at_prepare_boundary() {
    let mut tested = 0;
    for installation in INSTALLATIONS {
        let offer = (installation.offer)();
        if !conduit_semantic_catalog::face_data_label_required(offer.kind_id.as_str()) {
            continue;
        }
        assert!(
            offer
                .startup_parameters
                .iter()
                .any(|parameter| { parameter.name == "label" && !parameter.has_default }),
            "{} must advertise a required label",
            offer.kind_id.as_str()
        );
        let mut placement = placement(offer.clone());
        validate_placement(&placement, &offer).unwrap();
        placement.configuration.clear();
        assert!(
            validate_placement(&placement, &offer).is_err(),
            "{} refused an anonymous forged plan",
            offer.kind_id.as_str()
        );
        tested += 1;
    }
    assert!(tested >= 10, "cover the installed Face families");
}

#[test]
fn typed_leaf_faces_preserve_names_separately_from_canonical_values() {
    let cases = [
        (conduit_semantic_catalog::TEXT_FACE_KIND, b"ROOM".to_vec()),
        (
            conduit_semantic_catalog::COUNT_FACE_KIND,
            conduit_core::encode_count(7).to_vec(),
        ),
        (
            conduit_semantic_catalog::BOOL_FACE_KIND,
            conduit_core::InfoBool::TRUE.encode().to_vec(),
        ),
        (
            conduit_semantic_catalog::BOOL_VALUE_FACE_KIND,
            conduit_core::InfoBool::FALSE.encode().to_vec(),
        ),
        (
            conduit_semantic_catalog::SCALAR_VALUE_FACE_KIND,
            conduit_core::Scalar::from_raw_microunits(125_000)
                .encode()
                .to_vec(),
        ),
        (
            conduit_semantic_catalog::TICK_FACE_KIND,
            conduit_time::encode_tick(42).to_vec(),
        ),
    ];
    for (kind, canonical) in cases {
        let installation = INSTALLATIONS
            .iter()
            .find(|installation| (installation.offer)().kind_id.as_str() == kind)
            .unwrap();
        let placement = placement((installation.offer)());
        let result = installation.perform.unwrap()(&placement, &canonical).unwrap();
        let manifestation = result.manifestation.unwrap();
        assert_eq!(
            manifestation.data_label.as_deref(),
            Some("Declared data name"),
            "{kind}"
        );
        assert_eq!(manifestation.canonical_value, canonical, "{kind}");
    }
}
