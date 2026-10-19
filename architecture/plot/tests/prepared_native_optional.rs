use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn document() -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document("type Interval = {\n start: U64\n end: U64\n where .start <= .end\n}\ntype Holder = {\n interval: Interval?\n}\n"),&StartupCatalog::new()).unwrap()
}
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 16384,
        scopes: 64,
        invariants: 64,
    }
}
fn native<'a>(doc: &'a CheckedSyntaxDocument, name: &str) -> &'a CheckedNativeType {
    doc.native_types.iter().find(|v| v.name == name).unwrap()
}
fn option(doc: &CheckedSyntaxDocument) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = native(doc, "Holder").value_type.shape()
    else {
        panic!()
    };
    fields[0].value_type().clone()
}
fn bytes(doc: &CheckedSyntaxDocument, range: Option<(u64, u64)>) -> Vec<u8> {
    let optional = option(doc);
    let value = if let Some((start, end)) = range {
        let scalar = |v: u64| {
            primitive_into_structured(StructuredInfoType::leaf(kind_id("value/u64")).unwrap(), &v)
                .unwrap()
        };
        let inner = StructuredInfoValue::record(
            native(doc, "Interval").value_type.clone(),
            vec![
                StructuredFieldValue::new("end", scalar(end)).unwrap(),
                StructuredFieldValue::new("start", scalar(start)).unwrap(),
            ],
        )
        .unwrap();
        StructuredInfoValue::variant(optional, "some", inner).unwrap()
    } else {
        StructuredInfoValue::variant(
            optional,
            "none",
            StructuredInfoValue::leaf(
                StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
                vec![],
            )
            .unwrap(),
        )
        .unwrap()
    };
    StructuredInfoValue::record(
        native(doc, "Holder").value_type.clone(),
        vec![StructuredFieldValue::new("interval", value).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn exact_core_optional_none_skips_and_some_executes_nested_native_law_without_growth() {
    let doc = document();
    for range in [None, Some((2, 7)), Some((7, 2))] {
        let input = bytes(&doc, range);
        let mut admission =
            PreparedNativeAdmission::new(&doc, &native(&doc, "Holder").identity, bounds()).unwrap();
        let (result, allocations) = allocation_probe::observe(|| {
            admission.begin(&input).unwrap();
            loop {
                match admission.step() {
                    Ok(PreparedNativeProgress::Pending) => (),
                    Ok(PreparedNativeProgress::Admitted) => break Ok(()),
                    Err(reason) => break Err(reason),
                }
            }
        });
        assert_eq!(allocations.allocations, 0);
        assert_eq!(allocations.reallocations, 0);
        if range == Some((7, 2)) {
            let PreparedNativeRefusal::Invariant { scope, .. } = result.unwrap_err() else {
                panic!()
            };
            assert_eq!(
                admission.scope_native_type(scope).unwrap().identity,
                native(&doc, "Interval").identity
            );
            assert!(admission.admitted().is_err());
        } else {
            result.unwrap();
            assert_eq!(admission.admitted().unwrap().canonical_bytes(), input);
        }
    }
}
#[test]
fn optional_label_wrong_empty_payload_and_unknown_schema_never_exempt_native_metadata() {
    let mut doc = document();
    let legitimate = option(&doc);
    let StructuredInfoTypeShape::Variant { cases, .. } = legitimate.shape() else {
        panic!()
    };
    let foreign = StructuredInfoType::variant(kind_id("forged/optional"), cases.to_vec()).unwrap();
    let wrong_none = StructuredInfoType::variant(
        kind_id("conduit.conduitese.optional.v1"),
        vec![
            StructuredVariantCase::new(
                "none",
                StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
            )
            .unwrap(),
            cases.iter().find(|v| v.tag() == "some").unwrap().clone(),
        ],
    )
    .unwrap();
    let unknown = StructuredInfoType::record(
        kind_id("unknown/native-schema"),
        vec![StructuredFieldType::new(
            "value",
            StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    for (index, unsupported) in [foreign, wrong_none, unknown].into_iter().enumerate() {
        // An explicitly adversarial preparation profile: the root metadata is
        // complete; only an unsupported child schema is unowned. Do not let a
        // duplicate root metadata conflict mask the child recognition check.
        let identity = kind_id(&format!("profile/holder/{index}"));
        doc.native_types.push(CheckedNativeType {
            name: format!("profile-holder-{index}"),
            identity: identity.clone(),
            value_type: StructuredInfoType::record(
                identity.clone(),
                vec![StructuredFieldType::new("interval", unsupported).unwrap()],
            )
            .unwrap(),
            value_contracts: vec![],
            invariants: vec![],
        });
        assert!(matches!(
            PreparedNativeAdmission::new(&doc, &identity, bounds()),
            Err(PreparedNativeRefusal::MissingNativeMetadata)
        ));
    }
}
