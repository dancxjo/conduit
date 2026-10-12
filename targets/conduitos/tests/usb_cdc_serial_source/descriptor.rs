use super::*;

#[test]
fn exact_descriptor_profile_checks_every_reviewed_octet_in_both_evaluators() {
    fn completion(ty: &StructuredInfoType, descriptor: &[u8]) -> Vec<u8> {
        let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
            panic!()
        };
        let payload_type = cases
            .iter()
            .find(|case| case.tag() == "completed")
            .unwrap()
            .payload_type();
        let StructuredInfoTypeShape::Record { fields, .. } = payload_type.shape() else {
            panic!()
        };
        let payload = StructuredInfoValue::record(
            payload_type.clone(),
            fields
                .iter()
                .map(|field| {
                    let value = match field.name() {
                        "transferred" => StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            (descriptor.len() as u16).to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                        "short" => {
                            StructuredInfoValue::leaf(field.value_type().clone(), vec![0]).unwrap()
                        }
                        "input" => {
                            let StructuredInfoTypeShape::Sequence { element, .. } =
                                field.value_type().shape()
                            else {
                                panic!()
                            };
                            StructuredInfoValue::sequence(
                                field.value_type().clone(),
                                descriptor
                                    .iter()
                                    .map(|byte| {
                                        StructuredInfoValue::leaf(element.clone(), vec![*byte])
                                            .unwrap()
                                    })
                                    .collect(),
                            )
                            .unwrap()
                        }
                        _ => panic!(),
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap();
        StructuredInfoValue::variant(ty.clone(), "completed", payload)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    }

    let source = prepared();
    let profile = program(&source, "cdc-acm-topology-supported");
    let descriptor = [
        9, 2, 67, 0, 2, 1, 0, 128, 50, 9, 4, 0, 0, 1, 2, 2, 1, 0, 5, 36, 0, 16, 1, 5, 36, 1, 0, 1,
        4, 36, 2, 2, 5, 36, 6, 0, 1, 7, 5, 131, 3, 16, 0, 16, 9, 4, 1, 0, 2, 10, 0, 0, 0, 7, 5,
        129, 2, 64, 0, 0, 7, 5, 2, 2, 64, 0, 0,
    ];
    let observation = |bytes: &[u8], selections: bool| {
        let completion_type = field(&profile.input_type, "completion");
        record(
            &profile.input_type,
            vec![
                (
                    "completion",
                    StructuredInfoValue::from_canonical_bytes(&completion(completion_type, bytes))
                        .unwrap(),
                ),
                (
                    "selections",
                    leaf(
                        field(&profile.input_type, "selections"),
                        &[u8::from(selections)],
                    ),
                ),
            ],
        )
        .canonical_bytes()
        .unwrap()
    };
    let mut evaluator = conduit_plot::PreparedPortableExpressionEvaluator::new(&profile).unwrap();
    for (bytes, selections, expected) in [
        (descriptor.as_slice(), true, 1),
        (descriptor.as_slice(), false, 0),
        (&descriptor[..66], true, 0),
    ] {
        let input = observation(bytes, selections);
        assert_eq!(profile.evaluate(&input).unwrap(), [expected]);
        assert_eq!(evaluator.evaluate(&input).unwrap(), [expected]);
    }
    for index in 0..descriptor.len() {
        let mut foreign = descriptor;
        foreign[index] ^= 1;
        let input = observation(&foreign, true);
        assert_eq!(
            profile.evaluate(&input).unwrap(),
            [0],
            "interpreted octet {index}"
        );
        assert_eq!(
            evaluator.evaluate(&input).unwrap(),
            [0],
            "prepared octet {index}"
        );
    }
}

#[test]
fn explicit_serial_command_bounds_gate_setup_in_both_evaluators() {
    let source = prepared();
    let accept = program(&source, "cdc-serial-accept");
    let mut evaluator = conduit_plot::PreparedPortableExpressionEvaluator::new(&accept).unwrap();
    let event_type = fields(&accept.input_type)[1].value_type();
    for phase in [0, 8] {
        for (tag, bytes, expected) in [
            ("transmit", vec![], true),
            ("transmit", vec![255; 64], true),
            ("transmit", vec![255; 65], false),
            ("receive", 0u64.to_le_bytes().to_vec(), false),
            ("receive", 64u64.to_le_bytes().to_vec(), true),
            ("receive", 65u64.to_le_bytes().to_vec(), false),
        ] {
            let event = StructuredInfoValue::variant(
                event_type.clone(),
                "command",
                request(payload(event_type, "command"), tag, &bytes),
            )
            .unwrap();
            let current = state(fields(&accept.input_type)[0].value_type(), phase, 0);
            let input = pair(&accept.input_type, &current, &event)
                .canonical_bytes()
                .unwrap();
            for output in [
                accept.evaluate(&input).unwrap(),
                evaluator.evaluate(&input).unwrap().to_vec(),
            ] {
                let value = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
                assert_eq!(
                    scalar(member(&value, "accepted")),
                    [u8::from(expected)],
                    "{tag} phase {phase} len {}",
                    bytes.len()
                );
            }
        }
    }
}
