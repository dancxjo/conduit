use crate::{check_syntax_document, parse_syntax_document, StartupCatalog};
use conduit_core::{
    encode_count, FrontValueLocation, IntervalEndpoint, ValueConstraint, ValueConstraintRefusal,
};

fn check(source: &str) -> crate::CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
        .expect("checked refinements are canonical")
}

fn input_contract<'a>(
    checked: &'a crate::CheckedSyntaxDocument,
    port: &str,
) -> &'a conduit_core::CheckedValueContract {
    checked.plots[0]
        .runtime_front
        .value_contract(&FrontValueLocation::Input(conduit_core::port_id(port)))
        .expect("refined input retains its exact checked contract")
}

#[test]
fn authored_ranges_preserve_open_and_closed_endpoints() {
    let checked = check(
        "plot bounded (\n >> count: Count in 1..=4\n >> temperature: Temperature in 18°C..24°C\n) {\n}\n",
    );
    let count = input_contract(&checked, "count");
    assert_eq!(count.maximum_bytes, conduit_core::COUNT_ENCODED_LEN as u32);
    assert_eq!(
        count.constraints,
        vec![ValueConstraint::UnsignedRange {
            minimum: Some(1),
            maximum: Some(4),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }]
    );
    assert_eq!(count.validate(&encode_count(1)), Ok(()));
    assert_eq!(count.validate(&encode_count(4)), Ok(()));

    let temperature = input_contract(&checked, "temperature");
    assert_eq!(
        temperature.constraints,
        vec![ValueConstraint::QuantityRange {
            minimum: Some(conduit_core::Quantity::new(18, conduit_core::Unit::Celsius).into()),
            maximum: Some(conduit_core::Quantity::new(24, conduit_core::Unit::Celsius).into()),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Exclusive,
        }]
    );
}

#[test]
fn ieee_float_refinements_are_exact_finite_and_identity_bearing() {
    let checked = check(
        "plot floats (\n >> raw: F32\n >> finite: F32 finite\n >> probability: F32 finite in 0.0..=1.0\n) {\n}\n",
    );
    let finite = input_contract(&checked, "finite");
    assert_eq!(finite.maximum_bytes, 4);
    assert_eq!(finite.constraints, vec![ValueConstraint::FloatFinite]);
    let probability = input_contract(&checked, "probability");
    assert!(matches!(
        probability.constraints.as_slice(),
        [
            ValueConstraint::FloatFinite,
            ValueConstraint::FloatRange { .. }
        ]
    ));
    assert_ne!(finite.identity_bytes(), probability.identity_bytes());
    assert_eq!(
        finite.validate(&conduit_core::IeeeF32::from_bits(0x7fc0_0001).encode()),
        Err(ValueConstraintRefusal::FloatFinite)
    );
    assert_eq!(
        probability.validate(&conduit_core::IeeeF32::from(1.5).encode()),
        Err(ValueConstraintRefusal::FloatRange)
    );
}

#[test]
fn missing_range_ends_remain_semantically_open() {
    let checked = check(
        "plot bounded (\n >> low: Count in ..=4\n >> high: Count in 4..\n >> scalar: Scalar in ..=1.000000\n) {\n}\n",
    );
    assert_eq!(
        input_contract(&checked, "low").constraints,
        vec![ValueConstraint::UnsignedRange {
            minimum: None,
            maximum: Some(4),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }]
    );
    assert_eq!(
        input_contract(&checked, "high").constraints,
        vec![ValueConstraint::UnsignedRange {
            minimum: Some(4),
            maximum: None,
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }]
    );
    assert_eq!(
        input_contract(&checked, "scalar").constraints,
        vec![ValueConstraint::SignedRange {
            minimum: None,
            maximum: Some(1_000_000),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }]
    );
    assert_eq!(
        input_contract(&checked, "high").validate(&encode_count(u64::MAX)),
        Ok(())
    );

    let explicit = check(
        "plot bounded (\n >> low: Count in 0..=4\n >> high: Count in 4..=18446744073709551615\n >> scalar: Scalar in -9223372036854.775808..=1.000000\n) {\n}\n",
    );
    assert_ne!(
        checked.plots[0].checked_plot_id, explicit.plots[0].checked_plot_id,
        "semantic openness is distinct from the current carrier's extrema"
    );
}

