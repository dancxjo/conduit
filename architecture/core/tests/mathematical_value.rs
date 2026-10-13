use conduit_core::{
    ExactNumericRefusal as Numeric, ExactNumericWork as Work, ExactRational as Q,
    MathematicalComparison as Comparison, MathematicalNode as Node, MathematicalValue as Value,
    MathematicalValueBuilder as Builder, MathematicalValueRefusal as R,
    MathematicalValueView as View, MATHEMATICAL_VALUE_ID, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
    MATHEMATICAL_VALUE_VALIDATION_WORK,
};
fn work() -> Work {
    Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK)
}
fn bytes(value: &Value) -> Vec<u8> {
    let mut result = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let length = value.encode(&mut result).unwrap();
    result.truncate(length);
    result
}
fn pi_over_180(reverse_staging: bool) -> Value {
    let mut builder = Builder::new();
    let mut budget = work();
    let (pi, denominator) = if reverse_staging {
        let d = builder
            .insert(Node::Rational(Q::from_i128(180)), &mut budget)
            .unwrap();
        let p = builder.insert(Node::Pi, &mut budget).unwrap();
        (p, d)
    } else {
        let p = builder.insert(Node::Pi, &mut budget).unwrap();
        let d = builder
            .insert(Node::Rational(Q::from_i128(180)), &mut budget)
            .unwrap();
        (p, d)
    };
    let root = builder
        .insert(Node::Divide(pi, denominator), &mut budget)
        .unwrap();
    builder.finish(root, &mut budget).unwrap()
}
#[test]
fn exact_symbolic_transport_canonicalizes_staging_and_preserves_unknown() {
    let value = pi_over_180(false);
    let encoded = bytes(&value);
    let reordered = pi_over_180(true);
    assert_eq!(encoded, bytes(&reordered));
    assert_eq!(value.semantic_digest(), reordered.semantic_digest());
    let view = View::decode(&encoded, &mut work()).unwrap();
    assert_eq!(view.node_count(), 3);
    assert_eq!(view.semantic_digest(), value.semantic_digest());
    assert_eq!(Value::decode(&encoded, &mut work()).unwrap(), value);
    assert_eq!(view.as_rational(&mut work()).unwrap(), None);
    assert_eq!(
        value.compare(&reordered, &mut work()),
        Ok(Comparison::ProvedEqual)
    );
    let approximate = Value::parse_literal("0.017453292519943295", &mut work()).unwrap();
    assert_eq!(
        value.compare(&approximate, &mut work()),
        Ok(Comparison::Unknown)
    );
    let approximate_bytes = bytes(&approximate);
    let other = View::decode(&approximate_bytes, &mut work()).unwrap();
    assert_eq!(view.compare(&other, &mut work()), Ok(Comparison::Unknown));
    conduit_core::validate_primitive_info(MATHEMATICAL_VALUE_ID, &encoded).unwrap();
}
#[test]
fn rational_arithmetic_folds_to_one_inline_canonical_literal() {
    let mut b = Builder::new();
    let mut w = work();
    let q = Q::ratio(1, 3, &mut w).unwrap();
    let a = b.insert(Node::Rational(q), &mut w).unwrap();
    let two = b.insert(Node::Add(a, a), &mut w).unwrap();
    let one = b.insert(Node::Add(two, a), &mut w).unwrap();
    assert_eq!(b.finish(one, &mut w).unwrap(), Value::rational(Q::ONE));
    let tenth = b
        .insert(
            Node::Rational(Q::parse_decimal("0.1", &mut w).unwrap()),
            &mut w,
        )
        .unwrap();
    let fifth = b
        .insert(
            Node::Rational(Q::parse_decimal("0.2", &mut w).unwrap()),
            &mut w,
        )
        .unwrap();
    let sum = b.insert(Node::Add(tenth, fifth), &mut w).unwrap();
    let sum = b.finish(sum, &mut w).unwrap();
    assert_eq!(sum, Value::parse_literal("0.3", &mut w).unwrap());
    assert_eq!(
        sum.compare(&Value::rational(Q::ONE), &mut w),
        Ok(Comparison::ProvedDifferent)
    );
    assert_eq!(bytes(&sum)[1], 0);
}
#[test]
fn shared_nodes_and_principal_root_domain_are_bounded() {
    let mut b = Builder::new();
    let mut w = work();
    let two = b.insert(Node::Rational(Q::from_i128(2)), &mut w).unwrap();
    let root = b.insert(Node::PrincipalSquareRoot(two), &mut w).unwrap();
    assert_eq!(
        root,
        b.insert(Node::PrincipalSquareRoot(two), &mut w).unwrap()
    );
    let sum = b.insert(Node::Add(root, root), &mut w).unwrap();
    let value = b.finish(sum, &mut w).unwrap();
    let encoded = bytes(&value);
    assert_eq!(View::decode(&encoded, &mut w).unwrap().node_count(), 3);
    let zero = b.insert(Node::Rational(Q::ZERO), &mut w).unwrap();
    assert_eq!(
        b.insert(Node::Divide(root, zero), &mut w),
        Err(R::Numeric(Numeric::DivisionByZero))
    );
    assert_eq!(
        b.insert(Node::Divide(root, root), &mut w),
        Err(R::UnsupportedDomainProof)
    );
    let negative = b.insert(Node::Rational(Q::from_i128(-1)), &mut w).unwrap();
    assert_eq!(
        b.insert(Node::PrincipalSquareRoot(negative), &mut w),
        Err(R::NegativeRadicand)
    );
    assert_eq!(
        b.insert(Node::PrincipalSquareRoot(root), &mut w),
        Err(R::UnsupportedDomainProof)
    );
    let mut deep = root;
    for _ in 0..14 {
        deep = b.insert(Node::Negate(deep), &mut w).unwrap();
    }
    assert_eq!(b.insert(Node::Negate(deep), &mut w), Err(R::DepthBound));
    let mut wide = Builder::new();
    for i in 0..32 {
        wide.insert(Node::Rational(Q::from_i128(i)), &mut w)
            .unwrap();
    }
    assert_eq!(wide.insert(Node::Pi, &mut w), Err(R::NodeBound));
}
#[test]
fn hostile_codec_rejects_cycles_padding_unreachable_and_duplicate_nodes() {
    let encoded = bytes(&pi_over_180(false));
    for end in 0..encoded.len() {
        assert!(View::decode(&encoded[..end], &mut work()).is_err());
    }
    let mut bad = encoded.clone();
    bad.push(0);
    assert!(View::decode(&bad, &mut work()).is_err());
    let mut bad = encoded.clone();
    bad[0] = 2;
    assert!(matches!(
        View::decode(&bad, &mut work()),
        Err(R::UnsupportedVersion)
    ));
    let mut bad = encoded.clone();
    *bad.last_mut().unwrap() = 2;
    assert!(matches!(
        View::decode(&bad, &mut work()),
        Err(R::InvalidReference)
    ));
    for bad in [
        vec![1, 1, 2, 1, 1],
        vec![1, 1, 2, 1, 2, 1],
        vec![1, 1, 0],
        vec![1, 1, 33],
    ] {
        assert!(View::decode(&bad, &mut work()).is_err());
    }
    assert!(matches!(
        View::decode(&encoded, &mut Work::new(0)),
        Err(R::Numeric(Numeric::WorkLimit))
    ));
    // Deterministic byte mutations may denote another legitimate expression;
    // every accepted mutation must still re-encode identically.
    for index in 0..encoded.len() {
        for byte in [0, 1, 2, 7, 31, 255] {
            let mut altered = encoded.clone();
            altered[index] = byte;
            if let Ok(value) = Value::decode(&altered, &mut work()) {
                assert_eq!(bytes(&value), altered);
            }
        }
    }
}

