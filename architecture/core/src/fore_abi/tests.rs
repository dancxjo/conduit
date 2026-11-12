//! Independent old encoder reference proves the published ABI is unchanged.
use super::*;
use alloc::vec::Vec;
fn previous(abi: AssignedForeAbi<'_>) -> [u8; 32] {
    let mut bytes = Vec::from(&b"conduit.assigned/local-fore-abi@1\0"[..]);
    let mut field = |value: &[u8]| {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    };
    field(abi.value_kind.as_bytes());
    field(
        &abi.value_contract
            .map(CheckedValueContract::identity_bytes)
            .unwrap_or_default(),
    );
    field(abi.abnormal_kind.unwrap_or("").as_bytes());
    field(abi.temporal.as_bytes());
    bytes.extend_from_slice(&[
        match abi.track {
            ConnectionTrack::Payload => 0,
            ConnectionTrack::NormalClose => 1,
            ConnectionTrack::AbnormalTerminal => 2,
            ConnectionTrack::Quiescence => 3,
        },
        match abi.quantity_profile {
            QuantityValueProfile::Decimal => 0,
            QuantityValueProfile::Exact => 1,
        },
    ]);
    bytes.extend_from_slice(&abi.item_capacity.to_le_bytes());
    bytes.extend_from_slice(&abi.byte_capacity.to_le_bytes());
    crate::assigned_plan_payload_digest(&bytes)
}
#[test]
fn streaming_matches_original_transcript_for_all_profiles_and_contract_presence() {
    let contract = CheckedValueContract {
        value_kind: "value/count".into(),
        maximum_bytes: 8,
        constraints: alloc::vec![crate::ValueConstraint::ByteLength {
            minimum: 8,
            maximum: 8
        }],
    };
    for track in [
        ConnectionTrack::Payload,
        ConnectionTrack::NormalClose,
        ConnectionTrack::AbnormalTerminal,
        ConnectionTrack::Quiescence,
    ] {
        for quantity_profile in [QuantityValueProfile::Decimal, QuantityValueProfile::Exact] {
            for value_contract in [None, Some(&contract)] {
                for abnormal_kind in [None, Some("fixture/abnormal")] {
                    let abi = || AssignedForeAbi {
                        value_kind: "value/count",
                        value_contract,
                        abnormal_kind,
                        temporal: "stream",
                        track,
                        quantity_profile,
                        item_capacity: 1,
                        byte_capacity: 8,
                    };
                    assert_eq!(assigned_fore_abi_digest(abi()), previous(abi()));
                }
            }
        }
    }
}
