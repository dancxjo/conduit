#![cfg(feature = "semantic-bindings")]
//! C1: exercise the existing native specification family, not a replacement IR.
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::semantic::*;

fn phone(id: &str) -> PhoneId {
    PhoneId::new(id.into()).unwrap()
}

#[test]
fn all_six_states_retain_payload_order_and_exact_confidence_through_bytes() {
    let states = [
        PhoneSpecification::known(phone("inventory/underlying-t")).unwrap(),
        PhoneSpecification::unknown(),
        PhoneSpecification::unspecified(),
        PhoneSpecification::not_applicable(),
        PhoneSpecification::variable(
            BoundedSequence::try_from_iter([phone("inventory/tap"), phone("inventory/stop")])
                .unwrap(),
        )
        .unwrap(),
        PhoneSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.375)).unwrap(),
            phone("inventory/tap"),
        )
        .unwrap(),
    ];
    let encoded: Vec<_> = states
        .iter()
        .map(|state| state.clone().encode().unwrap())
        .collect();
    for (index, bytes) in encoded.iter().enumerate() {
        let decoded = PhoneSpecification::decode(bytes).unwrap();
        assert_eq!(decoded, states[index]);
        assert_eq!(decoded.clone().encode().unwrap(), *bytes);
        let original = states[index].clone().into_structured().unwrap();
        assert_eq!(decoded.into_structured().unwrap(), original);
        assert!(encoded[..index].iter().all(|earlier| earlier != bytes));
    }
    let reversed = PhoneSpecification::variable(
        BoundedSequence::try_from_iter([phone("inventory/stop"), phone("inventory/tap")]).unwrap(),
    )
    .unwrap();
    assert_ne!(reversed, states[4]);
    // A specification is not an IPA decoder or a membership receipt.
    assert!(PhonemeSpecification::decode(&encoded[0]).is_err());
}

#[test]
fn conditional_payloads_refuse_empty_excess_and_invalid_confidence() {
    assert!(PhoneSpecification::variable(BoundedSequence::new()).is_err());
    let excess = BoundedSequence::try_from_iter((0..5).map(|i| phone(&format!("p/{i}"))));
    if let Ok(excess) = excess {
        assert!(PhoneSpecification::variable(excess).is_err());
    }
    for value in [-0.01, 1.01, f32::NAN, f32::INFINITY] {
        assert!(SpeechConfidence::new(IeeeF32::from_value(value)).is_err());
    }
    for value in [0.0, 1.0] {
        let state = PhoneSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(value)).unwrap(),
            phone("inventory/tap"),
        )
        .unwrap();
        assert_eq!(
            PhoneSpecification::decode(&state.clone().encode().unwrap()).unwrap(),
            state
        );
    }
}
