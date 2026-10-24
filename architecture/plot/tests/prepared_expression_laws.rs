//! Selected-node Source laws, without claiming an installed expression Back.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const SOURCE: &str =
    "type Ordered = {\n hi: U8\n lo: U8\n where .lo <= .hi\n}\ntype Small = U8 in 0..=9\ntype Choice =\n known Ordered\n | missing\ntype Holder = {\n value: Ordered?\n}\n";
fn fixture() -> (NativeLawRegistry, StructuredInfoType, StructuredInfoType) {
    let doc =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let ordered = doc.native_law_witness("Ordered").unwrap();
    let small = doc.native_law_witness("Small").unwrap();
    (
        NativeLawRegistry::new(
            &doc.native_types
                .iter()
                .map(|ty| doc.native_law_witness(&ty.name).unwrap().clone())
                .collect::<Vec<_>>(),
        )
        .unwrap(),
        ordered.checked_type().value_type.clone(),
        small.checked_type().value_type.clone(),
    )
}
fn bounds() -> PreparedExpressionLawBounds {
    PreparedExpressionLawBounds {
        guards: 128,
        native: PreparedNativeBounds {
            canonical_bytes: 1 << 20,
            scopes: 128,
            invariants: 128,
            contracts: 128,
        },
        occurrences_per_guard: 16,
        steps: 65536,
    }
}
fn ty(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).unwrap()
}
fn literal(kind: &str, value: &str) -> PortableExpressionNode {
    PortableExpressionNode {
        value_type: ty(kind),
        operation: PortableExpressionOperation::Literal(value.into()),
    }
}
fn record(ordered: &StructuredInfoType, lo: &str, hi: &str) -> PortableExpressionNode {
    PortableExpressionNode {
        value_type: ordered.clone(),
        operation: PortableExpressionOperation::Record(vec![
            ("hi".into(), literal("value/u8", hi)),
            ("lo".into(), literal("value/u8", lo)),
        ]),
    }
}
fn program(root: PortableExpressionNode) -> PortableExpressionProgram {
    PortableExpressionProgram {
        input_maximum_bytes: Some(1),
        input_type: ty("value/bool"),
        output_type: root.value_type.clone(),
        root,
    }
}
fn prepare(
    p: &PortableExpressionProgram,
    r: &NativeLawRegistry,
) -> PreparedPortableExpressionEvaluator {
    PreparedPortableExpressionEvaluator::new_checked_native_laws(p, r, bounds()).unwrap()
}
fn bytes(ordered: &StructuredInfoType, lo: u8, hi: u8) -> Vec<u8> {
    let field = |name: &str, n: u8| {
        StructuredFieldValue::new(name, primitive_into_structured(ty("value/u8"), &n).unwrap())
            .unwrap()
    };
    StructuredInfoValue::record(ordered.clone(), vec![field("hi", hi), field("lo", lo)])
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn complete_unused_native_input_is_checked_before_a_constant_boolean_body() {
    let (registry, ordered, _) = fixture();
    let mut p = program(literal("value/bool", "true"));
    p.input_type = ordered.clone();
    p.input_maximum_bytes = Some(4096);
    let bad = bytes(&ordered, 2, 1);
    assert_eq!(
        PreparedPortableExpressionEvaluator::new_checked_arithmetic(&p)
            .unwrap()
            .evaluate(&bad)
            .unwrap(),
        &[1]
    );
    let mut e = prepare(&p, &registry);
    assert!(matches!(
        e.evaluate(&bad),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::Invariant { .. }
        ))
    ));
    assert!(matches!(
        e.evaluate(&bytes(&ordered, 1, 2)),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::State
        ))
    ));
}
#[test]
fn invalid_temporary_record_refuses_before_its_scalar_projection_is_consumed() {
    let (registry, ordered, _) = fixture();
    let p = program(PortableExpressionNode {
        value_type: ty("value/u8"),
        operation: PortableExpressionOperation::Projection {
            value: Box::new(record(&ordered, "2", "1")),
            member: PortableExpressionProjection::Field("hi".into()),
        },
    });
    assert_eq!(
        PreparedPortableExpressionEvaluator::new_checked_arithmetic(&p)
            .unwrap()
            .evaluate(&[0])
            .unwrap(),
        &[1]
    );
    let mut e = prepare(&p, &registry);
    assert!(matches!(
        e.evaluate(&[0]),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::Invariant { .. }
        ))
    ));
}
#[test]
fn lazy_dead_branch_stays_unexecuted_and_selected_failure_is_terminal_across_branches() {
    let (registry, ordered, _) = fixture();
    let p = program(PortableExpressionNode {
        value_type: ordered.clone(),
        operation: PortableExpressionOperation::Conditional {
            condition: Box::new(PortableExpressionNode {
                value_type: ty("value/bool"),
                operation: PortableExpressionOperation::Input,
            }),
            when_true: Box::new(record(&ordered, "2", "1")),
            when_false: Box::new(record(&ordered, "1", "2")),
        },
    });
    let mut e = prepare(&p, &registry);
    let valid = bytes(&ordered, 1, 2);
    assert_eq!(e.evaluate(&[0]).unwrap(), valid);
    assert!(matches!(
        e.evaluate(&[1]),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::Invariant { .. }
        ))
    ));
    assert!(matches!(
        e.evaluate(&[0]),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::State
        ))
    ));
}
#[test]
fn nominal_primitive_refinement_is_checked_with_exact_canonical_framing() {
    let (registry, _, small) = fixture();
    let p = program(PortableExpressionNode {
        value_type: small,
        operation: PortableExpressionOperation::Literal("10".into()),
    });
    let mut e = prepare(&p, &registry);
    assert!(matches!(
        e.evaluate(&[0]),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::Contract { .. }
        ))
    ));
}
#[test]
fn missing_owner_aggregate_guard_pressure_and_cancellation_refuse_explicitly() {
    let (registry, ordered, _) = fixture();
    let p = program(record(&ordered, "1", "2"));
    let empty = NativeLawRegistry::new(&[]).unwrap();
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new_checked_native_laws(&p, &empty, bounds()),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::MissingNativeMetadata
        ))
    ));
    let mut b = bounds();
    b.guards = 1;
    let pair = program(PortableExpressionNode {
        value_type: StructuredInfoType::collection(ordered.clone(), Some(2)).unwrap(),
        operation: PortableExpressionOperation::Collection(vec![
            record(&ordered, "1", "2"),
            record(&ordered, "1", "2"),
        ]),
    });
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new_checked_native_laws(&pair, &registry, b),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::Bounds
        ))
    ));
    let mut e = prepare(&p, &registry);
    e.cancel_checked_native_laws();
    assert!(matches!(
        e.evaluate(&[0]),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::Cancelled
        ))
    ));
}
#[test]
fn reused_success_and_terminal_refusal_do_not_allocate_or_reallocate() {
    let (registry, ordered, _) = fixture();
    let good = program(record(&ordered, "1", "2"));
    let bad = program(record(&ordered, "2", "1"));
    let mut e = prepare(&good, &registry);
    let mut refused = prepare(&bad, &registry);
    let (_, allocation) = allocation_probe::observe(|| {
        for _ in 0..8 {
            assert!(e.evaluate(&[0]).is_ok());
        }
        assert!(matches!(
            refused.evaluate(&[0]),
            Err(PortableExpressionEvaluationRefusal::NativeLaw(
                PreparedNativeRefusal::Invariant { .. }
            ))
        ));
        assert!(refused.evaluate(&[0]).is_err());
    });
    assert_eq!(allocation.allocations, 0);
    assert_eq!(allocation.reallocations, 0);
}

