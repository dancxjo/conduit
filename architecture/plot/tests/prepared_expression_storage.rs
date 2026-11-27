//! Reviewed storage for the authored catalogue's ordinary Some constructor.
use conduit_core as c;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram as Program, PreparedPortableExpressionEvaluator as Evaluator,
    ProfileCatalog, QualifiedExpressionStorage as Storage,
    QualifiedExpressionStorageRefusal as Refusal, StartupCatalog,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn authored() -> Program {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    let source = "plot present (\n >> reference: ResourceRef\n selected: ResourceRef? >>\n) {\n reference >> (some(.)) >> selected\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &catalog).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "present", &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let c::ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    Program::from_canonical_hex(encoded).unwrap()
}
fn reference(seed: u8) -> Vec<u8> {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 4096,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([seed; 32]),
            expires_at: None,
        },
    }
    .encode()
    .unwrap()
}
#[test]
fn authored_some_preflight_and_one_byte_short_refuse_without_allocating() {
    let program = authored();
    let (receipt, observed) =
        allocation_probe::observe(|| Storage::for_resource_reference_some(&program).unwrap());
    assert_eq!(observed.allocations, 0);
    let (result, observed) = allocation_probe::observe(|| {
        Evaluator::new_qualified_resource_reference_some(&program, receipt.preparation_bytes - 1)
    });
    assert!(matches!(result, Err(Refusal::InsufficientPreparation)));
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
}
#[test]
fn qualified_actual_retained_and_preparation_peak_fit_and_recurring_some_is_stable() {
    let program = authored();
    let receipt = Storage::for_resource_reference_some(&program).unwrap();
    let (prepared, observed) = allocation_probe::observe(|| {
        Evaluator::new_qualified_resource_reference_some(&program, receipt.preparation_bytes)
            .unwrap()
    });
    let mut prepared = prepared;
    let actual =
        core::mem::size_of::<Evaluator>() + prepared.qualified_retained_backing_bytes().unwrap();
    assert!(actual <= receipt.retained_bytes);
    assert!(
        observed.peak_bytes + core::mem::size_of::<Evaluator>() <= receipt.preparation_bytes,
        "peak {} exceeds {}",
        observed.peak_bytes,
        receipt.preparation_bytes
    );
    for seed in [1, 2, 255] {
        let bytes = reference(seed);
        let expected = program.evaluate(&bytes).unwrap();
        let (_, observed) = allocation_probe::observe(|| {
            assert_eq!(prepared.evaluate(&bytes).unwrap(), expected);
            assert!(prepared.evaluate(&bytes[..bytes.len() - 1]).is_err());
        });
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(
            core::mem::size_of::<Evaluator>()
                + prepared.qualified_retained_backing_bytes().unwrap(),
            actual
        );
    }
}
#[test]
fn unsupported_expression_is_not_given_a_guessed_receipt() {
    let mut program = authored();
    program.root.operation = conduit_plot::PortableExpressionOperation::Input;
    let (result, observed) =
        allocation_probe::observe(|| Storage::for_resource_reference_some(&program));
    assert_eq!(result, Err(Refusal::UnsupportedProgram));
    assert_eq!(observed.allocations, 0);
}

#[test]
fn wire_preflight_precedes_decoder_and_accounts_its_types_and_reserialization() {
    let source = authored();
    let encoded = source.canonical_hex().unwrap();
    let (receipt, observed) = allocation_probe::observe(|| {
        conduit_plot::QualifiedExpressionProgramStorage::resource_reference_some_hex(&encoded)
            .unwrap()
    });
    assert_eq!(observed.allocations, 0);
    let (result, observed) = allocation_probe::observe(|| {
        Program::from_qualified_resource_reference_some_hex(&encoded, receipt.preparation_bytes - 1)
    });
    assert!(matches!(result, Err(Refusal::InsufficientPreparation)));
    assert_eq!(observed.allocations, 0);
    let (program, observed) = allocation_probe::observe(|| {
        Program::from_qualified_resource_reference_some_hex(&encoded, receipt.preparation_bytes)
            .unwrap()
    });
    assert!(observed.peak_bytes + core::mem::size_of::<Program>() <= receipt.preparation_bytes);
    assert!(
        program.qualified_retained_backing_bytes().unwrap() + core::mem::size_of::<Program>()
            <= receipt.decoded_program_bytes
    );
    assert_eq!(program, source);
    for bad in [&encoded[..encoded.len() - 2], "00", "zz"] {
        let (result, observed) = allocation_probe::observe(|| {
            conduit_plot::QualifiedExpressionProgramStorage::resource_reference_some_hex(bad)
        });
        assert_eq!(result, Err(Refusal::UnsupportedProgram));
        assert_eq!(observed.allocations, 0);
    }
}
