use conduit_core::{
    kind_id, port_id, BoundedCollectSemanticLaw, CapabilityLimits, CheckedValueContract,
    FrontValueContract, FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, PortDescriptor,
    PortDirection, PortTemporal, PreparedLeafSequenceEncoder,
};

fn contract() -> Kind {
    let element = CheckedValueContract::new(kind_id("test/item"), 16, vec![]).unwrap();
    let encoder = PreparedLeafSequenceEncoder::new(element.value_kind.clone(), 16, 3).unwrap();
    let collection = CheckedValueContract::new(
        encoder
            .value_type()
            .unwrap()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        encoder.maximum_bytes(),
        vec![],
    )
    .unwrap();
    let overflow =
        CheckedValueContract::new(kind_id("test/collection-overflow"), 1, vec![]).unwrap();
    let input = port_id("values");
    let output = port_id("collected");
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("test/collect"),
        kind_contract_revision: KindIdentity::from("test/collect@1"),
        inputs: vec![PortDescriptor {
            port_id: input.clone(),
            value_kind: element.value_kind.clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: output.clone(),
            value_kind: collection.value_kind.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: Some(overflow.value_kind.clone()),
        }],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(input.clone()),
                    contract: element.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(output.clone()),
                    contract: collection.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::OutputAbnormal(output.clone()),
                    contract: overflow.clone(),
                },
            ]),
            KindSemanticLaw::BoundedCollect(BoundedCollectSemanticLaw {
                input_port_id: input,
                output_port_id: output,
                element,
                collection,
                maximum_items: 3,
                overflow_disposition: overflow,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 5,
            max_queue_bytes: 256,
        },
    }
}

#[test]
fn bounded_collect_requires_a_closing_flow_and_one_value_output() {
    contract().validate().unwrap();

    let mut open = contract();
    open.inputs[0].temporal = PortTemporal::Flow { closes: false };
    assert!(open.validate().is_err());

    let mut flow_output = contract();
    flow_output.outputs[0].temporal = PortTemporal::Flow { closes: true };
    assert!(flow_output.validate().is_err());
}

#[test]
fn bounded_collect_seals_maximum_output_and_overflow_disposition() {
    let mut zero = contract();
    let KindSemanticLaw::BoundedCollect(law) = &mut zero.semantic_laws[1] else {
        unreachable!()
    };
    law.maximum_items = 0;
    assert!(zero.validate().is_err());

    let mut wrong_collection = contract();
    let KindSemanticLaw::BoundedCollect(law) = &mut wrong_collection.semantic_laws[1] else {
        unreachable!()
    };
    law.collection.maximum_bytes += 1;
    assert!(wrong_collection.validate().is_err());

    let mut wrong_overflow = contract();
    wrong_overflow.outputs[0].abnormal_kind = Some(kind_id("test/other-overflow"));
    assert!(wrong_overflow.validate().is_err());
}

#[test]
fn bounded_collect_law_cannot_be_duplicated() {
    let mut duplicate = contract();
    duplicate
        .semantic_laws
        .push(duplicate.semantic_laws[1].clone());
    assert!(duplicate.validate().is_err());
}
