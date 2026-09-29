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
    checked.forms[0]
        .runtime_front
        .value_contract(&FrontValueLocation::Input(conduit_core::port_id(port)))
        .expect("refined input retains its exact checked contract")
}

#[test]
fn authored_ranges_preserve_open_and_closed_endpoints() {
    let checked = check(
        "form bounded (\n >> count: Count where range(1 exclusive, 4 inclusive)\n >> temperature: Temperature where range(18°C inclusive, 24°C exclusive)\n) {\n}\n",
    );
    let count = input_contract(&checked, "count");
    assert_eq!(count.maximum_bytes, conduit_core::COUNT_ENCODED_LEN as u32);
    assert_eq!(
        count.constraints,
        vec![ValueConstraint::UnsignedRange {
            minimum: 1,
            maximum: 4,
            minimum_endpoint: IntervalEndpoint::Exclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }]
    );
    assert_eq!(
        count.validate(&encode_count(1)),
        Err(ValueConstraintRefusal::UnsignedRange)
    );
    assert_eq!(count.validate(&encode_count(4)), Ok(()));

    let temperature = input_contract(&checked, "temperature");
    assert_eq!(
        temperature.constraints,
        vec![ValueConstraint::QuantityRange {
            minimum: conduit_core::Quantity::new(18, conduit_core::QuantityUnit::Celsius),
            maximum: conduit_core::Quantity::new(24, conduit_core::QuantityUnit::Celsius),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Exclusive,
        }]
    );
}

#[test]
fn membership_is_canonical_and_composes_with_pattern_independent_of_authored_order() {
    let first = check(
        "form choice (\n >> code: Text <= 8B where member(\"AB12\", \"CD34\") and pattern(r\"[A-Z]{2}[0-9]{2}\")\n) {\n}\n",
    );
    let reversed = check(
        "form choice (\n >> code: Text <= 8B where pattern(r\"[A-Z]{2}[0-9]{2}\") and member(\"CD34\", \"AB12\")\n) {\n}\n",
    );
    let contract = input_contract(&first, "code");
    assert!(matches!(
        contract.constraints.as_slice(),
        [ValueConstraint::CanonicalMembership { members }, ValueConstraint::TextPattern(_)]
            if members == &[b"AB12".to_vec(), b"CD34".to_vec()]
    ));
    assert_eq!(contract.validate(b"AB12"), Ok(()));
    assert_eq!(
        contract.validate(b"EF56"),
        Err(ValueConstraintRefusal::Membership)
    );
    assert_eq!(
        first.forms[0].checked_form_id, reversed.forms[0].checked_form_id,
        "constraint and membership source order cannot change checked meaning"
    );
}

#[test]
fn malformed_or_incompatible_authored_refinements_refuse_during_checking() {
    for (source, expected) in [
        (
            "form bad (\n >> value: Text <= 8B where range(1 inclusive, 2 inclusive)\n) {\n}\n",
            "requires Count, Scalar, or exact semantic Quantity",
        ),
        (
            "form bad (\n >> value: Count where member(1, 01)\n) {\n}\n",
            "same canonical value",
        ),
        (
            "form bad (\n >> value: Temperature where range(1m inclusive, 2m inclusive)\n) {\n}\n",
            "runtime Port value contract is invalid",
        ),
        (
            "form bad (\n >> value: Text <= 8B where member()\n) {\n}\n",
            "invalid",
        ),
    ] {
        let error = check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
            .expect_err("invalid refinement must refuse before Plan/Play");
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

#[test]
fn duplicate_constraint_family_refuses_instead_of_last_write_wins() {
    let source =
        "form bad (\n >> value: Count where range(1 inclusive, 4 inclusive) and range(2 inclusive, 3 inclusive)\n) {\n}\n";
    let error = check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
        .expect_err("two range truths cannot silently overwrite one another");
    assert!(error
        .message
        .contains("runtime Port value contract is invalid"));
}
