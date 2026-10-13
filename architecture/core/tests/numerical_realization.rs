use conduit_core::{
    ExactNumericWork as Work, ExactRational as Q, MathematicalValue as Value,
    NumericalAlgorithm as Algorithm, NumericalRealizationRefusal as R,
    NumericalRealizationRequest as Request, NumericalTarget as Target,
    PreparedNumericalRealization as Prepared,
};
fn work() -> Work {
    Work::new(10_000_000)
}
fn value(s: &str) -> Value {
    Value::parse_literal(s, &mut work()).unwrap()
}
#[test]
fn one_source_has_exact_and_outward_machine_realizations() {
    let source = value("0.3");
    let exact = Prepared::prepare(&source, Algorithm::ExactRational, Q::ONE, &mut work()).unwrap();
    let bounded = Prepared::prepare(&source, Algorithm::OutwardI64, Q::ONE, &mut work()).unwrap();
    assert_eq!(exact.source(), bounded.source());
    assert_eq!(exact.maximum_error(), Q::ZERO);
    assert_eq!(
        bounded.maximum_error(),
        Q::ratio(7, 10, &mut work()).unwrap()
    );
    let Target::OutwardI64(interval) = bounded.target() else {
        panic!()
    };
    assert_eq!(interval.lower().signed().unwrap(), 0);
    assert_eq!(interval.upper().signed().unwrap(), 1);
    assert!(exact.retained_payload_bytes() > bounded.retained_payload_bytes());
    assert!(bounded.spent_limb_work() <= bounded.admitted_limb_work());
    let strict = Request::new(source.clone(), Q::ZERO).unwrap();
    assert_eq!(exact.admits(&strict, &mut work()), Ok(()));
    assert_eq!(
        bounded.admits(&strict, &mut work()),
        Err(R::ToleranceExceeded)
    );
    let loose = Request::new(source, Q::ONE).unwrap();
    assert_eq!(bounded.admits(&loose, &mut work()), Ok(()));
}
#[test]
fn source_range_precision_and_domain_failures_do_not_weaken_meaning() {
    let source = value("0.3");
    let prepared =
        Prepared::prepare(&source, Algorithm::ExactRational, Q::ZERO, &mut work()).unwrap();
    let wrong = Request::new(value("0.4"), Q::ONE).unwrap();
    assert_eq!(prepared.admits(&wrong, &mut work()), Err(R::SourceMismatch));
    assert_eq!(
        Request::new(source, Q::from_i128(-1)),
        Err(R::NegativeTolerance)
    );
    assert_eq!(
        Prepared::prepare(&value("π"), Algorithm::ExactRational, Q::ZERO, &mut work()),
        Err(R::UnsupportedDenotation)
    );
    assert!(Prepared::prepare(
        &value("1000000000000000000000000000000"),
        Algorithm::OutwardI64,
        Q::ONE,
        &mut work()
    )
    .is_err());
    assert!(Prepared::prepare(
        &value("0.3"),
        Algorithm::OutwardI64,
        Q::ONE,
        &mut Work::new(0)
    )
    .is_err());
}

#[test]
fn typed_offer_property_recomputes_and_rejects_forged_certificate() {
    use conduit_core::{
        StructuredConfigurationValue, StructuredInfoValue, StructuredInfoValueShape,
    };
    for algorithm in [Algorithm::ExactRational, Algorithm::OutwardI64] {
        let prepared = Prepared::prepare(&value("0.3"), algorithm, Q::ONE, &mut work()).unwrap();
        let property = prepared.property().unwrap();
        let readmitted = Prepared::from_property(&property, &mut work()).unwrap();
        assert_eq!(readmitted, prepared);
        assert_eq!(readmitted.property().unwrap(), property);
        let decoded =
            StructuredInfoValue::from_canonical_bytes(property.canonical_value()).unwrap();
        let StructuredInfoValueShape::Leaf(bytes) = decoded.shape() else {
            panic!()
        };
        let mut bad = bytes.to_vec();
        // Fabricate spent work or target bytes while keeping a valid typed
        // envelope. The domain must reject it rather than trust the wrapper.
        bad[10] ^= 1;
        let forged = StructuredInfoValue::leaf(decoded.value_type().clone(), bad).unwrap();
        let forged = StructuredConfigurationValue::new(
            property.profile().clone(),
            forged.canonical_bytes().unwrap(),
        )
        .unwrap();
        assert!(Prepared::from_property(&forged, &mut work()).is_err());
        let mut bad = bytes.to_vec();
        *bad.last_mut().unwrap() ^= 1;
        let forged = StructuredInfoValue::leaf(decoded.value_type().clone(), bad).unwrap();
        let forged = StructuredConfigurationValue::new(
            property.profile().clone(),
            forged.canonical_bytes().unwrap(),
        )
        .unwrap();
        assert!(Prepared::from_property(&forged, &mut work()).is_err());
    }
}
