//! Explicit prepared coordinate profile; no installed Source availability claim.
use conduit_core::*;
use conduit_plot::{
    BinaryOperator as Op, PortableExpressionNode as Node, PortableExpressionOperation as Expr,
    PortableExpressionProgram as Program, PreparedPortableExpressionEvaluator as Evaluator,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static TRACK:Cell<bool>=const {Cell::new(false)};
    static ALLOCATIONS:Cell<usize>=const {Cell::new(0)};
}
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, n: usize) -> *mut u8 {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|v| v.set(v.get() + 1));
        }
        unsafe { System.realloc(ptr, layout, n) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).unwrap()
}
fn arithmetic(op: Op) -> Program {
    let quantity = leaf(DISTANCE_INFO_ID);
    Program {
        input_type: quantity.clone(),
        input_maximum_bytes: None,
        output_type: quantity.clone(),
        root: Node {
            value_type: quantity.clone(),
            operation: Expr::Binary {
                operator: op,
                proven: false,
                left: Box::new(Node {
                    value_type: quantity,
                    operation: Expr::Input,
                }),
                right: Box::new(Node {
                    value_type: leaf(SCALAR_INFO_ID),
                    operation: Expr::Literal("3".into()),
                }),
            },
        },
    }
}
fn equality() -> Program {
    let quantity = leaf(DISTANCE_INFO_ID);
    Program {
        input_type: quantity.clone(),
        input_maximum_bytes: None,
        output_type: leaf(BOOL_INFO_ID),
        root: Node {
            value_type: leaf(BOOL_INFO_ID),
            operation: Expr::Binary {
                operator: Op::Equal,
                proven: false,
                left: Box::new(Node {
                    value_type: quantity.clone(),
                    operation: Expr::Input,
                }),
                right: Box::new(Node {
                    value_type: quantity,
                    operation: Expr::CanonicalLiteral(
                        Quantity::parse_plot_literal("1m")
                            .unwrap()
                            .encode()
                            .to_vec(),
                    ),
                }),
            },
        },
    }
}
#[test]
fn independent_prepared_gears_transport_exact_thirds_without_warm_allocation_or_growth() {
    let mut divide = Evaluator::new_with_exact_quantities(&arithmetic(Op::Divide)).unwrap();
    let mut multiply = Evaluator::new_with_exact_quantities(&arithmetic(Op::Multiply)).unwrap();
    let mut compare = Evaluator::new_with_exact_quantities(&equality()).unwrap();
    let native = Quantity::parse_plot_literal("1m").unwrap().encode();
    let capacity = divide.output_capacity();
    assert_eq!(capacity, QUANTITY_VALUE_MAX_ENCODED_BYTES);
    let pointers = (
        divide.output_bytes().as_ptr(),
        multiply.output_bytes().as_ptr(),
    );
    let third = divide.evaluate(&native).unwrap().to_vec();
    assert_eq!(third[0], 2);
    assert!(third.len() < QUANTITY_ENCODED_LEN);
    let mut legacy = Evaluator::new(&arithmetic(Op::Multiply)).unwrap();
    assert!(legacy.evaluate(&third).is_err());
    ALLOCATIONS.with(|n| n.set(0));
    TRACK.with(|v| v.set(true));
    for _ in 0..128 {
        let third = divide.evaluate(&native).unwrap();
        let restored = multiply.evaluate(third).unwrap();
        assert_eq!(compare.evaluate(restored).unwrap(), [1]);
    }
    TRACK.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(divide.output_capacity(), capacity);
    assert_eq!(
        (
            divide.output_bytes().as_ptr(),
            multiply.output_bytes().as_ptr()
        ),
        pointers
    );
}
#[test]
fn explicit_input_envelopes_and_composite_profile_refusals_remain_before_execution() {
    let mut work = ExactNumericWork::new(4 * MATHEMATICAL_VALUE_VALIDATION_WORK);
    let coordinate =
        ExactRational::parse_decimal(&format!("1{}1", "0".repeat(500)), &mut work).unwrap();
    let native = Quantity::parse_plot_literal("1m").unwrap();
    let value = RationalQuantity::new(coordinate, native.unit(), QuantityRole::Linear).unwrap();
    let mut bytes = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(value, &mut bytes).unwrap();
    bytes.truncate(n);
    assert!(n > QUANTITY_ENCODED_LEN);
    let mut program = arithmetic(Op::Multiply);
    program.input_maximum_bytes = Some(QUANTITY_ENCODED_LEN as u32);
    let mut bounded = Evaluator::new_with_exact_quantities(&program).unwrap();
    assert!(bounded.evaluate(&bytes).is_err());
    program.input_maximum_bytes = Some(QUANTITY_VALUE_MAX_ENCODED_BYTES as u32);
    let mut admitted = Evaluator::new_with_exact_quantities(&program).unwrap();
    assert!(admitted.evaluate(&bytes).is_ok());
    let record = StructuredInfoType::record(
        kind_id("test/quantity-record@1"),
        vec![StructuredFieldType::new("q", leaf(QUANTITY_INFO_ID)).unwrap()],
    )
    .unwrap();
    let program = Program {
        input_type: record.clone(),
        input_maximum_bytes: None,
        output_type: record.clone(),
        root: Node {
            value_type: record,
            operation: Expr::Input,
        },
    };
    assert!(matches!(
        Evaluator::new_with_exact_quantities(&program),
        Err(conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedType(_))
    ));
}

