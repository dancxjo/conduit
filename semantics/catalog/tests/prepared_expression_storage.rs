#![cfg(feature = "kernel-step")]
//! Back composition retains the owning evaluator's full storage receipt.
use conduit_core as c;
use conduit_plot::{
    PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program,
};
use conduit_semantic_catalog::PreparedExpressionBack;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn program() -> Program {
    let reference = c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID)).unwrap();
    let optional = c::optional_info_type(reference.clone()).unwrap();
    Program {
        input_type: reference.clone(),
        input_maximum_bytes: None,
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
fn qualified_back_admits_full_nested_storage_before_preparation() {
    let program = program();
    let receipt =
        PreparedExpressionBack::qualified_resource_reference_some_storage(&program).unwrap();
    let (refused, allocations) = allocation_probe::observe(|| {
        PreparedExpressionBack::prepare_qualified_resource_reference_some(
            &program,
            c::PortTemporal::Value,
            receipt.preparation_bytes - 1,
        )
    });
    assert!(matches!(
        refused,
        Err(conduit_plot::QualifiedExpressionStorageRefusal::InsufficientPreparation)
    ));
    assert_eq!(allocations.allocations, 0);
    let (prepared, allocations) = allocation_probe::observe(|| {
        PreparedExpressionBack::prepare_qualified_resource_reference_some(
            &program,
            c::PortTemporal::Value,
            receipt.preparation_bytes,
        )
        .unwrap()
    });
    let inline = core::mem::size_of::<PreparedExpressionBack>();
    assert!(inline + allocations.peak_bytes <= receipt.preparation_bytes);
    assert!(
        inline + prepared.qualified_retained_backing_bytes().unwrap() <= receipt.retained_bytes
    );
    assert!(prepared.qualified_retained_backing_bytes().unwrap() > prepared.output_capacity());
}
