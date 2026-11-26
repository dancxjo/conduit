//! Codec-owned opaque references through ordinary prepared expression operations.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::*;
use conduit_plot::{
    PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program, PreparedPortableExpressionEvaluator as Evaluator,
};
fn ty() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap()
}
fn reference(maximum: bool) -> BoundedResourceRef {
    let name = if maximum {
        "r".repeat(MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES)
    } else {
        "resource/test".into()
    };
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: name.clone().into(),
        access_class: name.clone().into(),
        extent: ResourceExtent {
            bytes: 4096,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: maximum.then_some(TemporalInstant {
                ticks: 7,
                scale: TemporalScale::Milliseconds,
                clock_basis: name,
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            }),
        },
    }
}
fn input() -> Node {
    Node {
        value_type: ty(),
        operation: Op::Input,
    }
}
fn program(root: Node) -> Program {
    Program {
        input_maximum_bytes: None,
        input_type: ty(),
        output_type: root.value_type.clone(),
        root,
    }
}
#[test]
fn identity_preserves_exact_canonical_reference_and_codec_ceiling_without_allocation() {
    let program = program(input());
    assert_eq!(
        program.maximum_prepared_input_bytes().unwrap(),
        MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
    );
    assert_eq!(
        program.maximum_prepared_output_bytes().unwrap(),
        MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
    );
    let mut evaluator = Evaluator::new(&program).unwrap();
    let capacity = evaluator.output_capacity();
    for maximum in [false, true] {
        let bytes = reference(maximum).encode().unwrap();
        assert!(bytes.len() <= MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES);
        assert_eq!(
            allocation::allocations(|| assert_eq!(evaluator.evaluate(&bytes).unwrap(), bytes)),
            0
        );
        assert_eq!(evaluator.output_capacity(), capacity);
    }
}
#[test]
fn ordinary_some_constructor_keeps_exact_optional_type_and_opaque_payload() {
    let optional = optional_info_type(ty()).unwrap();
    let root = Node {
        value_type: optional.clone(),
        operation: Op::Variant {
            tag: "some".into(),
            payload: Box::new(input()),
        },
    };
    let program = program(root);
    let mut evaluator = Evaluator::new(&program).unwrap();
    let bytes = reference(true).encode().unwrap();
    let expected = program.evaluate(&bytes).unwrap();
    assert_eq!(
        allocation::allocations(|| assert_eq!(evaluator.evaluate(&bytes).unwrap(), expected)),
        0
    );
    let value = StructuredInfoValue::from_canonical_bytes(evaluator.output_bytes()).unwrap();
    assert_eq!(value.value_type(), &optional);
    let StructuredInfoValueShape::Variant {
        tag: "some",
        payload,
    } = value.shape()
    else {
        panic!("some variant")
    };
    assert_eq!(payload.value_type(), &ty());
    assert!(matches!(payload.shape(),StructuredInfoValueShape::Leaf(raw) if raw==bytes));
    let mut foreign = program.clone();
    foreign.root.value_type = StructuredInfoType::variant(
        kind_id("type/foreign@1"),
        vec![StructuredVariantCase::new("some", ty()).unwrap()],
    )
    .unwrap();
    assert!(
        Evaluator::new(&foreign).is_err(),
        "output and constructor Type cannot disagree"
    );
}
#[test]
fn malformed_or_oversized_reference_never_bypasses_validation_or_grows_storage() {
    let mut evaluator = Evaluator::new(&program(input())).unwrap();
    let valid = reference(false).encode().unwrap();
    let mut trailing = valid.clone();
    trailing.push(0);
    let mut foreign_version = valid.clone();
    foreign_version[0] = 255;
    let mut zero_identity = valid.clone();
    zero_identity[1..33].fill(0);
    for bad in [
        vec![],
        trailing,
        foreign_version,
        zero_identity,
        valid[..valid.len() - 1].to_vec(),
        vec![0; MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES + 1],
    ] {
        let capacity = evaluator.output_capacity();
        assert_eq!(
            allocation::allocations(|| assert!(evaluator.evaluate(&bad).is_err())),
            0
        );
        assert_eq!(evaluator.output_capacity(), capacity);
    }
    let mut limited = program(input());
    limited.input_maximum_bytes = Some(valid.len() as u32 - 1);
    assert!(Evaluator::new(&limited).unwrap().evaluate(&valid).is_err());
    assert_eq!(
        evaluator.evaluate(&valid).unwrap(),
        valid,
        "refusal does not poison subsequent valid identity"
    );
}
#[test]
fn exact_reference_equality_does_not_equate_distinct_generations() {
    let constant = StructuredInfoValue::leaf(ty(), reference(false).encode().unwrap()).unwrap();
    let root = Node {
        value_type: StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
        operation: Op::Binary {
            operator: conduit_plot::BinaryOperator::Equal,
            proven: false,
            left: Box::new(input()),
            right: Box::new(Node {
                value_type: ty(),
                operation: Op::Constant(constant),
            }),
        },
    };
    let mut evaluator = Evaluator::new(&program(root)).unwrap();
    let same = reference(false).encode().unwrap();
    let mut different = reference(false);
    different.lifetime.version = ResourceVersionIdentity::from_digest([3; 32]);
    let different = different.encode().unwrap();
    assert_eq!(
        allocation::allocations(|| {
            assert_eq!(evaluator.evaluate(&same).unwrap(), [1]);
            assert_eq!(evaluator.evaluate(&different).unwrap(), [0]);
        }),
        0
    );
}

#[test]
fn reference_atom_does_not_reinterpret_text_or_erase_nominal_identity() {
    let mut text_input = program(input());
    text_input.input_type = StructuredInfoType::leaf(kind_id("value/text")).unwrap();
    assert!(Evaluator::new(&text_input).is_err());
    let nominal = StructuredInfoType::nominal(kind_id("type/selected-reference@1"), ty()).unwrap();
    let nominal_program = Program {
        input_maximum_bytes: None,
        input_type: nominal.clone(),
        output_type: nominal.clone(),
        root: Node {
            value_type: nominal,
            operation: Op::Input,
        },
    };
    assert!(
        Evaluator::new(&nominal_program).is_err(),
        "unsupported nominal wrappers must not silently become raw ResourceRef"
    );
}
