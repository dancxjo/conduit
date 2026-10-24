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

#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn prepared_optional_reference_preserves_type_validates_encoding_and_allocates_nothing() {
    let source = "plot wrap (\n >> reference: ResourceRef\n selected: ResourceRef? >>\n) {\n reference >> (some(.)) >> selected\n}\n";
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let plot =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "wrap", &ProfileCatalog::new())
            .unwrap();
    let c::ConfigurationValue::Text(program) = &plot.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 4096,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let encoded = reference.encode().unwrap();
    let expected = program.evaluate(&encoded).unwrap();
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert!(prepared.output_capacity() < 1024);
    let mut invalid = encoded.clone();
    invalid[0] = 0;
    let (_, memory) = allocation_probe::observe(|| {
        for _ in 0..64 {
            assert_eq!(prepared.evaluate(&encoded).unwrap(), expected);
        }
        assert!(prepared.evaluate(&invalid).is_err());
        assert!(prepared.evaluate(&encoded[..encoded.len() - 1]).is_err());
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
