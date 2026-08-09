use patchbay_html::{
    demonstration_snapshot, RendererSnapshot, SnapshotError, MAX_SNAPSHOT_BYTES, SNAPSHOT_SCHEMA,
};

#[test]
fn typed_snapshot_round_trip_preserves_exact_provider_plan_play_and_evidence() {
    let snapshot = demonstration_snapshot().unwrap();
    let bytes = snapshot.encode().unwrap();
    let decoded = RendererSnapshot::decode(&bytes, snapshot.revision).unwrap();
    assert_eq!(decoded, snapshot);
    assert_eq!(decoded.schema, SNAPSHOT_SCHEMA);
    assert_eq!(
        decoded.routes[0].same_plan.candidates[0].provider,
        conduit_core::ConnectionProvider::UsbCdc
    );
    assert_eq!(
        decoded.routes[0].same_plan.candidates[1].provider,
        conduit_core::ConnectionProvider::WebSocket
    );
    assert_eq!(
        decoded.plan.as_ref().unwrap().plan_id,
        decoded.play.as_ref().unwrap().plan_id
    );
    assert!(!decoded.play.as_ref().unwrap().evidence.is_empty());
}

#[test]
fn stale_malformed_unknown_oversized_and_drifted_snapshots_fail_closed() {
    let snapshot = demonstration_snapshot().unwrap();
    let bytes = snapshot.encode().unwrap();
    assert_eq!(
        RendererSnapshot::decode(&bytes, snapshot.revision + 1),
        Err(SnapshotError::Stale {
            minimum: 2,
            offered: 1
        })
    );
    assert!(matches!(
        RendererSnapshot::decode(b"{", 0),
        Err(SnapshotError::Malformed(_))
    ));
    assert_eq!(
        RendererSnapshot::decode(&vec![b'x'; MAX_SNAPSHOT_BYTES + 1], 0),
        Err(SnapshotError::Oversized)
    );

    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["schema"] = "future".into();
    assert_eq!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::UnsupportedSchema)
    );
    value = serde_json::from_slice(&bytes).unwrap();
    value["plan"]["plan_id"] = "drifted".into();
    assert_eq!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::InvalidIdentity)
    );
    value = serde_json::from_slice(&bytes).unwrap();
    value["unexpected"] = true.into();
    assert!(matches!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::Malformed(_))
    ));

    value = serde_json::from_slice(&bytes).unwrap();
    value["routes"][0]["same_plan"]["candidates"][0]["provider"] = "DebugText".into();
    assert!(matches!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::Malformed(_))
    ));

    value = serde_json::from_slice(&bytes).unwrap();
    let item = value["document"]["forms"][0]["items"][0].clone();
    value["document"]["forms"][0]["items"] =
        serde_json::Value::Array(vec![item; patchbay_model::MAX_RENDERER_GRAPH_ITEMS + 1]);
    assert_eq!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::BoundExceeded("graph item"))
    );
}
