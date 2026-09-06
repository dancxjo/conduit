use conduit_data::{Natural, NaturalRefusal};

fn encoded(value: u128) -> Vec<u8> {
    let mut bytes = value.to_le_bytes().to_vec();
    while bytes.len() > 1 && bytes.last() == Some(&0) {
        bytes.pop();
    }
    bytes
}

#[test]
fn canonical_identity_and_zero_are_exact() {
    for invalid in [&[][..], &[0, 0], &[1, 0], &[255, 0]] {
        assert_eq!(
            Natural::from_bytes(invalid),
            Err(NaturalRefusal::NonCanonical)
        );
    }
    assert!(Natural::from_bytes(&[0]).unwrap().is_zero());
    assert!(!Natural::from_bytes(&[0, 1]).unwrap().is_zero());
    let mut output = [91; 3];
    let length = Natural::from_bytes(&[0])
        .unwrap()
        .predecessor(&mut output)
        .unwrap();
    assert_eq!(length, 1);
    assert_eq!(output, [0, 91, 91]);
}

#[test]
fn arithmetic_matches_independent_fixed_width_reference_where_it_fits() {
    for value in 0..=65536u128 {
        let input = encoded(value);
        let natural = Natural::from_bytes(&input).unwrap();
        let mut output = [197; 17];
        let length = natural.successor(&mut output).unwrap();
        assert_eq!(&output[..length], encoded(value + 1));
        assert!(output[length..].iter().all(|byte| *byte == 197));
        output.fill(197);
        let length = natural.predecessor(&mut output).unwrap();
        assert_eq!(&output[..length], encoded(value.saturating_sub(1)));
        assert!(output[length..].iter().all(|byte| *byte == 197));
    }
}

#[test]
fn capacity_exhaustion_is_atomic_and_larger_embodiment_preserves_the_prefix() {
    let mut small = [0; 1];
    let mut large = [0; 3];
    let mut small_len = 1;
    let mut large_len = 1;
    for _ in 0..255 {
        let mut next_small = [42; 1];
        let mut next_large = [42; 3];
        small_len = Natural::from_bytes(&small[..small_len])
            .unwrap()
            .successor(&mut next_small)
            .unwrap();
        large_len = Natural::from_bytes(&large[..large_len])
            .unwrap()
            .successor(&mut next_large)
            .unwrap();
        assert_eq!(&next_small[..small_len], &next_large[..large_len]);
        small = next_small;
        large = next_large;
    }
    let mut refused = [77; 1];
    assert_eq!(
        Natural::from_bytes(&small).unwrap().successor(&mut refused),
        Err(NaturalRefusal::CapacityExhausted)
    );
    assert_eq!(refused, [77]);
    let mut next = [77; 3];
    assert_eq!(
        Natural::from_bytes(&large[..large_len])
            .unwrap()
            .successor(&mut next),
        Ok(2)
    );
    assert_eq!(next, [0, 1, 77]);
    assert_eq!(small, [255]);
}

#[test]
fn magnitude_extends_beyond_machine_integer_widths() {
    let input = [255; 128];
    let mut output = [17; 130];
    assert_eq!(
        Natural::from_bytes(&input).unwrap().successor(&mut output),
        Ok(129)
    );
    assert_eq!(&output[..128], &[0; 128]);
    assert_eq!(output[128..], [1, 17]);
    let mut restored = [0; 128];
    assert_eq!(
        Natural::from_bytes(&output[..129])
            .unwrap()
            .predecessor(&mut restored),
        Ok(128)
    );
    assert_eq!(restored, input);
}

#[test]
fn predecessor_admits_only_its_actual_output_size() {
    let value = Natural::from_bytes(&[0, 1]).unwrap();
    assert_eq!(value.predecessor(&mut [0; 1]), Ok(1));
    let mut too_small = [73; 1];
    assert_eq!(
        Natural::from_bytes(&[1, 1])
            .unwrap()
            .predecessor(&mut too_small),
        Err(NaturalRefusal::CapacityExhausted)
    );
    assert_eq!(too_small, [73]);
    assert_eq!(
        value.successor(&mut []),
        Err(NaturalRefusal::CapacityExhausted)
    );
}