#[test]
fn native_extreme_decimal_comparisons_preserve_the_existing_contract() {
    let native = Quantity::parse_plot_literal("1m").unwrap();
    let native = Quantity::from_decimal(1, QUANTITY_MAX_EXPONENT, native.unit())
        .unwrap()
        .encode();
    let mut program = equality();
    let Expr::Binary { right, .. } = &mut program.root.operation else {
        panic!("binary")
    };
    right.operation = Expr::CanonicalLiteral(native.to_vec());
    assert_eq!(
        Evaluator::new(&program).unwrap().evaluate(&native).unwrap(),
        [1]
    );
    assert_eq!(
        Evaluator::new_with_exact_quantities(&program)
            .unwrap()
            .evaluate(&native)
            .unwrap(),
        [1]
    );
}

#[test]
fn a_structured_intermediate_cannot_hide_inside_primitive_exact_ports() {
    let quantity = leaf(DISTANCE_INFO_ID);
    let record = StructuredInfoType::record(
        kind_id("test/intermediate@1"),
        vec![StructuredFieldType::new("q", quantity.clone()).unwrap()],
    )
    .unwrap();
    let program = Program {
        input_type: quantity.clone(),
        input_maximum_bytes: None,
        output_type: quantity.clone(),
        root: Node {
            value_type: quantity.clone(),
            operation: Expr::Projection {
                value: Box::new(Node {
                    value_type: record,
                    operation: Expr::Record(vec![(
                        "q".into(),
                        Node {
                            value_type: quantity,
                            operation: Expr::Input,
                        },
                    )]),
                }),
                member: conduit_plot::PortableExpressionProjection::Field("q".into()),
            },
        },
    };
    let native = Quantity::parse_plot_literal("1m").unwrap().encode();
    assert_eq!(
        Evaluator::new(&program).unwrap().evaluate(&native).unwrap(),
        native
    );
    assert!(matches!(
        Evaluator::new_with_exact_quantities(&program),
        Err(conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedType(_))
    ));
}

#[test]
fn semantic_helpers_without_the_exact_codec_refuse_during_profile_admission() {
    let angle = leaf(ANGLE_INFO_ID);
    let program = Program {
        input_type: angle.clone(),
        input_maximum_bytes: None,
        output_type: leaf(MATHEMATICAL_VALUE_ID),
        root: Node {
            value_type: leaf(MATHEMATICAL_VALUE_ID),
            operation: Expr::SemanticCall {
                kind: "math/sin".into(),
                arguments: vec![Node {
                    value_type: angle,
                    operation: Expr::Input,
                }],
            },
        },
    };
    let native = Quantity::parse_plot_literal("0.25turn").unwrap().encode();
    assert!(Evaluator::new(&program).unwrap().evaluate(&native).is_ok());
    assert!(matches!(
        Evaluator::new_with_exact_quantities(&program),
        Err(conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedType(_))
    ));
}
