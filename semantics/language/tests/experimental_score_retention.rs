//! Standalone synthetic expression conformance; not an adopted decoder profile.
#[path = "common/parser_fixture.rs"]
mod values;
use conduit_core::*;
use conduit_plot::{
    rust_binding::{validate_native_contracts, validate_native_invariants},
    *,
};
use values::{field, field_type, record, text};
struct Prepared {
    native: CheckedNativeType,
    program: PortableExpressionProgram,
}
impl Prepared {
    fn new() -> Self {
        let checked = check_syntax_document(
            &parse_syntax_document(include_str!("../experiments/score_retention.conduit")),
            &StartupCatalog::new(),
        )
        .unwrap();
        let expanded = expand_canonical_plot_for_authoring(
            &checked,
            "experimental/retain-score-margin",
            &ProfileCatalog::new(),
        )
        .unwrap();
        let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!("pure expression")
        };
        Self {
            native: checked
                .native_types
                .iter()
                .find(|n| n.name == "ExperimentalRetentionInput")
                .unwrap()
                .clone(),
            program: PortableExpressionProgram::from_canonical_hex(hex).unwrap(),
        }
    }
    fn input(
        &self,
        active: [bool; 4],
        scores: [i64; 4],
        counts: [i64; 4],
        margin: (i64, i64),
        profile: &str,
        calibration: &str,
    ) -> StructuredInfoValue {
        let ty = &self.native.value_type;
        let signed = |name: &str, n: i64| {
            StructuredInfoValue::leaf(field_type(ty, name).clone(), n.to_le_bytes().to_vec())
                .unwrap()
        };
        let mut fields = vec![
            (
                "profile".to_owned(),
                text(field_type(ty, "profile"), profile),
            ),
            (
                "calibration".to_owned(),
                text(field_type(ty, "calibration"), calibration),
            ),
            (
                "confirmation".to_owned(),
                text(field_type(ty, "confirmation"), "synthetic-not-calibrated"),
            ),
            (
                "margin_numerator".to_owned(),
                signed("margin_numerator", margin.0),
            ),
            (
                "margin_denominator".to_owned(),
                signed("margin_denominator", margin.1),
            ),
        ];
        for i in 0..4 {
            let name = format!("active{i}");
            fields.push((
                name.clone(),
                StructuredInfoValue::leaf(field_type(ty, &name).clone(), vec![u8::from(active[i])])
                    .unwrap(),
            ));
            for (name, n) in [
                (format!("score{i}"), scores[i]),
                (format!("count{i}"), counts[i]),
            ] {
                fields.push((name.clone(), signed(&name, n)));
            }
        }
        record(
            ty,
            fields
                .iter()
                .map(|(n, v)| (n.as_str(), v.clone()))
                .collect(),
        )
    }
    fn run(&self, input: StructuredInfoValue) -> Result<(bool, [bool; 4]), String> {
        validate_native_contracts(&input, &self.native.value_contracts)
            .map_err(|e| format!("{e:?}"))?;
        validate_native_invariants(&input, &self.native.invariants)
            .map_err(|e| format!("{e:?}"))?;
        let bytes = self
            .program
            .evaluate(&input.canonical_bytes().unwrap())
            .map_err(|e| format!("{e:?}"))?;
        let output = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let bool_value = |v: &StructuredInfoValue| match v.shape() {
            StructuredInfoValueShape::Leaf(b) => b == &[1],
            _ => panic!("bool"),
        };
        let StructuredInfoValueShape::Collection(mask) = field(&output, "mask").shape() else {
            panic!("mask")
        };
        Ok((
            bool_value(field(&output, "accepted")),
            core::array::from_fn(|i| bool_value(&mask[i])),
        ))
    }
}
#[test]
fn synthetic_retention_preserves_ties_normalization_and_inactive_exclusion() {
    let p = Prepared::new();
    let kind = "experimental/normalized-margin@1";
    // Synthetic zero is explicit test data, never a default production margin.
    let input = |a, s, c, m| p.input(a, s, c, m, kind, "synthetic-only");
    assert_eq!(
        p.run(input([true; 4], [10; 4], [1; 4], (0, 1))).unwrap(),
        (true, [true; 4])
    );
    // Different scoring lengths/derivations with equal exact means all survive.
    assert_eq!(
        p.run(input([true; 4], [10, 20, 30, 40], [1, 2, 3, 4], (0, 1)))
            .unwrap(),
        (true, [true; 4])
    );
    assert_eq!(
        p.run(input(
            [false, true, true, true],
            [1000000, 8, 7, 0],
            [0, 1, 1, 1],
            (1, 1)
        ))
        .unwrap(),
        (true, [false, true, true, false])
    );
    assert_eq!(
        p.run(input(
            [true; 4],
            [-1000000, 1000000, 0, 0],
            [8, 1, 1, 1],
            (2000000, 64)
        ))
        .unwrap(),
        (true, [false, true, false, false])
    );
}
#[test]
fn missing_profiles_empty_beam_and_invalid_numeric_contracts_refuse() {
    let p = Prepared::new();
    let kind = "experimental/normalized-margin@1";
    for (a, c, profile, calibration) in [
        ([false; 4], [0; 4], kind, "synthetic-only"),
        ([true; 4], [0; 4], kind, "synthetic-only"),
        ([true; 4], [1; 4], "", "synthetic-only"),
        ([true; 4], [1; 4], "foreign/profile", "synthetic-only"),
        ([true; 4], [1; 4], kind, ""),
    ] {
        assert_eq!(
            p.run(p.input(a, [1; 4], c, (0, 1), profile, calibration))
                .unwrap(),
            (false, [false; 4])
        );
    }
    // Native admission rejects impossible overflow/count/denominator inputs
    // before any arithmetic executes; this is not a decoder observation.
    for (s, c, m) in [
        ([i64::MAX; 4], [1; 4], (0, 1)),
        ([0; 4], [9; 4], (0, 1)),
        ([0; 4], [1; 4], (0, 0)),
    ] {
        assert!(p
            .run(p.input([true; 4], s, c, m, kind, "synthetic-only"))
            .is_err());
    }
}
