//! Exact ResourceRef equality is representation identity, never a resource read.
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::*;
use conduit_plot::*;
fn checked(
    operator: &str,
    nested: bool,
    kind: &str,
) -> Result<CheckedSyntaxDocument, SyntaxCheckDiagnostic> {
    let member = if nested {
        "BoxedReference"
    } else {
        "ResourceRef"
    };
    let source = format!("type BoxedReference = {{\n reference: ResourceRef\n}}\ntype Pair = {{\n left: {member}\n right: {member}\n}}\nplot equality (input: Pair...| <= 4096B >> output: Boolean...|) = (.left {operator} .right)\n");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias("ResourceRef", kind_id(kind))
        .unwrap();
    check_syntax_document(&parse_syntax_document(&source), &catalog)
}
fn program(operator: &str, nested: bool, kind: &str) -> PortableExpressionProgram {
    let document = checked(operator, nested, kind).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&document, "equality", &ProfileCatalog::new())
            .unwrap()
            .expanded;
    let ConfigurationValue::Text(hex) = &expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    PortableExpressionProgram::from_canonical_hex(hex).unwrap()
}
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn value(ty: &StructuredInfoType, bytes: &[u8], nested: bool) -> StructuredInfoValue {
    if nested {
        StructuredInfoValue::record(
            ty.clone(),
            vec![StructuredFieldValue::new(
                "reference",
                StructuredInfoValue::leaf(field(ty, "reference"), bytes.to_vec()).unwrap(),
            )
            .unwrap()],
        )
        .unwrap()
    } else {
        StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()).unwrap()
    }
}
fn input(p: &PortableExpressionProgram, left: &[u8], right: &[u8], nested: bool) -> Vec<u8> {
    StructuredInfoValue::record(
        p.input_type.clone(),
        vec![
            StructuredFieldValue::new("left", value(&field(&p.input_type, "left"), left, nested))
                .unwrap(),
            StructuredFieldValue::new(
                "right",
                value(&field(&p.input_type, "right"), right, nested),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn reference() -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("test/content@1"),
        access_class: ResourceClassId::from("test/read@1"),
        extent: ResourceExtent {
            bytes: 100,
            items: Some(2),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}
#[test]
fn checked_source_reference_and_prepared_parity_covers_the_entire_reference() {
    let original = reference();
    let mut alternatives = vec![original.clone()];
    alternatives.push(BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([3; 32]),
        ..original.clone()
    });
    alternatives.push(BoundedResourceRef {
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([4; 32]),
            ..original.lifetime.clone()
        },
        ..original.clone()
    });
    alternatives.push(BoundedResourceRef {
        extent: ResourceExtent {
            bytes: 101,
            ..original.extent
        },
        ..original.clone()
    });
    alternatives.push(BoundedResourceRef {
        extent: ResourceExtent {
            items: None,
            ..original.extent
        },
        ..original.clone()
    });
    alternatives.push(BoundedResourceRef {
        extent: ResourceExtent {
            items: Some(3),
            ..original.extent
        },
        ..original.clone()
    });
    alternatives.push(BoundedResourceRef {
        content_profile: kind_id("test/other@1"),
        ..original.clone()
    });
    alternatives.push(BoundedResourceRef {
        access_class: ResourceClassId::from("test/other-read@1"),
        ..original.clone()
    });
    alternatives.push(BoundedResourceRef {
        lifetime: ResourceLifetime {
            expires_at: Some(TemporalInstant {
                ticks: 20,
                scale: TemporalScale::Seconds,
                clock_basis: "test/clock@1".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            }),
            ..original.lifetime.clone()
        },
        ..original.clone()
    });
    let mut different_expiry = alternatives.last().unwrap().clone();
    different_expiry.lifetime.expires_at.as_mut().unwrap().ticks += 1;
    alternatives.push(different_expiry);
    for nested in [false, true] {
        for operator in ["==", "!="] {
            let p = program(operator, nested, RESOURCE_REFERENCE_INFO_ID);
            let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
            let capacity = prepared.output_capacity();
            for original in &alternatives {
                for other in &alternatives {
                    let left = original.encode().unwrap();
                    let right = other.encode().unwrap();
                    // No alternate encoding of the admitted field values is accepted.
                    assert_eq!(
                        BoundedResourceRef::decode(&right)
                            .unwrap()
                            .encode()
                            .unwrap(),
                        right
                    );
                    let bytes = input(&p, &left, &right, nested);
                    let expected = if operator == "==" {
                        original == other
                    } else {
                        original != other
                    };
                    assert_eq!(
                        InfoBool::decode(&p.evaluate(&bytes).unwrap())
                            .unwrap()
                            .get(),
                        expected
                    );
                    let (actual, allocation) = allocation_probe::observe(|| {
                        InfoBool::decode(prepared.evaluate(&bytes).unwrap())
                            .unwrap()
                            .get()
                    });
                    assert_eq!(actual, expected);
                    assert_eq!(allocation.allocations, 0);
                    assert_eq!(allocation.reallocations, 0);
                    assert_eq!(allocation.peak_bytes, 0);
                    assert_eq!(prepared.output_capacity(), capacity);
                }
            }
        }
    }
}
#[test]
fn malformed_reference_payloads_refuse_even_when_both_bytes_are_identical() {
    let valid = reference().encode().unwrap();
    let mut zero_identity = valid.clone();
    zero_identity[1..33].fill(0);
    let mut zero_version = valid.clone();
    zero_version[33..65].fill(0);
    let mut wrong_version = valid.clone();
    wrong_version[0] = 2;
    let mut bad_expiry_tag = valid.clone();
    *bad_expiry_tag.last_mut().unwrap() = 2;
    let mut trailing = valid.clone();
    trailing.push(0);
    let invalid = [
        vec![],
        valid[..valid.len() - 1].to_vec(),
        zero_identity,
        zero_version,
        wrong_version,
        bad_expiry_tag,
        trailing,
    ];
    for nested in [false, true] {
        for operator in ["==", "!="] {
            let p = program(operator, nested, RESOURCE_REFERENCE_INFO_ID);
            let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
            for invalid in &invalid {
                assert!(BoundedResourceRef::validate_encoded(invalid).is_err());
                for right in [invalid.as_slice(), valid.as_slice()] {
                    // Generic structured admission does not validate opaque Core
                    // leaves: equality itself must execute the reference laws.
                    let bytes = input(&p, invalid, right, nested);
                    assert!(p.evaluate(&bytes).is_err());
                    assert!(prepared.evaluate(&bytes).is_err());
                }
            }
        }
    }
}
#[test]
fn source_does_not_admit_reference_ordering_or_other_opaque_leaf_equality() {
    for op in ["<", "<=", ">", ">="] {
        assert!(checked(op, false, RESOURCE_REFERENCE_INFO_ID).is_err());
    }
    let p = program("==", false, "test/unsupported-opaque@1");
    let bytes = input(&p, b"same", b"same", false);
    assert!(matches!(
        p.evaluate(&bytes),
        Err(PortableExpressionEvaluationRefusal::UnsupportedType(_))
    ));
    // Preparation may construct a retained structured selector, but evaluation
    // must retain the existing unsupported-kind refusal.
    match PreparedPortableExpressionEvaluator::new(&p) {
        Ok(mut prepared) => assert!(prepared.evaluate(&bytes).is_err()),
        Err(PortableExpressionEvaluationRefusal::UnsupportedType(_)) => (),
        Err(other) => panic!("unexpected refusal: {other:?}"),
    }
}