#[test]
fn membership_is_canonical_and_composes_with_pattern_independent_of_authored_order() {
    let first = check(
        "plot choice (\n >> code: Text <= 8B in [\"AB12\", \"CD34\"] ~ /[A-Z]{2}[0-9]{2}/\n) {\n}\n",
    );
    let reversed = check(
        "plot choice (\n >> code: Text <= 8B ~ /[A-Z]{2}[0-9]{2}/ in [\"CD34\", \"AB12\"]\n) {\n}\n",
    );
    let contract = input_contract(&first, "code");
    assert!(matches!(
        contract.constraints.as_slice(),
        [ValueConstraint::CanonicalMembership { members, negated: false }, ValueConstraint::TextPattern { negated: false, .. }]
            if members == &[b"AB12".to_vec(), b"CD34".to_vec()]
    ));
    assert_eq!(contract.validate(b"AB12"), Ok(()));
    assert_eq!(
        contract.validate(b"EF56"),
        Err(ValueConstraintRefusal::Membership)
    );
    assert_eq!(
        first.plots[0].checked_plot_id, reversed.plots[0].checked_plot_id,
        "constraint and membership source order cannot change checked meaning"
    );
}

#[test]
fn negated_membership_and_pattern_are_checked_relations() {
    let checked = check(
        "plot guarded (\n >> name: Text <= 8B not in [\"admin\", \"root\"]\n >> code: Text <= 8B !~ /[A-Z]{2}/i\n) {\n}\n",
    );
    let name = input_contract(&checked, "name");
    assert!(matches!(
        name.constraints.as_slice(),
        [ValueConstraint::CanonicalMembership { negated: true, .. }]
    ));
    assert_eq!(name.validate(b"guest"), Ok(()));
    assert_eq!(
        name.validate(b"root"),
        Err(ValueConstraintRefusal::Membership)
    );

    let code = input_contract(&checked, "code");
    assert!(matches!(
        code.constraints.as_slice(),
        [ValueConstraint::TextPattern { negated: true, .. }]
    ));
    assert_eq!(code.validate(b"12"), Ok(()));
    assert_eq!(
        code.validate(b"xxAByy"),
        Err(ValueConstraintRefusal::TextPattern)
    );
    assert_eq!(
        code.validate(b"xxabyy"),
        Err(ValueConstraintRefusal::TextPattern)
    );

    let positive = check(
        "plot guarded (\n >> name: Text <= 8B in [\"admin\", \"root\"]\n >> code: Text <= 8B ~ /[A-Z]{2}/i\n) {\n}\n",
    );
    assert_ne!(
        checked.plots[0].checked_plot_id,
        positive.plots[0].checked_plot_id
    );
}

