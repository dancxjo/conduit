use super::*;
use alloc::collections::BTreeSet;
use alloc::string::ToString;

#[test]
fn inline_variant_layout_is_explicit_and_preserves_semantic_identity() {
    let types = crate::check_syntax_document(
        &crate::parse_syntax_document("type Event =\n    loaded {\n        value: U8\n    }\n    | idle\ntype Other =\n    first\n    | second\ntype Count = U8\n"),
        &crate::StartupCatalog::new(),
    ).unwrap().native_types;
    let plain = generate_rust_bindings(&types, &RustBindingOptions::default()).unwrap();
    let mut options = RustBindingOptions {
        inline_variant_types: BTreeSet::from(["Event".to_string()]),
        ..RustBindingOptions::default()
    };
    let inline = generate_rust_bindings(&types, &options).unwrap();
    assert_eq!(plain.semantic_type_bytes, inline.semantic_type_bytes);
    assert!(!plain.source.contains("large_enum_variant"));
    assert_eq!(inline.source.matches("large_enum_variant").count(), 1);
    assert!(inline.source.contains("Loaded(EventLoaded)"));
    assert!(!inline.source.contains("Box<EventLoaded>"));
    options
        .boxed_variant_payloads
        .insert("Event.loaded".to_string());
    assert!(generate_rust_bindings(&types, &options).is_err());
    options.boxed_variant_payloads.clear();
    for invalid in ["Count", "Missing"] {
        options.inline_variant_types = BTreeSet::from([invalid.to_string()]);
        assert!(generate_rust_bindings(&types, &options).is_err());
    }
}

#[test]
fn boxed_physical_leaf_layout_preserves_exact_type_identity() {
    let types = crate::check_syntax_document(
        &crate::parse_syntax_document(
            "type PhysicalOutcome =\n partial Quantity\n | target Unit\n | absent\n",
        ),
        &crate::StartupCatalog::new(),
    )
    .unwrap()
    .native_types;
    let plain = generate_rust_bindings(&types, &RustBindingOptions::default()).unwrap();
    let options = RustBindingOptions {
        boxed_variant_payloads: [
            "PhysicalOutcome.partial".into(),
            "PhysicalOutcome.target".into(),
        ]
        .into(),
        ..RustBindingOptions::default()
    };
    let boxed = generate_rust_bindings(&types, &options).unwrap();
    assert_eq!(plain.semantic_type_bytes, boxed.semantic_type_bytes);
    assert!(boxed
        .source
        .contains("Partial(Box<conduit_core::Quantity>)"));
    assert!(boxed.source.contains("Target(Box<conduit_core::Unit>)"));
    let invalid = RustBindingOptions {
        boxed_variant_payloads: ["PhysicalOutcome.absent".into()].into(),
        ..RustBindingOptions::default()
    };
    assert!(generate_rust_bindings(&types, &invalid).is_err());
}
