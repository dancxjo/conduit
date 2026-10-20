//! Recognize v5 computation definitions without granting unsupported consumption.
//! Retain exact v4 authored Types and replay their existing shared CPE unchanged.
use conduit_core::{projection::*, *};
use conduit_plot::*;
use conduit_semantic_catalog::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn case(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!()
    };
    cases
        .iter()
        .find(|c| c.tag() == name)
        .unwrap()
        .payload_type()
        .clone()
}
fn historical() -> StructuredInfoType {
    let source = include_str!("fixtures/numerical-types-near-v4.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    checked
        .native_types
        .into_iter()
        .find(|t| t.name == "NearQuantity")
        .unwrap()
        .value_type
}
fn leaf(ty: StructuredInfoType, bytes: Vec<u8>) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty, bytes).unwrap()
}
fn number(ty: StructuredInfoType, q: ExactRational) -> StructuredInfoValue {
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = MathematicalValue::rational(q).encode(&mut bytes).unwrap();
    bytes.truncate(n);
    leaf(ty, bytes)
}
fn record(ty: StructuredInfoType, fields: Vec<(&str, StructuredInfoValue)>) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty,
        fields
            .into_iter()
            .map(|(k, v)| StructuredFieldValue::new(k, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn tag(ty: StructuredInfoType, name: &str) -> StructuredInfoValue {
    let payload = case(&ty, name);
    StructuredInfoValue::variant(ty, name, leaf(payload, vec![])).unwrap()
}
fn exact_frame(ty: StructuredInfoType, computed: bool) -> Vec<u8> {
    let wire = Quantity::parse_plot_literal("0.1Hz").unwrap().encode();
    let requested = Quantity::parse_plot_literal("0.2Hz").unwrap();
    let source =
        RationalQuantity::from_quantity(Quantity::decode(&wire).unwrap(), &mut work()).unwrap();
    let limit = RationalQuantity::from_quantity(requested, &mut work()).unwrap();
    let domain = ExactNearProjectionDomain;
    let policy = ExactNearProjectionPolicy {
        maximum_error: limit,
    };
    let ids = NearProjectionContentIds::new(&wire, source, requested, None);
    let facts = domain.facts();
    let report = ProjectionReport::new(
        &domain,
        &policy,
        ids.input(&domain, &source, &source, &facts),
    )
    .unwrap();
    let mut evidence = vec![0; RATIONAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES];
    let n = encode_projection_evidence(
        &report,
        &ExactNearEvidenceCodec::new(policy),
        RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
        &mut evidence,
    )
    .unwrap();
    evidence.truncate(n);
    let rep = field(&ty, "representation");
    let representation = if computed {
        let p = case(&rep, "binary64_computation");
        let value = record(
            p.clone(),
            vec![
                (
                    "target",
                    leaf(field(&p, "target"), IeeeF64::from_bits(0).encode().to_vec()),
                ),
                ("rounding", tag(field(&p, "rounding"), "nearest_ties_even")),
                ("program", leaf(field(&p, "program"), vec![])),
                ("incoming", leaf(field(&p, "incoming"), vec![])),
            ],
        );
        StructuredInfoValue::variant(rep, "binary64_computation", value).unwrap()
    } else {
        let p = case(&rep, "exact_rational");
        StructuredInfoValue::variant(rep, "exact_rational", number(p, source.coordinate())).unwrap()
    };
    record(
        ty.clone(),
        vec![
            ("source", number(field(&ty, "source"), source.coordinate())),
            (
                "source-wire",
                leaf(field(&ty, "source-wire"), wire.to_vec()),
            ),
            (
                "unit",
                leaf(field(&ty, "unit"), source.unit().encode().to_vec()),
            ),
            ("role", tag(field(&ty, "role"), "linear")),
            (
                "requested-limit",
                leaf(field(&ty, "requested-limit"), requested.encode().to_vec()),
            ),
            ("limit", number(field(&ty, "limit"), limit.coordinate())),
            (
                "maximum-error",
                number(field(&ty, "maximum-error"), ExactRational::ZERO),
            ),
            ("claim", tag(field(&ty, "claim"), "exact_distance")),
            (
                "projection-evidence",
                leaf(field(&ty, "projection-evidence"), evidence),
            ),
            ("representation", representation),
        ],
    )
    .canonical_bytes()
    .unwrap()
}
#[test]
fn v4_and_v5_exact_records_replay_without_retagging_their_pinned_schema() {
    let old = historical();
    let current = near_quantity_type();
    assert_ne!(old, current);
    assert_eq!(NUMERICAL_NEAR_REVISION, "numeric/quantity-near@5");
    near_quantity_semantic_contract().validate().unwrap();
    let limit = RationalQuantity::from_quantity(
        Quantity::parse_plot_literal("0.2Hz").unwrap(),
        &mut work(),
    )
    .unwrap();
    for schema in [&old, &current] {
        let bytes = exact_frame(schema.clone(), false);
        let verifier = PreparedNearQuantityVerifier::new(schema).unwrap();
        let result = verifier
            .verify(&bytes, FREQUENCY_INFO_ID, limit, None, &mut work())
            .unwrap();
        assert_eq!(
            result.representation(),
            VerifiedNearRepresentation::ExactRational
        );
        assert_eq!(
            result.source().coordinate(),
            ExactRational::ratio(1, 10, &mut work()).unwrap()
        );
        let other = if schema == &old { &current } else { &old };
        assert_eq!(
            PreparedNearQuantityVerifier::new(other).unwrap().verify(
                &bytes,
                FREQUENCY_INFO_ID,
                limit,
                None,
                &mut work()
            ),
            Err(NearQuantityEvidenceRefusal::Schema)
        );
    }
}
#[test]
fn a_recognized_computation_schema_is_not_a_numerical_witness() {
    let schema = near_quantity_type();
    let bytes = exact_frame(schema.clone(), true);
    let limit = RationalQuantity::from_quantity(
        Quantity::parse_plot_literal("0.2Hz").unwrap(),
        &mut work(),
    )
    .unwrap();
    assert_eq!(
        PreparedNearQuantityVerifier::new(&schema).unwrap().verify(
            &bytes,
            FREQUENCY_INFO_ID,
            limit,
            None,
            &mut work()
        ),
        Err(NearQuantityEvidenceRefusal::UnsupportedRepresentation)
    );
}

#[test]
fn computation_definitions_require_binary64_and_both_retained_artifacts() {
    for altered in [
        NUMERICAL_TYPES_SOURCE.replace(
            "target: F64 finite\n        rounding: NumericalRounding\n        program:",
            "target: F32 finite\n        rounding: NumericalRounding\n        program:",
        ),
        NUMERICAL_TYPES_SOURCE.replace("        incoming: Bytes <= 131072B\n", ""),
    ] {
        assert_ne!(altered, NUMERICAL_TYPES_SOURCE);
        let checked =
            check_syntax_document(&parse_syntax_document(&altered), &StartupCatalog::new())
                .unwrap();
        let malformed = checked
            .native_types
            .into_iter()
            .find(|t| t.name == "NearQuantity")
            .unwrap()
            .value_type;
        assert!(matches!(
            PreparedNearQuantityVerifier::new(&malformed),
            Err(NearQuantityEvidenceRefusal::Schema)
        ));
    }
}
