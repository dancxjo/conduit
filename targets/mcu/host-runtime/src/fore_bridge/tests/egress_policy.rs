//! Complete cold egress ABI and original-policy pairing, without live authority.
use super::*;
fn abi() -> c::AssignedForeAbi<'static> {
    c::AssignedForeAbi {
        value_kind: c::COUNT_INFO_ID,
        value_contract: None,
        abnormal_kind: None,
        temporal: "flow-open",
        track: c::ConnectionTrack::Payload,
        quantity_profile: c::QuantityValueProfile::Decimal,
        item_capacity: 1,
        byte_capacity: 8,
    }
}
fn output_pin(digest: [u8; 32]) -> c::AssignedLineForeRequirement {
    c::AssignedLineForeRequirement::new(
        c::AssignedForeRequirement::new(
            1,
            1,
            c::AssignedForeDirection::Output,
            c::AssignedIdentity([6; 16]),
            digest,
        ),
        c::AssignedRemoteBinding {
            line: c::AssignedIdentity([1; 16]),
            local_host: c::AssignedIdentity([2; 16]),
            local_boot: c::AssignedIdentity([3; 16]),
            peer_host: c::AssignedIdentity([4; 16]),
            peer_boot: c::AssignedIdentity([5; 16]),
        },
        c::AssignedIdentity([7; 16]),
        c::AssignedIdentity([8; 16]),
        [9; 32],
    )
}
#[test]
fn every_claimed_output_abi_field_is_pinned_and_unsupported_shapes_refuse() {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let checked = EgressSpec::checked(&pins.lines[1], abi()).unwrap();
    assert_eq!(checked.pin(), &pins.lines[1]);
    assert_eq!(checked.byte_capacity(), 8);
    let reject = |candidate| {
        assert!(matches!(
            EgressSpec::checked(&pins.lines[1], candidate),
            Err(Refusal::UnsupportedAbi)
        ))
    };
    let mut candidate = abi();
    candidate.byte_capacity = 7;
    reject(candidate);
    let mut candidate = abi();
    candidate.value_kind = c::TEXT_INFO_ID;
    reject(candidate);
    let mut candidate = abi();
    candidate.temporal = "flow-closing";
    reject(candidate);
    let mut candidate = abi();
    candidate.quantity_profile = c::QuantityValueProfile::Exact;
    reject(candidate);
    let contract = c::CheckedValueContract::new(
        c::COUNT_INFO_ID.into(),
        8,
        vec![c::ValueConstraint::ByteLength {
            minimum: 8,
            maximum: 8,
        }],
    )
    .unwrap();
    let mut candidate = abi();
    candidate.value_contract = Some(&contract);
    reject(candidate);
    assert!(matches!(
        EgressSpec::checked(&pins.lines[0], abi()),
        Err(Refusal::UnsupportedAbi)
    ));
    // Even an EXACT digest cannot enable unsupported control/abnormal profiles
    // or change the bridge's explicit one-item payload window.
    for shape in 0..5 {
        let make = || {
            let mut candidate = abi();
            match shape {
                0 => candidate.abnormal_kind = Some("conduit.fixture/failure@1"),
                1 => candidate.track = c::ConnectionTrack::NormalClose,
                2 => candidate.item_capacity = 2,
                3 => candidate.item_capacity = 0,
                _ => candidate.byte_capacity = 0,
            }
            candidate
        };
        let pin = output_pin(c::assigned_fore_abi_digest(make()));
        assert!(matches!(
            EgressSpec::checked(&pin, make()),
            Err(Refusal::UnsupportedAbi)
        ));
    }
}
fn cold_contract() -> c::CheckedValueContract {
    // Native structured Back validation is outside this metadata-only fixture.
    // The complete trusted structured contract still belongs in the ABI hash.
    c::CheckedValueContract::new(
        "conduit.fixture/structured-read-result@1".into(),
        837,
        vec![c::ValueConstraint::ByteLength {
            minimum: 1,
            maximum: 837,
        }],
    )
    .unwrap()
}
#[test]
fn checked_structured_output_retires_all_cold_metadata() {
    let pin = {
        let contract = cold_contract();
        let kind = contract.value_kind.0.clone();
        output_pin(c::assigned_fore_abi_digest(c::AssignedForeAbi {
            value_kind: &kind,
            value_contract: Some(&contract),
            byte_capacity: 837,
            ..abi()
        }))
    };
    let spec = {
        let contract = cold_contract();
        let kind = contract.value_kind.0.clone();
        let spec = EgressSpec::checked(
            &pin,
            c::AssignedForeAbi {
                value_kind: &kind,
                value_contract: Some(&contract),
                byte_capacity: 837,
                ..abi()
            },
        )
        .unwrap();
        drop(kind);
        drop(contract);
        spec // Return lifetime is tied ONLY to pin, never cold contract/kind.
    };
    assert_eq!(spec.byte_capacity(), 837);
    assert_eq!(spec.pin(), &pin);
}
#[test]
fn original_policy_can_be_checked_before_admission_and_again_after_bind() {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let changed = Pins::new(c::TEXT_INFO_ID);
    let (input_spec, output_spec) = pins.specs(c::COUNT_INFO_ID);
    let mut input = [0; 8];
    let mut output = [0; 8];
    let prepared =
        PreparedForeBridge::prepare([4; 32], input_spec, output_spec, &mut input, &mut output)
            .unwrap();
    assert_eq!(
        prepared.check_original_policy(&[4; 32], &pins.lines[0], &pins.lines[1]),
        Ok(())
    );
    assert_eq!(
        prepared.check_original_policy(&[5; 32], &pins.lines[0], &pins.lines[1]),
        Err(Refusal::Scope)
    );
    assert_eq!(
        prepared.check_original_policy(&[4; 32], &changed.lines[0], &pins.lines[1]),
        Err(Refusal::Scope)
    );
    assert_eq!(
        prepared.check_original_policy(&[4; 32], &pins.lines[0], &changed.lines[1]),
        Err(Refusal::Scope)
    );
    assert_eq!(
        prepared.check_original_policy(&[4; 32], &pins.lines[1], &pins.lines[0]),
        Err(Refusal::Scope)
    );
    // No provider or network IO in this explicit unit fixture.
    let bound = unsafe { prepared.bind() };
    assert_eq!(
        bound.check_original_policy(&[4; 32], &pins.lines[0], &pins.lines[1]),
        Ok(())
    );
    assert_eq!(
        bound.check_original_policy(&[4; 32], &pins.lines[0], &changed.lines[1]),
        Err(Refusal::Scope)
    );
    let (_observer, port) = bound.activate(pins.activation()).unwrap();
    port.stage_ingress(0, &[0; 8]).unwrap();
}
