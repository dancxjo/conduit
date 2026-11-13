extern crate alloc;
pub type Back = conduit_semantic_catalog::StaticStructuredExpressionBack<
    { input_native::CONTRACT_COUNT },
    { output_native::CONTRACT_COUNT },
    { CHILD_COUNT },
    2048,
>;
pub fn back(temporal: conduit_core::PortTemporal) -> Back {
    Back::from_resident_structured(prepare().unwrap(), temporal).unwrap()
}
pub fn refuse_bounds() {
    type Small = conduit_semantic_catalog::StaticStructuredExpressionBack<
        { input_native::CONTRACT_COUNT },
        { output_native::CONTRACT_COUNT },
        { CHILD_COUNT },
        1,
    >;
    assert!(matches!(
        Small::from_resident_structured(prepare().unwrap(), conduit_core::PortTemporal::Value),
        Err(conduit_semantic_catalog::StaticExpressionBackRefusal::Bounds)
    ));
    assert!(matches!(
        Back::from_resident_structured(
            prepare().unwrap(),
            conduit_core::PortTemporal::Flow { closes: false }
        ),
        Err(conduit_semantic_catalog::StaticExpressionBackRefusal::UnsupportedTemporal)
    ));
}

pub fn refuse_excess_work_and_mismatched_child_guards() {
    use conduit_core::PrimitiveInfoKind as K;
    use conduit_plot::{
        ScalarTapeInstruction as I, ScalarTapeOperation as Op, StaticStructuredValue as V,
    };
    static LITERALS: [I; 128] = [const {
        I {
            kind: K::Bool,
            destination: 0,
            operation: Op::Literal(alloc::borrow::Cow::Borrowed(&[0])),
        }
    }; 128];
    static LARGE_CODE: [conduit_plot::StructuredTapeInstruction; 512] =
        [conduit_plot::StructuredTapeInstruction::Value(0); 512];
    static LARGE: conduit_plot::StaticStructuredProgram = conduit_plot::StaticStructuredProgram {
        instructions: &LARGE_CODE,
        constants: &[],
        maximum_input_bytes: 4096,
        maximum_output_bytes: 2048,
    };
    static SCALAR_PROGRAM: conduit_plot::StaticProjectedProgram =
        conduit_plot::StaticProjectedProgram {
            maximum_input_bytes: 4096,
            output: K::Bool,
            instructions: &LITERALS,
            result: 0,
            inputs: &[],
            output_contract: None,
        };
    let scalar = conduit_plot::StaticProjectedExpression::new(
        input_native::prepare().unwrap(),
        &SCALAR_PROGRAM,
    )
    .unwrap();
    let image = conduit_plot::StaticStructuredExpression::new(
        input_native::prepare().unwrap(),
        output_native::prepare().unwrap(),
        [V::Scalar(scalar)],
        &LARGE,
    )
    .unwrap();
    assert!(image.computation_work() > u16::MAX as u32 - 2);
    type Large = conduit_semantic_catalog::StaticStructuredExpressionBack<
        { input_native::CONTRACT_COUNT },
        { output_native::CONTRACT_COUNT },
        1,
        2048,
    >;
    assert!(matches!(
        Large::from_resident_structured(image, conduit_core::PortTemporal::Value),
        Err(conduit_semantic_catalog::StaticExpressionBackRefusal::Work)
    ));
    // Same schema and contract count cannot substitute a different retained
    // predicate table when child evaluation reuses the parent's checked input.
    use conduit_core::{
        FixedIntegerContractSpec as Spec, FixedIntegerRange as Range, IntervalEndpoint::Inclusive,
    };
    use conduit_plot::rust_binding::{
        ResidentNativeValidator, StaticNativeContract as Contract, StaticNativePathStep as Path,
    };
    static PREDICATE: Spec<'static> = Spec {
        value_kind: "value/u8",
        maximum_bytes: 1,
        range: Some(Range {
            minimum: Some(&[1]),
            maximum: Some(&[7]),
            minimum_endpoint: Inclusive,
            maximum_endpoint: Inclusive,
        }),
    };
    static CONTRACTS: [Contract; 1] = [Contract {
        scope: 0,
        contract: 0,
        path: &[Path::Field("address")],
        predicate: &PREDICATE,
    }];
    static SCOPES: [&[Path]; 0] = [];
    let fake = ResidentNativeValidator::new(
        input_native::prepare().unwrap().schema_bytes(),
        4096,
        &CONTRACTS,
        &SCOPES,
        4096,
    )
    .unwrap();
    let scalar = conduit_plot::StaticProjectedExpression::new(fake, &SCALAR_PROGRAM).unwrap();
    assert!(conduit_plot::StaticStructuredExpression::new(
        input_native::prepare().unwrap(),
        output_native::prepare().unwrap(),
        [V::Scalar(scalar)],
        &LARGE
    )
    .is_err());
}
