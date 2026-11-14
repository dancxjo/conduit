//! Linked program storage stays immutable; preparing it still checks every guard.
use conduit_core::PrimitiveInfoKind as K;
use conduit_plot::{
    rust_binding::ResidentNativeValidator, ScalarTapeInstruction as I, ScalarTapeOperation as Op,
    StaticProjectedExpression as Expression, StaticProjectedProgram as Program,
    StaticStructuredExpression,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

static CODE: [I; 1] = [I {
    kind: K::U8,
    destination: 0,
    operation: Op::Literal(alloc::borrow::Cow::Borrowed(&[42])),
}];
extern crate alloc;
static PROGRAM: Program = Program {
    maximum_input_bytes: 0,
    output: K::U8,
    instructions: &CODE,
    result: 0,
    inputs: &[],
    output_contract: None,
};

#[test]
fn linked_child_state_is_bounded_by_references_and_execution_does_not_allocate() {
    // Child count scales the retained owner. Code/operands remain in the linked
    // program instead of reserving a duplicate tape description for every child.
    let growth = core::mem::size_of::<StaticStructuredExpression<0, 0, 1>>()
        - core::mem::size_of::<StaticStructuredExpression<0, 0, 0>>();
    assert!(growth <= 3 * core::mem::size_of::<usize>(), "{growth}");
    let (expression, cost) = allocation_probe::observe(|| {
        Expression::new(
            ResidentNativeValidator::intrinsic_empty(64).unwrap(),
            &PROGRAM,
        )
        .unwrap()
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    let mut output = [0xa5; 2];
    let (result, cost) = allocation_probe::observe(|| {
        for _ in 0..32 {
            assert_eq!(expression.evaluate_into(&[], &mut output).unwrap(), 1);
            assert_eq!(output, [42, 0xa5]);
        }
        expression.evaluate_into(&[0], &mut output)
    });
    assert!(result.is_err());
    assert_eq!(output, [42, 0xa5]);
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}

#[test]
fn linked_program_is_not_a_preparation_witness() {
    static WRONG_RESULT: Program = Program {
        result: 1,
        ..PROGRAM
    };
    static INPUT_CODE: [I; 1] = [I {
        kind: K::U8,
        destination: 0,
        operation: Op::Input,
    }];
    static MISSING_DESCRIPTOR: Program = Program {
        instructions: &INPUT_CODE,
        ..PROGRAM
    };
    for program in [&WRONG_RESULT, &MISSING_DESCRIPTOR] {
        assert!(Expression::new(
            ResidentNativeValidator::intrinsic_empty(64).unwrap(),
            program,
        )
        .is_err());
    }
}

#[test]
fn descriptor_loop_refuses_later_children_without_publishing_partial_preparation() {
    use conduit_plot::{
        StaticExpressionPathStep as Path, StaticProjectedRefusal,
        StaticStructuredChildProgram as C, StaticStructuredProgram, StaticStructuredRefusal,
        StructuredTapeInstruction as Op,
    };
    static ROOT: StaticStructuredProgram = StaticStructuredProgram {
        instructions: &[Op::Value(0)],
        constants: &[],
        maximum_input_bytes: 0,
        maximum_output_bytes: 1,
    };
    static BAD_SCALAR: Program = Program {
        result: 1,
        ..PROGRAM
    };
    static CHILDREN: [C; 3] = [
        C::Scalar(&PROGRAM),
        C::Scalar(&BAD_SCALAR),
        C::Scalar(&PROGRAM),
    ];
    static BAD_PATH: [C; 2] = [C::Scalar(&PROGRAM), C::InputPath(&[Path::Field("")])];
    let native = || ResidentNativeValidator::intrinsic_empty(64).unwrap();
    let (result, cost) = allocation_probe::observe(|| {
        StaticStructuredExpression::from_child_programs(native(), native(), &CHILDREN, &ROOT)
    });
    assert!(matches!(
        result,
        Err(StaticStructuredRefusal::Child {
            index: 1,
            refusal: StaticProjectedRefusal::Expression(_),
        })
    ));
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert!(matches!(
        StaticStructuredExpression::from_child_programs(native(), native(), &BAD_PATH, &ROOT),
        Err(StaticStructuredRefusal::Expression(_))
    ));
}
