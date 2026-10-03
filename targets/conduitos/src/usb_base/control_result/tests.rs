use super::*;
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};

#[test]
fn canonical_completed_result_retains_exact_count_shortness_and_octets() {
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let mut setup = [0; 8];
    setup[0] = 128;
    setup[6..].copy_from_slice(&256_u16.to_le_bytes());
    let request = ControlTransferRequest::new(setup, &[], 256).unwrap();
    let bytes: [u8; 256] = core::array::from_fn(|index| index as u8);
    for actual in 0..=256_u16 {
        let encoded = encoder
            .completed(&request, actual, &bytes[..usize::from(actual)])
            .unwrap();
        assert!(encoded.len() <= CONTROL_MAXIMUM_BYTES as usize);
        let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        assert_eq!(value.value_type(), contract.result_type());
        let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            panic!("variant")
        };
        assert_eq!(tag, "completed");
        let StructuredInfoValueShape::Record(fields) = payload.shape() else {
            panic!("completed record")
        };
        let member = |name| {
            fields
                .iter()
                .find(|field| field.name() == name)
                .unwrap()
                .value()
        };
        assert_eq!(
            member("transferred").shape(),
            StructuredInfoValueShape::Leaf(&actual.to_le_bytes())
        );
        assert_eq!(
            member("short").shape(),
            StructuredInfoValueShape::Leaf(&[u8::from(actual < 256)])
        );
        let StructuredInfoValueShape::Collection(octets) = member("input").shape() else {
            panic!("octets")
        };
        assert_eq!(octets.len(), usize::from(actual));
        for (index, octet) in octets.iter().enumerate() {
            assert_eq!(
                octet.shape(),
                StructuredInfoValueShape::Leaf(&[index as u8])
            );
        }
    }
}

#[test]
fn impossible_counts_and_output_input_confusion_are_not_success() {
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let setup = [128, 0, 0, 0, 0, 0, 8, 0];
    let request = ControlTransferRequest::new(setup, &[], 256).unwrap();
    assert_eq!(
        encoder.completed(&request, 9, &[0; 9]),
        Err(ControlResultRefusal::ActualLength)
    );
    assert_eq!(
        encoder.completed(&request, 3, &[0; 2]),
        Err(ControlResultRefusal::InputLength)
    );
    let output_setup = [0, 0, 0, 0, 0, 0, 3, 0];
    let output = ControlTransferRequest::new(output_setup, &[1, 2, 3], 256).unwrap();
    assert_eq!(
        encoder.completed(&output, 2, &[]),
        Err(ControlResultRefusal::ActualLength)
    );
    assert_eq!(
        encoder.completed(&output, 3, &[1]),
        Err(ControlResultRefusal::UnexpectedInputData)
    );
    assert!(encoder.completed(&output, 3, &[]).is_ok());
    let oversized = ControlTransferRequest::new([128, 0, 0, 0, 0, 0, 1, 1], &[], 257).unwrap();
    assert_eq!(
        encoder.completed(&oversized, 257, &[0; 257]),
        Err(ControlResultRefusal::DataEnvelope)
    );
}

#[test]
fn semantic_dispositions_keep_separate_tags_across_repeated_requests() {
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    for _ in 0..1000 {
        for (disposition, expected) in [
            (ControlTransferDisposition::Stalled, "stalled"),
            (ControlTransferDisposition::ProviderLost, "provider-lost"),
            (ControlTransferDisposition::Unsupported, "unsupported"),
        ] {
            let value = StructuredInfoValue::from_canonical_bytes(
                encoder.disposition(disposition).unwrap(),
            )
            .unwrap();
            assert_eq!(value.value_type(), contract.result_type());
            let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
                panic!("variant")
            };
            assert_eq!(tag, expected);
            assert_eq!(payload.shape(), StructuredInfoValueShape::Leaf(&[]));
        }
    }
}