#[test]
fn native_variant_and_core_optional_preserve_case_selection_and_nested_laws() {
    let (registry, ordered, _) = fixture();
    let choice = registry
        .source_witnesses()
        .iter()
        .find(|w| w.checked_type().name == "Choice")
        .unwrap()
        .checked_type()
        .value_type
        .clone();
    let holder = &registry
        .source_witnesses()
        .iter()
        .find(|w| w.checked_type().name == "Holder")
        .unwrap()
        .checked_type()
        .value_type;
    let StructuredInfoTypeShape::Record { fields, .. } = holder.shape() else {
        panic!()
    };
    let optional = fields[0].value_type().clone();
    for (variant, present, absent) in [(choice, "known", "missing"), (optional, "some", "none")] {
        let none = program(PortableExpressionNode {
            value_type: variant.clone(),
            operation: PortableExpressionOperation::Variant {
                tag: absent.into(),
                payload: Box::new(literal(EMPTY_INFO_ID, "empty")),
            },
        });
        assert!(prepare(&none, &registry).evaluate(&[0]).is_ok());
        for (lo, hi, valid) in [("1", "2", true), ("2", "1", false)] {
            let some = program(PortableExpressionNode {
                value_type: variant.clone(),
                operation: PortableExpressionOperation::Variant {
                    tag: present.into(),
                    payload: Box::new(record(&ordered, lo, hi)),
                },
            });
            let result = prepare(&some, &registry).evaluate(&[0]).map(|_| ());
            if valid {
                result.unwrap()
            } else {
                assert!(matches!(
                    result,
                    Err(PortableExpressionEvaluationRefusal::NativeLaw(
                        PreparedNativeRefusal::Invariant { .. }
                    ))
                ))
            }
        }
    }
}

#[test]
fn unequal_earlier_equality_field_cannot_hide_invalid_later_native_value() {
    let (registry, ordered, _) = fixture();
    let tuple = tuple_info_type(vec![ty("value/u8"), ordered.clone()]).unwrap();
    let side = |prefix: &str, lo: &str, hi: &str| PortableExpressionNode {
        value_type: tuple.clone(),
        operation: PortableExpressionOperation::Tuple(vec![
            literal("value/u8", prefix),
            record(&ordered, lo, hi),
        ]),
    };
    let p = program(PortableExpressionNode {
        value_type: ty("value/bool"),
        operation: PortableExpressionOperation::Binary {
            operator: BinaryOperator::Equal,
            proven: false,
            left: Box::new(side("1", "2", "1")),
            right: Box::new(side("2", "1", "2")),
        },
    });
    assert_eq!(
        PreparedPortableExpressionEvaluator::new_checked_arithmetic(&p)
            .unwrap()
            .evaluate(&[0])
            .unwrap(),
        &[0]
    );
    assert!(matches!(
        prepare(&p, &registry).evaluate(&[0]),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::Invariant { .. }
        ))
    ));
}

#[test]
fn native_law_profile_checks_forged_arithmetic_hints_and_is_send_on_host() {
    fn send<T: Send>() {}
    send::<PreparedPortableExpressionEvaluator>();
    let (registry, _, _) = fixture();
    let p = program(PortableExpressionNode {
        value_type: ty("value/u8"),
        operation: PortableExpressionOperation::Binary {
            operator: BinaryOperator::Add,
            proven: true,
            left: Box::new(literal("value/u8", "255")),
            right: Box::new(literal("value/u8", "1")),
        },
    });
    let original = p.canonical_bytes().unwrap();
    assert!(matches!(
        prepare(&p, &registry).evaluate(&[0]),
        Err(PortableExpressionEvaluationRefusal::Arithmetic)
    ));
    assert_eq!(p.canonical_bytes().unwrap(), original);
}
