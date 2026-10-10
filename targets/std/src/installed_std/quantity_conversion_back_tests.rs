fn typed_operand(kind: &str, source: &str) -> ConfigurationValue {
    if matches!(
        kind,
        conversion::temperature_difference::KIND | conversion::comparison::DIFFERENCE_KIND
    ) {
        ConfigurationValue::TemperatureDifference(
            ExactTemperatureDifferenceConfigurationValue::parse(source).unwrap(),
        )
    } else {
        ConfigurationValue::Quantity(QuantityConfigurationValue::parse(source).unwrap())
    }
}

use super::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};

fn placement_for(kind: &str, source: &str, target: &str) -> PlannedGear {
    let offer = offer_for(kind).unwrap();
    let contract = conversion::operation_contract(kind).unwrap();
    let first = contract.configuration[0].key.clone();
    let second = contract.configuration[1].key.clone();
    conduit_core::planned_gear_from_parts! {
        semantic_contract: contract.semantic_contract(),
        placement_id: PlacementId::from("quantity-placement"), gear_id: GearId::from("quantity"),
        kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry { key: first, value: typed_operand(kind, source) },
            ConfigurationEntry { key: second, value: if matches!(kind, conversion::comparison::KIND | conversion::comparison::DIFFERENCE_KIND) { typed_operand(kind, target) } else { ConfigurationValue::Unit(UnitConfigurationValue::parse(target).unwrap()) } }],
        host_id: HostId::from("quantity-host"), boot_id: BootId::from("quantity-boot"), offer_generation: OfferGeneration(1),
        capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id, artifact_id: offer.implementation.artifact_id,
        base: None, realization_characteristics: Vec::new(), limits: offer.limits,
        inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: Vec::new(), host_calls: offer.host_calls,
        resources: Vec::new(), authority: Vec::new(), pool_references: Vec::new(),
    }
}

fn placements() -> [PlannedGear; 4] {
    [
        placement_for(conversion::KIND, "1Qm", "qm"),
        placement_for(conversion::temperature_difference::KIND, "9°F", "K"),
        placement_for(conversion::comparison::KIND, "1000mm", "0.001km"),
        placement_for(conversion::comparison::DIFFERENCE_KIND, "9°F", "5K"),
    ]
}

#[test]
fn exact_offer_and_finite_configuration_are_required_before_preparation() {
    for correct in placements() {
        assert!(admitted(&correct).is_ok());
        let mut artifact = correct.clone();
        artifact.artifact_id = "foreign/artifact".into();
        let mut front = correct.clone();
        front.outputs[0].value_kind = "value/text".into();
        let mut contract = correct.clone();
        contract.semantic_contract.laws.clear();
        let mut relabeled = correct.clone();
        relabeled.kind_id = if correct.kind_id.as_str() == conversion::KIND {
            conversion::temperature_difference::KIND
        } else {
            conversion::KIND
        }
        .into();
        let mut extra_configuration = correct.clone();
        extra_configuration.configuration.push(ConfigurationEntry {
            key: "offset".into(),
            value: ConfigurationValue::I64(1),
        });
        let mut duplicate = correct.clone();
        duplicate.configuration[1] = duplicate.configuration[0].clone();
        let mut wrong_type = correct.clone();
        wrong_type.configuration[0].value = ConfigurationValue::U64(1);
        let mut revision = correct.clone();
        revision.kind_contract_revision = "foreign/revision".into();
        let mut profile = correct.clone();
        profile.execution_profile_id = "foreign/profile".into();
        let mut implementation = correct.clone();
        implementation.implementation_id = "foreign/implementation".into();
        let mut capability = correct.clone();
        capability.capability_id = "foreign/capability".into();
        let mut temporal = correct.clone();
        temporal.outputs[0].temporal = PortTemporal::Flow { closes: true };
        let mut limits = correct.clone();
        limits.limits.max_queue_items += 1;
        let mut oversized = correct.clone();
        oversized.configuration[0].value = ConfigurationValue::Text("1".repeat(129));
        let mut invalid = correct.clone();
        invalid.configuration[0].value = ConfigurationValue::Text("21C".into());
        for invalid in [
            artifact,
            relabeled,
            extra_configuration,
            front,
            contract,
            duplicate,
            wrong_type,
            revision,
            profile,
            implementation,
            capability,
            temporal,
            limits,
            oversized,
            invalid,
        ] {
            assert!(budget(&invalid).is_err());
            let mut values = HostedValueStore::new(
                2,
                conversion::MAXIMUM_RECEIPT_BYTES,
                conversion::MAXIMUM_RECEIPT_BYTES * 2,
            )
            .unwrap();
            assert!(prepare(&invalid, &mut values).is_err());
        }
    }
}