#[test]
fn canonical_anchors_select_prefix_suffix_and_whole_value_profiles() {
    let checked = check(
        r#"plot anchored (
 >> prefix: Text <= 8B ~ /^AB/
 >> suffix: Text <= 8B ~ /AB$/
 >> whole: Text <= 8B ~ /^AB$/
 >> escaped-dollar: Text <= 8B ~ /USD\$/
 >> backslash-suffix: Text <= 16B ~ /path\\$/
) {
}
"#,
    );
    let prefix = input_contract(&checked, "prefix");
    assert_eq!(prefix.validate(b"ABxx"), Ok(()));
    assert_eq!(
        prefix.validate(b"xxAB"),
        Err(ValueConstraintRefusal::TextPattern)
    );

    let suffix = input_contract(&checked, "suffix");
    assert_eq!(suffix.validate(b"xxAB"), Ok(()));
    assert_eq!(
        suffix.validate(b"ABxx"),
        Err(ValueConstraintRefusal::TextPattern)
    );

    let whole = input_contract(&checked, "whole");
    assert_eq!(whole.validate(b"AB"), Ok(()));
    assert_eq!(
        whole.validate(b"xxAB"),
        Err(ValueConstraintRefusal::TextPattern)
    );
    assert_eq!(
        whole.validate(b"ABxx"),
        Err(ValueConstraintRefusal::TextPattern)
    );

    let escaped_dollar = input_contract(&checked, "escaped-dollar");
    assert_eq!(escaped_dollar.validate(b"USD$ xx"), Ok(()));

    let backslash_suffix = input_contract(&checked, "backslash-suffix");
    assert_eq!(backslash_suffix.validate(b"before path\\"), Ok(()));
    assert_eq!(
        backslash_suffix.validate(b"path\\ after"),
        Err(ValueConstraintRefusal::TextPattern)
    );
}

#[test]
fn leading_lookahead_is_checked_once_and_searches_within_the_admitted_text() {
    let checked = check(
        "plot guarded (\n >> positive: Text <= 8B ~ /(?=AB)A./\n >> negative: Text <= 8B ~ /(?!AB)A./\n >> folded: Text <= 8B ~ /(?=ab)A./i\n) {\n}\n",
    );
    let positive = input_contract(&checked, "positive");
    assert_eq!(positive.validate(b"xxAByy"), Ok(()));
    assert_eq!(
        positive.validate(b"xxACyy"),
        Err(ValueConstraintRefusal::TextPattern)
    );

    let negative = input_contract(&checked, "negative");
    assert_eq!(negative.validate(b"xxACyy"), Ok(()));
    assert_eq!(
        negative.validate(b"xxAByy"),
        Err(ValueConstraintRefusal::TextPattern)
    );

    let folded = input_contract(&checked, "folded");
    assert_eq!(folded.validate(b"xxAbyy"), Ok(()));
}

#[test]
fn malformed_or_incompatible_authored_refinements_refuse_during_checking() {
    for (source, expected) in [
        (
            "plot bad (\n >> value: Text <= 8B in 1..=2\n) {\n}\n",
            "requires Count, Scalar, or exact semantic Quantity",
        ),
        (
            "plot bad (\n >> value: Count in [1, 01]\n) {\n}\n",
            "same canonical value",
        ),
        (
            "plot bad (\n >> value: Temperature in 1m..=2m\n) {\n}\n",
            "runtime Port value contract is invalid",
        ),
        (
            "plot bad (\n >> value: Temperature in ..=2m\n) {\n}\n",
            "runtime Port value contract is invalid",
        ),
        (
            "plot bad (\n >> value: Text <= 8B in []\n) {\n}\n",
            "invalid",
        ),
        ("plot bad (\n >> value: Scalar in ..\n) {\n}\n", "invalid"),
        (
            "plot bad (\n >> value: U32 finite\n) {\n}\n",
            "finite may refine only F32 or F64",
        ),
        (
            "plot bad (\n >> value: F32 in NaN..=1.0\n) {\n}\n",
            "exact finite float literal",
        ),
        (
            "plot bad (\n >> value: F32 in 0.1234567891..=1.0\n) {\n}\n",
            "exact finite float literal",
        ),
    ] {
        let error = check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
            .expect_err("invalid refinement must refuse before Plan/Play");
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

#[test]
fn duplicate_constraint_family_refuses_instead_of_last_write_wins() {
    let source = "plot bad (\n >> value: Count in 1..=4 in 2..=3\n) {\n}\n";
    let error = check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
        .expect_err("two range truths cannot silently overwrite one another");
    assert!(error
        .message
        .contains("runtime Port value contract is invalid"));
}
