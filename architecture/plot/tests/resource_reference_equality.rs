#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
// Exact portable-reference equality; possession is not read authority.
use conduit_core as c;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
fn program(operator: &str, nested: bool) -> PortableExpressionProgram {
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    let member = if nested { "Wrapped" } else { "ResourceRef" };
    let source = format!("type Wrapped = {{\n    reference: ResourceRef\n}}\ntype Pair = {{\n    left: {member}\n    right: {member}\n}}\nplot equal (\n    >> input: Pair\n    result: Boolean >>\n) = (.left {operator} .right)");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "equal", &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let c::ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn reference() -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: "lexicon/test@1".into(),
        access_class: "resource/read@1".into(),
        extent: c::ResourceExtent {
            bytes: 12,
            items: Some(2),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}
fn field<'a>(ty: &'a c::StructuredInfoType, name: &str) -> &'a c::StructuredInfoType {
    let c::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn value(
    ty: &c::StructuredInfoType,
    reference: &c::BoundedResourceRef,
    nested: bool,
) -> c::StructuredInfoValue {
    if nested {
        c::StructuredInfoValue::record(
            ty.clone(),
            vec![c::StructuredFieldValue::new(
                "reference",
                c::StructuredInfoValue::leaf(
                    field(ty, "reference").clone(),
                    reference.encode().unwrap(),
                )
                .unwrap(),
            )
            .unwrap()],
        )
        .unwrap()
    } else {
        c::StructuredInfoValue::leaf(ty.clone(), reference.encode().unwrap()).unwrap()
    }
}
#[test]
fn direct_and_nested_equality_preserve_every_reference_field() {
    let original = reference();
    let mut alternatives = Vec::new();
    let mut changed = original.clone();
    changed.identity = c::ResourceSemanticIdentity::from_digest([3; 32]);
    alternatives.push(changed);
    let mut changed = original.clone();
    changed.content_profile = "lexicon/other@1".into();
    alternatives.push(changed);
    let mut changed = original.clone();
    changed.access_class = "resource/other@1".into();
    alternatives.push(changed);
    let mut changed = original.clone();
    changed.extent.bytes += 1;
    alternatives.push(changed);
    let mut changed = original.clone();
    changed.extent.items = Some(3);
    alternatives.push(changed);
    let mut changed = original.clone();
    changed.extent.items = None;
    alternatives.push(changed);
    let mut changed = original.clone();
    changed.lifetime.version = c::ResourceVersionIdentity::from_digest([4; 32]);
    alternatives.push(changed);
    let mut changed = original.clone();
    changed.lifetime.expires_at = Some(c::TemporalInstant {
        ticks: 2000,
        scale: c::TemporalScale::Milliseconds,
        clock_basis: "unix/utc@1".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    });
    alternatives.push(changed);
    for nested in [false, true] {
        for operator in ["==", "!="] {
            let p = program(operator, nested);
            for other in std::iter::once(&original).chain(&alternatives) {
                let input = c::StructuredInfoValue::record(
                    p.input_type.clone(),
                    vec![
                        c::StructuredFieldValue::new(
                            "left",
                            value(field(&p.input_type, "left"), &original, nested),
                        )
                        .unwrap(),
                        c::StructuredFieldValue::new(
                            "right",
                            value(field(&p.input_type, "right"), other, nested),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap()
                .canonical_bytes()
                .unwrap();
                let mut prepared =
                    conduit_plot::PreparedPortableExpressionEvaluator::new(&p).unwrap();
                let expected = p.evaluate(&input).unwrap();
                let capacity = prepared.output_capacity();
                let ((), storage) = allocation_probe::observe(|| {
                    assert_eq!(prepared.evaluate(&input).unwrap(), expected);
                });
                assert_eq!(storage.allocations, 0);
                assert_eq!(storage.reallocations, 0);
                assert_eq!(prepared.output_capacity(), capacity);
                let actual = c::InfoBool::decode(&p.evaluate(&input).unwrap())
                    .unwrap()
                    .get();
                assert_eq!(actual, (original == *other) == (operator == "=="));
            }
        }
    }
}

#[test]
fn malformed_reference_payloads_refuse_in_both_evaluators() {
    for nested in [false, true] {
        let p = program("==", nested);
        let valid = reference().encode().unwrap();
        for malformed in [
            vec![],
            vec![0; valid.len()],
            valid[..valid.len() - 1].to_vec(),
            {
                let mut trailing = valid.clone();
                trailing.push(0);
                trailing
            },
        ] {
            let valid_input = c::StructuredInfoValue::record(
                p.input_type.clone(),
                vec![
                    c::StructuredFieldValue::new(
                        "left",
                        value(field(&p.input_type, "left"), &reference(), nested),
                    )
                    .unwrap(),
                    c::StructuredFieldValue::new(
                        "right",
                        value(field(&p.input_type, "right"), &reference(), nested),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .canonical_bytes()
            .unwrap();
            let input = corrupt_reference(valid_input, &valid, &malformed);
            assert!(p.evaluate(&input).is_err());
            let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&p).unwrap();
            let (result, storage) =
                allocation_probe::observe(|| prepared.evaluate(&input).map(|_| ()));
            assert!(result.is_err());
            assert_eq!(storage.allocations, 0);
            assert_eq!(storage.reallocations, 0);
        }
    }
}

#[test]
fn distinct_expiry_fields_remain_part_of_exact_reference_identity() {
    let mut original = reference();
    original.lifetime.expires_at = Some(c::TemporalInstant {
        ticks: 2000,
        scale: c::TemporalScale::Milliseconds,
        clock_basis: "unix/utc@1".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    });
    let mut alternatives = Vec::new();
    for index in 0..5 {
        let mut changed = original.clone();
        let expiry = changed.lifetime.expires_at.as_mut().unwrap();
        match index {
            0 => expiry.ticks += 1,
            1 => expiry.scale = c::TemporalScale::Seconds,
            2 => expiry.clock_basis = "clock/other@1".into(),
            3 => expiry.resolution_ticks += 1,
            _ => expiry.uncertainty_ticks += 1,
        }
        alternatives.push(changed);
    }
    for nested in [false, true] {
        for operator in ["==", "!="] {
            let p = program(operator, nested);
            let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&p).unwrap();
            for other in std::iter::once(&original).chain(&alternatives) {
                let input = c::StructuredInfoValue::record(
                    p.input_type.clone(),
                    vec![
                        c::StructuredFieldValue::new(
                            "left",
                            value(field(&p.input_type, "left"), &original, nested),
                        )
                        .unwrap(),
                        c::StructuredFieldValue::new(
                            "right",
                            value(field(&p.input_type, "right"), other, nested),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap()
                .canonical_bytes()
                .unwrap();
                let expected = p.evaluate(&input).unwrap();
                assert_eq!(
                    c::InfoBool::decode(&expected).unwrap().get(),
                    (original == *other) == (operator == "==")
                );
                let ((), storage) = allocation_probe::observe(|| {
                    assert_eq!(prepared.evaluate(&input).unwrap(), expected)
                });
                assert_eq!(storage.allocations, 0);
                assert_eq!(storage.reallocations, 0);
            }
            assert!(
                conduit_plot::PreparedPortableExpressionEvaluator::new_scalar_tape(&p).is_err(),
                "reference equality must not silently enter the primitive scalar-tape profile"
            );
        }
    }
}

// Inject malformed wire content after constructing an otherwise valid frame.
fn corrupt_reference(mut input: Vec<u8>, valid: &[u8], malformed: &[u8]) -> Vec<u8> {
    let start = input
        .windows(valid.len())
        .position(|bytes| bytes == valid)
        .unwrap();
    assert_eq!(
        &input[start - 4..start],
        &(valid.len() as u32).to_le_bytes()
    );
    input[start - 4..start].copy_from_slice(&(malformed.len() as u32).to_le_bytes());
    input.splice(start..start + valid.len(), malformed.iter().copied());
    input
}
#[test]
fn short_circuit_cannot_hide_malformed_reference_members() {
    for shape in ["record", "variant", "sequence"] {
        let definition = match shape {
            "record" => "{\n    prefix: Boolean\n    reference: ResourceRef\n}",
            "variant" => "\n    present ResourceRef\n    | other ResourceRef",
            _ => "sequence ResourceRef in 0..=2",
        };
        let source = format!("type Wrapped = {definition}\ntype Pair = {{\n    left: Wrapped\n    right: Wrapped\n}}\nplot equal (\n    >> input: Pair\n    result: Boolean >>\n) = (.left == .right)");
        let mut startup = StartupCatalog::new();
        startup
            .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
            .unwrap();
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "equal", &ProfileCatalog::new())
                .unwrap()
                .expanded;
        let c::ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("program")
        };
        let p = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let outer = field(&p.input_type, "left");
        let wrapped = match outer.shape() {
            c::StructuredInfoTypeShape::Nominal { representation, .. } => representation,
            _ => outer,
        };
        let valid = reference().encode().unwrap();
        let leaf = |ty: &c::StructuredInfoType| {
            c::StructuredInfoValue::leaf(ty.clone(), valid.clone()).unwrap()
        };
        let (left, right) = match wrapped.shape() {
            c::StructuredInfoTypeShape::Record { .. } => {
                let make = |prefix| {
                    c::StructuredInfoValue::record(
                        wrapped.clone(),
                        vec![
                            c::StructuredFieldValue::new(
                                "prefix",
                                c::StructuredInfoValue::leaf(
                                    field(wrapped, "prefix").clone(),
                                    vec![prefix],
                                )
                                .unwrap(),
                            )
                            .unwrap(),
                            c::StructuredFieldValue::new(
                                "reference",
                                leaf(field(wrapped, "reference")),
                            )
                            .unwrap(),
                        ],
                    )
                    .unwrap()
                };
                (make(0), make(1))
            }
            c::StructuredInfoTypeShape::Variant { cases, .. } => {
                let payload = |tag| {
                    cases
                        .iter()
                        .find(|case| case.tag() == tag)
                        .unwrap()
                        .payload_type()
                };
                (
                    c::StructuredInfoValue::variant(
                        wrapped.clone(),
                        "present",
                        leaf(payload("present")),
                    )
                    .unwrap(),
                    c::StructuredInfoValue::variant(
                        wrapped.clone(),
                        "other",
                        leaf(payload("other")),
                    )
                    .unwrap(),
                )
            }
            c::StructuredInfoTypeShape::Sequence { element, .. } => (
                c::StructuredInfoValue::sequence(wrapped.clone(), vec![leaf(element)]).unwrap(),
                c::StructuredInfoValue::sequence(wrapped.clone(), vec![]).unwrap(),
            ),
            _ => panic!("fixture shape {shape}: {:?}", wrapped.shape()),
        };
        let (left, right) = if outer != wrapped {
            (
                c::StructuredInfoValue::nominal(outer.clone(), left).unwrap(),
                c::StructuredInfoValue::nominal(outer.clone(), right).unwrap(),
            )
        } else {
            (left, right)
        };
        let input = c::StructuredInfoValue::record(
            p.input_type.clone(),
            vec![
                c::StructuredFieldValue::new("left", left).unwrap(),
                c::StructuredFieldValue::new("right", right).unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert_eq!(p.evaluate(&input).unwrap(), [0]);
        let malformed = corrupt_reference(input, &valid, &vec![0; valid.len()]);
        assert!(
            p.evaluate(&malformed).is_err(),
            "{shape}: allocating equality must validate skipped members"
        );
        let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let (result, storage) =
            allocation_probe::observe(|| prepared.evaluate(&malformed).map(|_| ()));
        assert!(
            result.is_err(),
            "{shape}: prepared equality must validate skipped members"
        );
        assert_eq!(storage.allocations, 0);
        assert_eq!(storage.reallocations, 0);
    }
}
