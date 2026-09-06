use conduit_data::{Natural, NaturalDomain, NaturalRefusal};

fn domain(maximum_bytes: usize) -> NaturalDomain {
    NaturalDomain::new(maximum_bytes).unwrap()
}

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
            Natural::from_bytes(domain(2), invalid),
            Err(NaturalRefusal::NonCanonical)
        );
    }
    assert!(Natural::from_bytes(domain(2), &[0]).unwrap().is_zero());
    assert!(!Natural::from_bytes(domain(2), &[0, 1]).unwrap().is_zero());
    let mut output = [91; 3];
    let length = Natural::from_bytes(domain(2), &[0])
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
        let natural = Natural::from_bytes(domain(17), &input).unwrap();
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
        small_len = Natural::from_bytes(domain(2), &small[..small_len])
            .unwrap()
            .successor(&mut next_small)
            .unwrap();
        large_len = Natural::from_bytes(domain(3), &large[..large_len])
            .unwrap()
            .successor(&mut next_large)
            .unwrap();
        assert_eq!(&next_small[..small_len], &next_large[..large_len]);
        small = next_small;
        large = next_large;
    }
    let mut refused = [77; 1];
    assert_eq!(
        Natural::from_bytes(domain(2), &small)
            .unwrap()
            .successor(&mut refused),
        Err(NaturalRefusal::CapacityExhausted)
    );
    assert_eq!(refused, [77]);
    let mut next = [77; 3];
    assert_eq!(
        Natural::from_bytes(domain(3), &large[..large_len])
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
        Natural::from_bytes(domain(130), &input)
            .unwrap()
            .successor(&mut output),
        Ok(129)
    );
    assert_eq!(&output[..128], &[0; 128]);
    assert_eq!(output[128..], [1, 17]);
    let mut restored = [0; 128];
    assert_eq!(
        Natural::from_bytes(domain(130), &output[..129])
            .unwrap()
            .predecessor(&mut restored),
        Ok(128)
    );
    assert_eq!(restored, input);
}

#[test]
fn predecessor_admits_only_its_actual_output_size() {
    let value = Natural::from_bytes(domain(2), &[0, 1]).unwrap();
    assert_eq!(value.predecessor(&mut [0; 1]), Ok(1));
    let mut too_small = [73; 1];
    assert_eq!(
        Natural::from_bytes(domain(2), &[1, 1])
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

#[test]
fn semantic_domain_overflow_is_distinct_from_realization_exhaustion() {
    assert_eq!(NaturalDomain::new(0), Err(NaturalRefusal::DomainOverflow));
    assert_eq!(
        Natural::from_bytes(domain(1), &[0, 1]),
        Err(NaturalRefusal::DomainOverflow)
    );

    let value = Natural::from_bytes(domain(1), &[255]).unwrap();
    let mut unchanged = [73; 2];
    assert_eq!(
        value.successor(&mut unchanged),
        Err(NaturalRefusal::DomainOverflow)
    );
    assert_eq!(unchanged, [73; 2]);

    let value = Natural::from_bytes(domain(2), &[255]).unwrap();
    let mut too_small = [81; 1];
    assert_eq!(
        value.successor(&mut too_small),
        Err(NaturalRefusal::CapacityExhausted)
    );
    assert_eq!(too_small, [81]);

    let mut sufficient = [81; 2];
    assert_eq!(value.successor(&mut sufficient), Ok(2));
    assert_eq!(sufficient, [0, 1]);
}
