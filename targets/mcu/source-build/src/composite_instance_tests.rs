//! One checked closure, two exact merge contracts, ordinary MCU definition emission.
use super::*;
use sha2::{Digest, Sha256};
const TYPES: &str =
    "type First = {\n value: U64\n}\ntype Second = {\n ready: Boolean\n count: U64\n}\n";
fn instance(name: &str, maximum_bytes: u32) -> c::KindId {
    instance_for(TYPES, name, maximum_bytes, "flow/merge/finite")
}
fn instance_for(source: &str, name: &str, maximum_bytes: u32, family: &str) -> c::KindId {
    let syntax = p::parse_syntax_document(source);
    let types = p::prepare_source_types(&syntax, &p::StartupCatalog::new()).unwrap();
    let schema = types.named_type(name).unwrap();
    let contract = c::CheckedValueContract::new(
        schema.profile().unwrap().value_kind().clone(),
        maximum_bytes,
        vec![],
    )
    .unwrap();
    c::typed_kind_instance_id(family, core::iter::once((schema, &contract))).unwrap()
}
fn source(first: &c::KindId, second: &c::KindId) -> String {
    format!("with {} as FirstMerge\nwith {} as SecondMerge\n\n{TYPES}\nplot both (\n >> first-left: First...|\n >> first-right: First...|\n >> second-left: Second...|\n >> second-right: Second...|\n first: First...| >>\n second: Second...| >>\n) {{\n a: FirstMerge\n b: SecondMerge\n first-left >> a.left\n first-right >> a.right\n a.merged >> first\n second-left >> b.left\n second-right >> b.right\n b.merged >> second\n}}\n", first.as_str(), second.as_str())
}
fn requests() -> [Specialization<'static>; 2] {
    [
        Specialization::MergeInstance(ValueReference {
            type_name: "First",
            maximum_bytes: 512,
        }),
        Specialization::MergeInstance(ValueReference {
            type_name: "Second",
            maximum_bytes: 640,
        }),
    ]
}
fn compile(
    source: &str,
    requests: &[Specialization<'_>],
) -> Result<crate::workset::Workset, String> {
    crate::workset::compile_specialized(
        source.as_bytes(),
        &format!("{:x}", Sha256::digest(source.as_bytes())),
        &["both".into()],
        None,
        requests,
    )
}
#[test]
fn two_exact_merges_share_a_source_without_overwriting_their_contracts() {
    let first = instance("First", 512);
    let second = instance("Second", 640);
    assert_ne!(first, second);
    let text = source(&first, &second);
    let workset = compile(&text, &requests()).unwrap();
    assert_eq!(
        workset.sha256,
        <[u8; 32]>::from(Sha256::digest(text.as_bytes()))
    );
    let material = crate::software::compile_with_types(
        &workset.entries[0].plot.expanded,
        Some(&workset.checked_source),
        32 * 1024,
    )
    .unwrap();
    assert_eq!(material.nodes.len(), 2);
    let ids: Vec<_> = material
        .nodes
        .iter()
        .map(|node| node.kind.clone())
        .collect();
    assert!(ids.contains(&first));
    assert!(ids.contains(&second));
    let mut bounds = Vec::new();
    for node in &material.nodes {
        let crate::software::SoftwareRole::Composite(role) = &node.role else {
            panic!("composite owner")
        };
        let Role::Merge(value) = role else {
            panic!("merge owner")
        };
        bounds.push(value.contract.maximum_bytes);
        let definition = emit_definition(role).unwrap();
        assert!(definition.contains("MergeStatic"));
        assert!(node.offer_image.is_some());
        assert_eq!(node.offer.as_ref().unwrap().kind_id, node.kind);
    }
    bounds.sort_unstable();
    assert_eq!(bounds, [512, 640]);
    assert_ne!(
        material.nodes[0].offer.as_ref().unwrap().capability_id,
        material.nodes[1].offer.as_ref().unwrap().capability_id
    );
    let mut forged = workset.entries[0].plot.expanded.gears[0].clone();
    forged.kind_id =
        "flow/merge/finite/typed-0000000000000000000000000000000000000000000000000000000000000000"
            .into();
    assert!(selected(&forged, Some(&workset.checked_source)).is_err());
}
#[test]
fn wrong_envelope_and_missing_instance_refuse_the_original_source_import() {
    let text = source(&instance("First", 512), &instance("Second", 640));
    assert!(compile(&text, &requests()[..1]).is_err());
    let changed = [
        Specialization::MergeInstance(ValueReference {
            type_name: "First",
            maximum_bytes: 513,
        }),
        Specialization::MergeInstance(ValueReference {
            type_name: "Second",
            maximum_bytes: 640,
        }),
    ];
    assert!(compile(&text, &changed).is_err());
    // The legacy family remains separately installable in the same catalogue.
    let mut all = Vec::from(requests());
    all.push(Specialization::Merge(ValueReference {
        type_name: "First",
        maximum_bytes: 512,
    }));
    assert!(compile(&text, &all).is_ok());
}

#[test]
fn distinct_terminal_state_instances_reuse_the_same_finite_runtime_family() {
    let types = "type FirstState = {\n terminal: Boolean\n}\ntype SecondState = {\n count: U64\n terminal: Boolean\n}\n";
    let first = instance_for(types, "FirstState", 512, "state/seeded/flow/until");
    let second = instance_for(types, "SecondState", 640, "state/seeded/flow/until");
    let source = format!("with {} as FirstCell\nwith {} as SecondCell\n{types}\nplot first-next (\n state: FirstState...| >> next: FirstState...|\n) = ({{terminal: true}})\nplot second-next (\n state: SecondState...| >> next: SecondState...|\n) = ({{count: .count, terminal: true}})\nplot both (\n >> left: FirstState...|\n >> right: SecondState...|\n first: FirstState...| >>\n second: SecondState...| >>\n) {{\n a: FirstCell\n b: SecondCell\n left >> a.seed\n right >> b.seed\n a.current >> first-next() >> a.next\n b.current >> second-next() >> b.next\n a.current >> first\n b.current >> second\n}}\n",first.as_str(),second.as_str());
    let requests = [
        Specialization::SeededUntilInstance(ValueReference {
            type_name: "FirstState",
            maximum_bytes: 512,
        }),
        Specialization::SeededUntilInstance(ValueReference {
            type_name: "SecondState",
            maximum_bytes: 640,
        }),
    ];
    let set = compile(&source, &requests).unwrap();
    let material = crate::software::compile_with_types(
        &set.entries[0].plot.expanded,
        Some(&set.checked_source),
        32 * 1024,
    )
    .unwrap();
    assert_eq!(material.nodes.len(), 2);
    for node in &material.nodes {
        let crate::software::SoftwareRole::Composite(Role::SeededUntil(_)) = &node.role else {
            panic!("finite state")
        };
        assert!(node.kind == first || node.kind == second);
    }
    assert_ne!(material.nodes[0].kind, material.nodes[1].kind);
}
