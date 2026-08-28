#[test]
fn portable_owner_has_no_upward_host_or_semantic_catalog_dependency() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("portable owner declares dependencies")
        .1
        .split_once("[features]")
        .expect("portable owner declares features")
        .0;
    for forbidden in ["conduit-semantic-catalog", "conduit-std-host", "targets/"] {
        assert!(
            !dependencies.contains(forbidden),
            "portable HTTP/JSON meaning must not depend upward on {forbidden}"
        );
    }
}

#[test]
fn exact_http_and_json_identities_and_bounds_are_stable() {
    let client = conduit_web::http_client_semantics();
    let server = conduit_web::http_server_semantics();
    let encode = conduit_web::json_encode_semantics();
    let decode = conduit_web::json_decode_semantics();

    assert_eq!(client.kind_id.as_str(), "http/client");
    assert_eq!(
        client.kind_contract_revision.as_str(),
        "conduit.http/client@1"
    );
    assert_eq!(server.kind_id.as_str(), "http/server");
    assert_eq!(
        server.kind_contract_revision.as_str(),
        "conduit.http/server@1"
    );
    assert_eq!(
        client.inputs[0].value_kind,
        conduit_web::http_request_type()
            .profile()
            .expect("finite HTTP request profile")
            .value_kind()
            .clone()
    );
    assert_eq!(
        client.outputs[0].value_kind,
        conduit_web::http_response_type()
            .profile()
            .expect("finite HTTP response profile")
            .value_kind()
            .clone()
    );
    assert_eq!(client.limits.max_active_instances, 4);
    assert_eq!(client.limits.max_queue_items, 4);
    assert_eq!(client.limits.max_queue_bytes, 262_144);

    assert_eq!(encode.kind_id.as_str(), "json/encode");
    assert_eq!(
        encode.kind_contract_revision.as_str(),
        "conduit.std/json-encode@1"
    );
    assert_eq!(decode.kind_id.as_str(), "json/decode");
    assert_eq!(
        decode.kind_contract_revision.as_str(),
        "conduit.std/json-decode@1"
    );
    assert_eq!(
        encode.inputs[0].value_kind.as_str(),
        conduit_core::JSON_INFO_ID
    );
    assert_eq!(
        decode.outputs[0].value_kind.as_str(),
        conduit_core::JSON_INFO_ID
    );
    assert_eq!(
        encode.limits.max_queue_bytes,
        conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32
    );
}
