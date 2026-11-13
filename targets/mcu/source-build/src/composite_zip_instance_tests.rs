//! Checked MCU construction tests, not ConduitOS or physical execution proof.
use super::*;
use sha2::{Digest, Sha256};

const TYPES: &str =
    "type First = {\n value: U64\n}\ntype Second = {\n ready: Boolean\n count: U64\n}\n";
#[derive(Clone, Copy)]
struct Spec {
    left: &'static str,
    left_bytes: u32,
    right: &'static str,
    right_bytes: u32,
    feedback: bool,
}
fn value(name: &str, maximum: u32) -> Value {
    let syntax = p::parse_syntax_document(TYPES);
    let types = p::prepare_source_types(&syntax, &p::StartupCatalog::new()).unwrap();
    let schema = types.named_type(name).unwrap().clone();
    let contract = c::CheckedValueContract::new(
        schema.profile().unwrap().value_kind().clone(),
        maximum,
        vec![],
    )
    .unwrap();
    Value { schema, contract }
}
fn identity(spec: Spec) -> c::KindId {
    let left = value(spec.left, spec.left_bytes);
    let right = value(spec.right, spec.right_bytes);
    c::typed_kind_instance_id(
        if spec.feedback {
            "flow/zip/feedback"
        } else {
            "flow/zip/finite"
        },
        [
            (&left.schema, &left.contract),
            (&right.schema, &right.contract),
        ]
        .into_iter(),
    )
    .unwrap()
}
fn request(spec: Spec) -> Specialization<'static> {
    Specialization::ZipInstance {
        left: ValueReference {
            type_name: spec.left,
            maximum_bytes: spec.left_bytes,
        },
        right: ValueReference {
            type_name: spec.right,
            maximum_bytes: spec.right_bytes,
        },
        feedback: spec.feedback,
    }
}
fn source(specs: [Spec; 2]) -> String {
    let [first_id, second_id] = specs.map(identity);
    let (first, second) = (first_id.as_str(), second_id.as_str());
    format!(
        "with {first} as FirstZip\nwith {first}/paired as FirstPair\nwith {second} as SecondZip\nwith {second}/paired as SecondPair\n\n{TYPES}\nplot both (\n >> first-left: {}...|\n >> first-right: {}...|\n >> second-left: {}...|\n >> second-right: {}...|\n first: FirstPair...| >>\n second: SecondPair...| >>\n) {{\n a: FirstZip\n b: SecondZip\n first-left >> a.left\n first-right >> a.right\n a.paired >> first\n second-left >> b.left\n second-right >> b.right\n b.paired >> second\n}}\n",
        specs[0].left, specs[0].right, specs[1].left, specs[1].right,
    )
}
fn compile(text: &str, requests: &[Specialization<'_>]) -> Result<crate::workset::Workset, String> {
    crate::workset::compile_specialized(
        text.as_bytes(),
        &format!("{:x}", Sha256::digest(text.as_bytes())),
        &["both".into()],
        None,
        requests,
    )
}
fn finite() -> Spec {
    Spec {
        left: "First",
        left_bytes: 512,
        right: "Second",
        right_bytes: 640,
        feedback: false,
    }
}
fn check(specs: [Spec; 2]) {
    let text = source(specs);
    let workset = compile(&text, &specs.map(request)).unwrap();
    assert_eq!(
        workset.sha256,
        <[u8; 32]>::from(Sha256::digest(text.as_bytes()))
    );
    let entry = &workset.entries[0];
    let material = crate::software::compile_with_types(
        &entry.plot.expanded,
        Some(&workset.checked_source),
        32 * 1024,
    )
    .unwrap();
    assert_eq!(material.nodes.len(), 2);
    for spec in specs {
        let id = identity(spec);
        let node = material.nodes.iter().find(|n| n.kind == id).unwrap();
        let crate::software::SoftwareRole::Composite(role) = &node.role else {
            panic!("composite owner")
        };
        let Role::Zip {
            left,
            right,
            feedback,
        } = role
        else {
            panic!("zip owner")
        };
        assert_eq!(*feedback, spec.feedback);
        assert_eq!(left.contract.maximum_bytes, spec.left_bytes);
        assert_eq!(right.contract.maximum_bytes, spec.right_bytes);
        assert_eq!(left.schema, value(spec.left, spec.left_bytes).schema);
        assert_eq!(right.schema, value(spec.right, spec.right_bytes).schema);
        let pair = c::PreparedTypedTuplePairEncoder::new(
            left.schema.clone(),
            spec.left_bytes,
            right.schema.clone(),
            spec.right_bytes,
        )
        .unwrap();
        let pair_kind = pair.value_type().profile().unwrap().value_kind().clone();
        assert_eq!(
            workset.checked_source.structured_type(&pair_kind),
            Some(pair.value_type())
        );
        let offer = node.offer.as_ref().unwrap();
        assert_eq!(offer.kind_id, id);
        assert_eq!(offer.outputs.len(), 1);
        assert_eq!(offer.outputs[0].value_kind, pair_kind);
        let output = offer
            .semantic_contract
            .value_contracts()
            .iter()
            .find(|v| v.location == c::FrontValueLocation::Output(c::port_id("paired")))
            .unwrap();
        assert_eq!(output.contract.maximum_bytes, pair.maximum_bytes());
        assert_eq!(output.contract.value_kind, pair_kind);
        assert!(node.offer_image.is_some());
        let definition = emit_definition(role).unwrap();
        assert!(definition.contains("ZipStatic"));
        assert!(definition.contains(&format!("feedback:{}", spec.feedback)));
        let mut forged = entry
            .plot
            .expanded
            .gears
            .iter()
            .find(|g| g.kind_id == id)
            .unwrap()
            .clone();
        forged.kind_id = "flow/zip/finite/typed-0000000000000000000000000000000000000000000000000000000000000000".into();
        assert!(selected(&forged, Some(&workset.checked_source)).is_err());
    }
    assert_ne!(
        material.nodes[0].offer.as_ref().unwrap().capability_id,
        material.nodes[1].offer.as_ref().unwrap().capability_id
    );
}

#[test]
fn two_finite_instances_keep_ordered_pair_types_envelopes_and_exact_capabilities() {
    let a = finite();
    let b = Spec {
        left: a.right,
        left_bytes: a.right_bytes,
        right: a.left,
        right_bytes: a.left_bytes,
        ..a
    };
    assert_ne!(identity(a), identity(b));
    check([a, b]);
}

#[test]
fn finite_and_feedback_instances_coexist_without_conflating_mode() {
    let a = finite();
    let b = Spec {
        feedback: true,
        ..a
    };
    assert_ne!(identity(a), identity(b));
    check([a, b]);
}

#[test]
fn missing_instance_changed_envelope_or_mode_refuses_original_imports() {
    let a = finite();
    let b = Spec {
        feedback: true,
        ..a
    };
    let text = source([a, b]);
    assert!(compile(&text, &[request(a)]).is_err());
    assert!(compile(
        &text,
        &[
            request(Spec {
                left_bytes: 513,
                ..a
            }),
            request(b)
        ]
    )
    .is_err());
    assert!(compile(
        &text,
        &[
            request(a),
            request(Spec {
                feedback: false,
                ..b
            })
        ]
    )
    .is_err());
    let forged = text.replace(
        identity(a).as_str(),
        "flow/zip/finite/typed-0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(compile(&forged, &[request(a), request(b)]).is_err());
}
