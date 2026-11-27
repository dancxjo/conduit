//! Initial descriptor material for the exact ordinary catalogue constructor.
use conduit_core as c;
use conduit_plot::{
    PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program, QualifiedExpressionStorageRefusal as E,
};
use conduit_semantic_catalog::{
    prepare_qualified_resource_reference_some_contract as prepare,
    qualified_resource_reference_some_contract_retained_bytes as retained,
    QualifiedSomeExpressionContractStorage as Storage,
};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn program(maximum: Option<u32>) -> Program {
    let reference = c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID)).unwrap();
    let optional = c::optional_info_type(reference.clone()).unwrap();
    Program {
        input_type: reference.clone(),
        input_maximum_bytes: maximum,
        output_type: optional.clone(),
        root: Node {
            value_type: optional,
            operation: Op::Variant {
                tag: "some".into(),
                payload: Box::new(Node {
                    value_type: reference,
                    operation: Op::Input,
                }),
            },
        },
    }
}
#[test]
fn v2_and_v3_descriptor_peak_and_actual_retained_fit_without_changing_identity() {
    for maximum in [None, Some(1), Some(512)] {
        let program = program(maximum);
        let encoded = program.canonical_hex().unwrap();
        let (receipt, preflight) =
            allocation_probe::observe(|| Storage::for_program(&program).unwrap());
        assert_eq!(preflight.allocations, 0);
        let (wire, preflight) =
            allocation_probe::observe(|| Storage::for_canonical_hex(&encoded).unwrap());
        assert_eq!(preflight.allocations, 0);
        assert_eq!(receipt, wire);
        for temporal in [
            c::PortTemporal::Value,
            c::PortTemporal::Flow { closes: true },
        ] {
            let expected =
                conduit_semantic_catalog::pure_expression_contract(&program, temporal).unwrap();
            let (kind, cost) = allocation_probe::observe(|| {
                prepare(&program, temporal, receipt.preparation_bytes).unwrap()
            });
            assert_eq!(kind, expected);
            assert!(
                cost.peak_bytes + core::mem::size_of::<c::Kind>() <= receipt.preparation_bytes,
                "peak {} plus inline {} exceeds {}",
                cost.peak_bytes,
                core::mem::size_of::<c::Kind>(),
                receipt.preparation_bytes
            );
            let (bytes, inspection) = allocation_probe::observe(|| retained(&kind).unwrap());
            assert_eq!(inspection.allocations, 0);
            assert!(bytes + core::mem::size_of::<c::Kind>() <= receipt.retained_bytes);
            let c::ConfigurationValue::Text(text) = &kind.configuration[0].default_value else {
                panic!("program")
            };
            assert_eq!(text, &encoded);
        }
    }
}
#[test]
fn descriptor_preflight_shortage_and_unsupported_inputs_refuse_without_allocation() {
    let mut program = program(None);
    let receipt = Storage::for_program(&program).unwrap();
    let (short, cost) = allocation_probe::observe(|| {
        prepare(
            &program,
            c::PortTemporal::Value,
            receipt.preparation_bytes - 1,
        )
    });
    assert!(matches!(short, Err(E::InsufficientPreparation)));
    assert_eq!(cost.allocations, 0);
    let (current, cost) = allocation_probe::observe(|| {
        prepare(
            &program,
            c::PortTemporal::Current,
            receipt.preparation_bytes,
        )
    });
    assert!(matches!(current, Err(E::UnsupportedProgram)));
    assert_eq!(cost.allocations, 0);
    program.root.operation = Op::Input;
    let (unsupported, cost) = allocation_probe::observe(|| Storage::for_program(&program));
    assert_eq!(unsupported, Err(E::UnsupportedProgram));
    assert_eq!(cost.allocations, 0);
}
#[test]
fn retained_inspection_refuses_unreviewed_nested_configuration_and_laws() {
    let program = program(None);
    let receipt = Storage::for_program(&program).unwrap();
    let mut kind = prepare(&program, c::PortTemporal::Value, receipt.preparation_bytes).unwrap();
    kind.semantic_laws
        .push(c::KindSemanticLaw::RealizationRequirement {
            property_profile: "test/unreviewed".into(),
            configuration_key: "test".into(),
        });
    assert_eq!(retained(&kind), Err(E::UnsupportedProgram));
}
