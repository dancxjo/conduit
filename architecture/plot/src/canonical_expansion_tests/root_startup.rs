//! Ordinary Root invocation, before expansion; no renderer or Host authority.
use super::*;
use crate::{
    expand_canonical_plot_for_authoring_with_startup, CanonicalBackCatalog, CanonicalStartupValue,
    CheckedStartupBinding, PreparedRootStartupInputs, RootStartupRefusal as R,
};
fn binding(name: &str, ty: &str, value: CanonicalStartupValue) -> CheckedStartupBinding {
    CheckedStartupBinding {
        name: name.into(),
        value_type: ty.into(),
        value,
    }
}
fn fixture() -> (crate::CheckedSyntaxDocument, ProfileCatalog) {
    let (startup, profile) = catalogs();
    let source = "plot root (\n count: Count = 1\n input: test/value >> output: test/value\n) {\n pass: test/pass(count)\n input >> pass >> output\n}\nplot other (\n count: Count = 1\n input: test/value >> output: test/value\n) {\n pass: test/pass(count)\n input >> pass >> output\n}\n";
    (
        check_syntax_document(&parse_syntax_document(source), &startup).unwrap(),
        profile,
    )
}
#[test]
fn typed_root_override_changes_real_configuration_and_expansion_identity() {
    let (document, profile) = fixture();
    let backs = CanonicalBackCatalog::new();
    let default = expand_canonical_plot_for_authoring(&document, "root", &profile).unwrap();
    let empty = PreparedRootStartupInputs::prepare(&document, "root", &profile, &[]).unwrap();
    assert_eq!(
        expand_canonical_plot_for_authoring_with_startup(
            &document, "root", &profile, &backs, &empty
        )
        .unwrap(),
        default
    );
    let inputs = PreparedRootStartupInputs::prepare(
        &document,
        "root",
        &profile,
        &[binding(
            "count",
            "Count",
            CanonicalStartupValue::Literal("2".into()),
        )],
    )
    .unwrap();
    let changed = expand_canonical_plot_for_authoring_with_startup(
        &document, "root", &profile, &backs, &inputs,
    )
    .unwrap();
    assert_eq!(
        changed.expanded.source_document_id,
        default.expanded.source_document_id
    );
    assert_eq!(
        changed.expanded.checked_plot_id,
        default.expanded.checked_plot_id
    );
    assert_ne!(
        changed.expanded.expanded_plot_id,
        default.expanded.expanded_plot_id
    );
    assert!(changed.expanded.gears.iter().any(|g| g
        .configuration
        .iter()
        .any(|v| v.value == ConfigurationValue::U64(2))));
    assert!(expand_canonical_plot_for_authoring_with_startup(
        &document, "other", &profile, &backs, &inputs
    )
    .is_err());
    let mut forged = document.clone();
    forged.plots[0].startup_parameters[0].maximum_bytes = Some(0);
    assert!(expand_canonical_plot_for_authoring_with_startup(
        &forged, "root", &profile, &backs, &inputs
    )
    .is_err());
}
#[test]
fn root_inputs_refuse_unknown_duplicate_wrong_type_nonconcrete_and_capacity() {
    let (document, profile) = fixture();
    let one = binding("count", "Count", CanonicalStartupValue::Literal("2".into()));
    let prepare = |v: &[CheckedStartupBinding]| {
        PreparedRootStartupInputs::prepare(&document, "root", &profile, v)
    };
    assert_eq!(
        prepare(&[binding("foreign", "Count", one.value.clone())])
            .err()
            .unwrap(),
        R::UnknownInput
    );
    assert_eq!(
        prepare(&[one.clone(), one.clone()]).err().unwrap(),
        R::DuplicateInput
    );
    assert_eq!(
        prepare(&[binding("count", "U64", one.value.clone())])
            .err()
            .unwrap(),
        R::WrongType
    );
    assert_eq!(
        prepare(&[binding(
            "count",
            "Count",
            CanonicalStartupValue::Literal("false".into())
        )])
        .err()
        .unwrap(),
        R::WrongType
    );
    assert_eq!(
        prepare(&[binding(
            "count",
            "Count",
            CanonicalStartupValue::PlotParameter("count".into())
        )])
        .err()
        .unwrap(),
        R::NonConcrete
    );
    assert_eq!(prepare(&vec![one; 65]).err().unwrap(), R::Capacity);
    let mut bounded = document.clone();
    bounded
        .plots
        .iter_mut()
        .find(|plot| plot.name == "root")
        .unwrap()
        .startup_parameters[0]
        .maximum_bytes = Some(1);
    assert_eq!(
        PreparedRootStartupInputs::prepare(
            &bounded,
            "root",
            &profile,
            &[binding(
                "count",
                "Count",
                CanonicalStartupValue::Literal("256".into())
            )]
        )
        .err()
        .unwrap(),
        R::ValueContract
    );
}
#[test]
fn root_native_laws_are_checked_before_expansion() {
    let profile = ProfileCatalog::new();
    let syntax = parse_syntax_document("type Limited = {\n value: U16 in 1..=3\n}\nplot root (\n choice: Limited = {value: 1}\n) {\n}\n");
    let document = check_syntax_document(&syntax, &profile.startup_catalog().unwrap()).unwrap();
    let ty = document
        .native_types
        .iter()
        .find(|n| n.name == "Limited")
        .unwrap()
        .value_type
        .clone();
    let leaf = conduit_core::StructuredInfoValue::leaf(
        conduit_core::StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
        4u16.to_le_bytes().to_vec(),
    )
    .unwrap();
    let value = conduit_core::StructuredInfoValue::record(
        ty,
        vec![conduit_core::StructuredFieldValue::new("value", leaf).unwrap()],
    )
    .unwrap();
    let input = binding(
        "choice",
        "Limited",
        CanonicalStartupValue::Structured(
            crate::CanonicalStructuredStartupValue::from_owned_checked_value(value),
        ),
    );
    assert_eq!(
        PreparedRootStartupInputs::prepare(&document, "root", &profile, &[input])
            .err()
            .unwrap(),
        R::ValueContract
    );
}

#[test]
fn typed_physical_values_cannot_be_relabelled_as_opaque_root_inputs() {
    let (mut startup, profile) = catalogs();
    startup
        .insert_value_kind_alias("Opaque", kind_id("test/opaque"))
        .unwrap();
    startup
        .insert_value_kind_alias("DisplayUnit", kind_id(conduit_core::UNIT_INFO_ID))
        .unwrap();
    let document = check_syntax_document(
        &parse_syntax_document("plot root (\n choice: Opaque\n unit: DisplayUnit\n) {\n}\n"),
        &startup,
    )
    .unwrap();
    let unit =
        CanonicalStartupValue::Unit(conduit_core::UnitConfigurationValue::parse("m").unwrap());
    assert!(PreparedRootStartupInputs::prepare(
        &document,
        "root",
        &profile,
        &[binding("unit", "DisplayUnit", unit.clone())],
    )
    .is_ok());
    let quantity = CanonicalStartupValue::Quantity(
        conduit_core::QuantityConfigurationValue::from_value(Quantity::new(500, Unit::Millimeter))
            .unwrap(),
    );
    for forged in [unit, quantity] {
        let result = PreparedRootStartupInputs::prepare(
            &document,
            "root",
            &profile,
            &[binding("choice", "Opaque", forged)],
        );
        assert!(
            matches!(result, Err(R::WrongType)),
            "foreign physical value admitted: {result:?}"
        );
    }
}
