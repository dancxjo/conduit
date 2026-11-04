//! Opaque leaves keep raw semantic framing through prepared structured paths.
use conduit_core as c;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn indexed_opaque_reference_preserves_bytes_and_recovers_without_allocating() {
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("Reference", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    let source = "type Query = {\n references: collection Reference = 2\n index: U64\n}\nplot select (\n input: Query <= 1024B >> result: Reference\n) = (sequence/at(.references, .index))\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "select", &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let c::ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let references = [reference(1), reference(2)];
    for index in [0u64, 1, 2, 0] {
        let input = input(&program.input_type, &references, index);
        let expected = program.evaluate(&input);
        if let Ok(expected) = &expected {
            assert_eq!(*expected, references[index as usize].encode().unwrap());
        }
        let (_, cost) = allocation_probe::observe(|| {
            match &expected {
                Ok(expected) => {
                    assert_eq!(evaluator.evaluate(&input).unwrap(), expected);
                }
                Err(expected) => {
                    assert_eq!(evaluator.evaluate(&input).unwrap_err(), *expected);
                }
            }
            assert!(evaluator.evaluate(&input[..input.len() - 1]).is_err());
        });
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
    }
}
fn reference(seed: u8) -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: "test/data@1".into(),
        access_class: "resource/read@1".into(),
        extent: c::ResourceExtent {
            bytes: 12,
            items: Some(2),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([seed; 32]),
            expires_at: None,
        },
    }
}
fn input(
    ty: &c::StructuredInfoType,
    references: &[c::BoundedResourceRef; 2],
    index: u64,
) -> Vec<u8> {
    let c::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("query")
    };
    c::StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                let ty = f.value_type();
                let value = if f.name() == "index" {
                    c::StructuredInfoValue::leaf(ty.clone(), index.to_le_bytes().to_vec()).unwrap()
                } else {
                    let c::StructuredInfoTypeShape::Collection { element, .. } = ty.shape() else {
                        panic!("references")
                    };
                    c::StructuredInfoValue::collection(
                        ty.clone(),
                        references
                            .iter()
                            .map(|r| {
                                c::StructuredInfoValue::leaf(element.clone(), r.encode().unwrap())
                                    .unwrap()
                            })
                            .collect(),
                    )
                    .unwrap()
                };
                c::StructuredFieldValue::new(f.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