#[test]
fn prepared_conversion_survives_pressure_and_emits_once_without_allocating() {
    for placement in placements() {
        let expected = admitted(&placement).unwrap();
        let budget = budget(&placement).unwrap();
        let mut values = HostedValueStore::new(
            budget.value_items,
            budget.maximum_value_bytes,
            budget.value_bytes,
        )
        .unwrap();
        let InstalledBack::StructuredLiteral(mut back) = prepare(&placement, &mut values).unwrap()
        else {
            panic!("existing fixed Value Back");
        };
        let input = StepInputBytes::test_frame([None; 1], None);
        let allocations = crate::allocation_probe::begin();
        let mut blocked_correct = true;
        for _ in 0..1000 {
            let mut blocked = StepIo::test_frame([None; 1], [false; 1], [None; 1], None, 8);
            blocked_correct &= back.step(&mut blocked, &input) == StepOutcome::Await;
            blocked_correct &= blocked.test_output(PortId(0)).is_none();
        }
        let mut ready = StepIo::test_frame(
            [None; 1],
            [false; 1],
            [Some(budget.maximum_value_bytes)],
            None,
            8,
        );
        let outcome = back.step(&mut ready, &input);
        let value = ready.test_output(PortId(0));
        let mut next = StepIo::test_frame(
            [None; 1],
            [false; 1],
            [Some(budget.maximum_value_bytes)],
            None,
            8,
        );
        let completed = back.step(&mut next, &input);
        let allocation_count = allocations.finish();
        assert!(blocked_correct);
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(completed, StepOutcome::Complete);
        assert!(next.test_output(PortId(0)).is_none());
        assert_eq!(allocation_count, 0);
        assert_eq!(values.get(value.unwrap()).unwrap(), expected);
    }
}

#[test]
fn cancellation_of_a_pressured_receipt_prevents_delivery() {
    let value = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: 8192,
    };
    let mut back = StructuredLiteralBack::prepared(value);
    let input = StepInputBytes::test_frame([None; 1], None);
    let mut blocked = StepIo::test_frame([None; 1], [false; 1], [None; 1], None, 8);
    assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
    <StructuredLiteralBack as StepBack<1>>::cancel(&mut back);
    let mut ready = StepIo::test_frame([None; 1], [false; 1], [Some(8192)], None, 8);
    assert_eq!(back.step(&mut ready, &input), StepOutcome::Complete);
    assert!(ready.test_output(PortId(0)).is_none());
}

#[test]
fn comparator_play_has_no_allocations_and_refuses_forged_receipts() {
    let mut placement = placement_for(conversion::KIND, "1kHz", "Hz");
    let offer = offer_for(conversion::converted_equals::KIND).unwrap();
    placement.kind_id = offer.kind_id;
    placement.kind_contract_revision = offer.kind_contract_revision;
    placement.capability_id = offer.capability_id;
    placement.execution_profile_id = offer.implementation.execution_profile_id;
    placement.implementation_id = offer.implementation.implementation_id;
    placement.artifact_id = offer.implementation.artifact_id;
    placement.inputs = offer.inputs;
    placement.outputs = offer.outputs;
    placement.limits = offer.limits;
    placement.semantic_contract = conversion::converted_equals::contract().semantic_contract();
    placement.configuration = vec![ConfigurationEntry {
        key: "expected".into(),
        value: ConfigurationValue::Quantity(QuantityConfigurationValue::parse("1000Hz").unwrap()),
    }];
    let receipt = conversion::prepare_operation_configuration(
        conversion::KIND,
        &placement_for(conversion::KIND, "1kHz", "Hz").configuration,
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    for forged in [false, true] {
        let mut bytes = receipt.clone();
        if forged {
            bytes[0] ^= 1;
        }
        let budget = budget(&placement).unwrap();
        let mut values = HostedValueStore::new(
            budget.value_items,
            budget.maximum_value_bytes,
            budget.value_bytes,
        )
        .unwrap();
        let mut back = prepare(&placement, &mut values).unwrap();
        let input = values.store(&bytes).unwrap();
        let inputs = StepInputBytes::test_frame([Some(bytes.as_slice())], None);
        let mut blocked = StepIo::test_frame([Some(input)], [false], [None], None, 8);
        let mut ready = StepIo::test_frame([Some(input)], [false], [Some(1)], None, 8);
        let allocations = crate::allocation_probe::begin();
        let blocked_outcome = back.step(&mut blocked, &inputs);
        let outcome = back.step(&mut ready, &inputs);
        let allocations = allocations.finish();
        assert_eq!(allocations, 0);
        assert_eq!(blocked_outcome, StepOutcome::Await);
        if forged {
            assert!(matches!(outcome, StepOutcome::Fail(_)));
        } else {
            assert_eq!(outcome, StepOutcome::Complete);
            assert_eq!(
                values.get(ready.test_output(PortId(0)).unwrap()).unwrap(),
                InfoBool::TRUE.encode()
            );
        }
    }
}
