//! Optional resource references retain their codec-owned finite byte envelope.
use conduit_core as c;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
fn expand(
    bound: Option<u64>,
) -> Result<conduit_plot::ExpandedCanonicalPlot, conduit_plot::CanonicalExpansionDiagnostic> {
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let bound = bound
        .map(|bytes| format!(" <= {bytes}B"))
        .unwrap_or_default();
    let source = format!("plot retained {{\n seed: keep ResourceRef?{bound} for life\n}}\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    expand_canonical_plot(&checked, "retained", &ProfileCatalog::new())
}
#[test]
fn optional_resource_reference_bound_uses_canonical_framing_and_the_reference_codec_limit() {
    let expanded = expand(None).unwrap();
    let [gear] = expanded.gears.as_slice() else {
        panic!("one ordinary State")
    };
    assert_eq!(gear.kind_id.as_str(), c::STATE_VALUE_KIND);
    let entry = |key| {
        &gear
            .configuration
            .iter()
            .find(|entry| entry.key == key)
            .unwrap()
            .value
    };
    assert_eq!(
        entry("retained-duration"),
        &c::ConfigurationValue::Text("body".into())
    );
    let c::ConfigurationValue::Structured(initial) = entry("initial") else {
        panic!("canonical initializer")
    };
    let none = c::StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).unwrap();
    assert!(matches!(
        none.shape(),
        c::StructuredInfoValueShape::Variant { tag: "none", .. }
    ));
    let c::ConfigurationValue::U64(maximum) = entry("maximum-bytes") else {
        panic!("finite bound")
    };
    assert_eq!(
        *maximum,
        (initial.canonical_value().len() + c::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES) as u64
    );
    assert!(*maximum < 1024);
    let reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: "p"
            .repeat(c::MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES)
            .into(),
        access_class: "a"
            .repeat(c::MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES)
            .into(),
        extent: c::ResourceExtent {
            bytes: c::MAXIMUM_REFERENCED_BYTES,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let encoded = reference.encode().unwrap();
    let some = c::StructuredInfoValue::variant(
        none.value_type().clone(),
        "some",
        c::StructuredInfoValue::leaf(
            c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID)).unwrap(),
            encoded,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(some.canonical_bytes().unwrap().len() <= *maximum as usize);
    assert!(expand(Some(*maximum)).is_ok());
    let refused = expand(Some(*maximum - 1)).unwrap_err();
    assert!(refused.message.contains("admitted value envelope"));
}
