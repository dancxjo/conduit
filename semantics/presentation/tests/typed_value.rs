//! Canonical typed values retain exact meaning across Face identity and two Masks.
use conduit_core::{
    kind_id, BoundedResourceRef, CheckedValueContract, Quantity, ResourceClassId, ResourceExtent,
    ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, Unit, ValueConstraint,
    QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID, TEMPERATURE_INFO_ID,
};
use conduit_presentation::{
    display_typed_value, plan_face_utterances, render_linear_presentation, spoken_typed_value,
    Presentation, PresentationBasis, PresentationError, PresentationProperty,
    PresentationPropertyValue, PresentationRole, PresentationSubject, MAX_FACE_VALUE_BYTES,
};
fn contract(kind: &str, maximum: u32) -> CheckedValueContract {
    CheckedValueContract::new(kind_id(kind), maximum, vec![]).unwrap()
}
fn quantity(coefficient: i128, exponent: i16) -> PresentationPropertyValue {
    PresentationPropertyValue::TypedValue {
        contract: contract(QUANTITY_INFO_ID, QUANTITY_ENCODED_LEN as u32),
        bytes: Quantity::from_decimal(coefficient, exponent, Unit::Celsius)
            .unwrap()
            .encode()
            .to_vec(),
    }
}
fn face(value: PresentationPropertyValue) -> Result<Presentation, PresentationError> {
    Presentation::new_with_semantics(
        7,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "temperature".into(),
            role: PresentationRole::Info,
            name: "Temperature".into(),
        }],
        vec![],
        vec![PresentationProperty {
            subject: "temperature".into(),
            name: "value".into(),
            value,
        }],
        vec![],
        vec![],
        vec![],
    )
}
#[test]
fn inline_values_admit_only_finite_contracts_and_matching_canonical_bytes() {
    let encoded = Quantity::from_decimal(225, -1, Unit::Celsius)
        .unwrap()
        .encode()
        .to_vec();
    for maximum in [0, MAX_FACE_VALUE_BYTES + 1] {
        assert_eq!(
            face(PresentationPropertyValue::TypedValue {
                contract: contract(QUANTITY_INFO_ID, maximum),
                bytes: encoded.clone()
            }),
            Err(PresentationError::InvalidContent)
        );
    }
    assert!(face(PresentationPropertyValue::TypedValue {
        contract: contract(QUANTITY_INFO_ID, MAX_FACE_VALUE_BYTES),
        bytes: encoded.clone()
    })
    .is_ok());
    assert_eq!(
        face(PresentationPropertyValue::TypedValue {
            contract: contract(QUANTITY_INFO_ID, 19),
            bytes: encoded.clone()
        }),
        Err(PresentationError::InvalidContent)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        face(PresentationPropertyValue::TypedValue {
            contract: contract(QUANTITY_INFO_ID, 21),
            bytes: trailing
        }),
        Err(PresentationError::InvalidContent)
    );
    let mut malformed = encoded;
    malformed[0] = 255;
    assert_eq!(
        face(PresentationPropertyValue::TypedValue {
            contract: contract(QUANTITY_INFO_ID, 20),
            bytes: malformed
        }),
        Err(PresentationError::InvalidContent)
    );
    // An exact value may fit the primitive codec and byte ceiling while still
    // violating its declared application contract: this must fail Face admission.
    let constrained = CheckedValueContract::new(
        kind_id(QUANTITY_INFO_ID),
        22,
        vec![ValueConstraint::CanonicalMembership {
            negated: false,
            members: vec![Quantity::from_decimal(21, 0, Unit::Celsius)
                .unwrap()
                .encode()
                .to_vec()],
        }],
    )
    .unwrap();
    assert_eq!(
        face(PresentationPropertyValue::TypedValue {
            contract: constrained,
            bytes: Quantity::from_decimal(225, -1, Unit::Celsius)
                .unwrap()
                .encode()
                .to_vec()
        }),
        Err(PresentationError::InvalidContent)
    );
}
#[test]
fn inline_quantity_and_resource_reference_are_distinct_semantic_contracts() {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([3; 32]),
        content_profile: kind_id(QUANTITY_INFO_ID),
        access_class: ResourceClassId::from("content/public@1"),
        extent: ResourceExtent {
            bytes: 20,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([4; 32]),
            expires_at: None,
        },
    }
    .encode()
    .unwrap();
    assert!(face(PresentationPropertyValue::Content(reference.clone())).is_ok());
    assert_eq!(
        face(PresentationPropertyValue::TypedValue {
            contract: contract(QUANTITY_INFO_ID, MAX_FACE_VALUE_BYTES),
            bytes: reference
        }),
        Err(PresentationError::InvalidContent)
    );
    let bytes = Quantity::from_decimal(21, 0, Unit::Celsius)
        .unwrap()
        .encode()
        .to_vec();
    assert_eq!(
        face(PresentationPropertyValue::Content(bytes)),
        Err(PresentationError::InvalidContent)
    );
}
#[test]
fn face_identity_binds_the_value_kind_contract_and_exact_coordinate() {
    let baseline = face(quantity(225, -1)).unwrap();
    assert_ne!(baseline.identity, face(quantity(23, 0)).unwrap().identity);
    let mut changed = quantity(225, -1);
    let PresentationPropertyValue::TypedValue {
        contract: changed_contract,
        ..
    } = &mut changed
    else {
        unreachable!()
    };
    changed_contract.maximum_bytes += 1;
    assert_ne!(baseline.identity, face(changed).unwrap().identity);
    let bytes = Quantity::new(21, Unit::Celsius).encode().to_vec();
    let unqualified = face(PresentationPropertyValue::TypedValue {
        contract: contract(QUANTITY_INFO_ID, 22),
        bytes: bytes.clone(),
    })
    .unwrap();
    let temperature = face(PresentationPropertyValue::TypedValue {
        contract: contract(TEMPERATURE_INFO_ID, 22),
        bytes,
    })
    .unwrap();
    assert_ne!(unqualified.identity, temperature.identity);
}
#[test]
fn serde_and_postcard_preserve_the_new_value_and_existing_reference_variants() {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([3; 32]),
        content_profile: kind_id(QUANTITY_INFO_ID),
        access_class: ResourceClassId::from("content/public@1"),
        extent: ResourceExtent {
            bytes: 20,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([4; 32]),
            expires_at: None,
        },
    }
    .encode()
    .unwrap();
    assert_eq!(
        postcard::to_allocvec(&PresentationPropertyValue::Flag(true)).unwrap(),
        vec![5, 1]
    );
    assert_eq!(
        postcard::to_allocvec(&PresentationPropertyValue::Content(reference.clone())).unwrap()[0],
        7
    );
    assert_eq!(postcard::to_allocvec(&quantity(-35, -1)).unwrap()[0], 8);
    for original in [
        face(quantity(-35, -1)).unwrap(),
        face(PresentationPropertyValue::Flag(true)).unwrap(),
        face(PresentationPropertyValue::Content(reference)).unwrap(),
    ] {
        let json = serde_json::to_vec(&original).unwrap();
        let decoded: Presentation = serde_json::from_slice(&json).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, original);
        let binary = postcard::to_allocvec(&original).unwrap();
        let decoded: Presentation = postcard::from_bytes(&binary).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, original);
    }
}
#[test]
fn graphical_and_spoken_wording_keep_exact_negative_and_extreme_decimals() {
    for (coefficient, exponent, number) in [
        (225, -1, "22.5".to_owned()),
        (-35, -1, "-3.5".to_owned()),
        (-1, -2, "-0.01".to_owned()),
        (0, -128, "0".to_owned()),
        (1, 128, format!("1{}", "0".repeat(128))),
        (-1, -128, format!("-0.{}1", "0".repeat(127))),
        (10_i128.pow(38) - 1, 0, "9".repeat(38)),
    ] {
        let value = quantity(coefficient, exponent);
        let PresentationPropertyValue::TypedValue { contract, bytes } = &value else {
            unreachable!()
        };
        assert_eq!(display_typed_value(contract, bytes), format!("{number} °C"));
        assert_eq!(
            spoken_typed_value(contract, bytes),
            format!("{number} degrees Celsius")
        );
        let face = face(value).unwrap();
        let linear = render_linear_presentation(&face).unwrap();
        assert!(linear
            .lines
            .iter()
            .any(|line| line.contains(&format!("{number} °C"))));
        let spoken = plan_face_utterances(&face).unwrap();
        assert!(spoken
            .clauses
            .iter()
            .any(|clause| clause.text.contains(&format!("{number} degrees Celsius"))));
        assert_eq!(spoken.source_face_identity, face.identity.as_str());
        assert_eq!(spoken.source_face_revision, 7);
    }
}

#[test]
fn physical_units_and_dimension_specific_quantities_have_semantic_wording() {
    let unit_contract = contract(
        conduit_core::UNIT_INFO_ID,
        conduit_core::UNIT_ENCODED_LEN as u32,
    );
    for symbol in ["kHz", "°C"] {
        let bytes = Unit::resolve(symbol).unwrap().encode();
        assert_eq!(display_typed_value(&unit_contract, &bytes), symbol);
    }
    let frequency_contract = contract(conduit_core::FREQUENCY_INFO_ID, QUANTITY_ENCODED_LEN as u32);
    let frequency = Quantity::parse_plot_literal("1kHz").unwrap();
    assert_eq!(
        display_typed_value(&frequency_contract, &frequency.encode()),
        "1 kHz"
    );
    assert_eq!(
        display_typed_value(&frequency_contract, &Quantity::new(1, Unit::Meter).encode()),
        "Invalid typed value"
    );
}