#[test]
fn references_cannot_silently_rebind_to_another_builders_node() {
    let mut first = Builder::new();
    let mut second = Builder::new();
    let mut w = work();
    let pi = first.insert(Node::Pi, &mut w).unwrap();
    second.insert(Node::Rational(Q::ONE), &mut w).unwrap();
    assert_eq!(
        second.insert(Node::Negate(pi), &mut w),
        Err(R::InvalidReference)
    );
    assert_eq!(second.finish(pi, &mut w), Err(R::InvalidReference));
}

#[test]
fn tau_is_exactly_two_pi_and_keeps_one_canonical_encoding() {
    let mut w = work();
    let tau = Value::parse_literal("τ", &mut w).unwrap();
    assert_eq!(tau, Value::parse_literal("tau", &mut w).unwrap());
    let mut builder = Builder::new();
    let two = builder
        .insert(Node::Rational(Q::from_i128(2)), &mut w)
        .unwrap();
    let pi = builder.insert(Node::Pi, &mut w).unwrap();
    let root = builder.insert(Node::Multiply(two, pi), &mut w).unwrap();
    let explicit = builder.finish(root, &mut w).unwrap();
    assert_eq!(bytes(&tau), bytes(&explicit));
    assert_eq!(tau.compare(&explicit, &mut w), Ok(Comparison::ProvedEqual));
    assert_eq!(tau.as_rational(), None);
    let encoded = bytes(&tau);
    assert_eq!(Value::decode(&encoded, &mut w).unwrap(), tau);
    let pi_value = Value::parse_literal("π", &mut w).unwrap();
    assert_eq!(tau.compare(&pi_value, &mut w), Ok(Comparison::Unknown));
}
