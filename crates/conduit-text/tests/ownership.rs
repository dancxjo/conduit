#[test]
fn portable_text_owner_has_no_upward_host_or_semantic_catalog_dependency() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("text owner declares dependencies")
        .1
        .split_once("[features]")
        .expect("text owner declares features")
        .0;
    for forbidden in ["conduit-semantic-catalog", "conduit-std-host", "hosts/"] {
        assert!(!dependencies.contains(forbidden));
    }
}

#[test]
fn exact_text_identities_faces_configuration_and_bounds_are_stable() {
    let literal = conduit_text::text_literal_semantics();
    let upper = conduit_text::text_upper_semantics();
    let join = conduit_text::text_join_semantics();

    assert_eq!(literal.kind_id.as_str(), "text/literal");
    assert_eq!(
        literal.kind_contract_revision.as_str(),
        "conduit.std/text-literal@1"
    );
    assert_eq!(upper.kind_id.as_str(), "text/upper");
    assert_eq!(
        upper.kind_contract_revision.as_str(),
        "conduit.std/text-upper@1"
    );
    assert_eq!(join.kind_id.as_str(), "text/join");
    assert_eq!(
        join.kind_contract_revision.as_str(),
        "conduit.std/text-join@1"
    );
    assert_eq!(literal.outputs[0].value_kind.as_str(), "value/text@1");
    assert_eq!(upper.inputs, join.inputs);
    assert_eq!(upper.outputs, join.outputs);
    assert_eq!(literal.configuration[0].key, "value");
    assert_eq!(join.configuration[0].key, "prefix");
    assert_eq!(literal.limits.max_active_instances, 16);
    assert_eq!(literal.limits.max_queue_items, 4);
    assert_eq!(literal.limits.max_queue_bytes, 256);
}
