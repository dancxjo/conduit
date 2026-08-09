use conduit_core::ConnectionBase;
use conduit_presentation::PresentationPropertyValue;
use patchbay_html::{
    demonstration_snapshot, RendererSnapshot, SnapshotError, MAX_SNAPSHOT_BYTES, SNAPSHOT_SCHEMA,
};

#[test]
fn portable_snapshot_round_trip_preserves_lifecycle_base_plan_play_and_clue() {
    let snapshot = demonstration_snapshot().unwrap();
    let bytes = snapshot.encode().unwrap();
    let decoded = RendererSnapshot::decode(&bytes, snapshot.revision).unwrap();
    assert_eq!(decoded, snapshot);
    assert_eq!(decoded.schema, SNAPSHOT_SCHEMA);
    assert!(decoded.presentation.properties.iter().any(|property| {
        property.value == PresentationPropertyValue::ConnectionBase(ConnectionBase::UsbCdc)
    }));
    assert!(decoded.presentation.properties.iter().any(|property| {
        property.value == PresentationPropertyValue::ConnectionBase(ConnectionBase::WebSocket)
    }));
    let basis = &decoded.presentation.basis;
    assert!(basis.plan_id.is_some() && basis.active_play_id.is_some());
    assert!(!basis.clue_ids.is_empty());
    assert!(!basis.body_id.as_str().is_empty());
    assert!(!basis.wake_id.as_str().is_empty());
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
    value["presentation"]["basis"]["plan_id"] = "drifted".into();
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
    let base = value["presentation"]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .position(|property| property["name"] == "base")
        .unwrap();
    value["presentation"]["properties"][base]["value"]["ConnectionBase"] = "DebugText".into();
    assert!(matches!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::Malformed(_))
    ));
}
