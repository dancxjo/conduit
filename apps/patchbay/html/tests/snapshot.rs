use conduit_core::BaseImplementationId;
use conduit_presentation::{
    ManifestationFailure, ManifestationLifecycle, PresentationPropertyValue,
};
use patchbay_html::{
    demonstration_snapshot, RendererSnapshot, SnapshotError, MAX_SNAPSHOT_BYTES, SNAPSHOT_SCHEMA,
};

#[test]
fn portable_snapshot_round_trip_preserves_lifecycle_base_plan_play_and_sign() {
    let snapshot = demonstration_snapshot().unwrap();
    let bytes = snapshot.encode().unwrap();
    let decoded = RendererSnapshot::decode(&bytes, snapshot.revision).unwrap();
    assert_eq!(decoded, snapshot);
    assert_eq!(decoded.schema, SNAPSHOT_SCHEMA);
    assert!(decoded.presentation.properties.iter().any(|property| {
        property.value
            == PresentationPropertyValue::BaseImplementationId(BaseImplementationId::from(
                "conduit.base/usb-cdc-acm@1",
            ))
    }));
    assert!(decoded.presentation.properties.iter().any(|property| {
        property.value
            == PresentationPropertyValue::BaseImplementationId(BaseImplementationId::from(
                "conduit.base/websocket-rfc6455@1",
            ))
    }));
    let basis = &decoded.presentation.basis;
    assert!(basis.plan_id.is_some() && basis.active_play_id.is_some());
    assert!(!basis.sign_ids.is_empty());
    assert!(!basis.body_id.as_ref().unwrap().as_str().is_empty());
    assert!(!basis.wake_id.as_ref().unwrap().as_str().is_empty());
    assert_eq!(
        decoded.renderer.manifestation.lifecycle,
        ManifestationLifecycle::Prepared
    );
    let workbench = decoded.workbench.as_ref().unwrap();
    assert_eq!(workbench.current.friendly_name, "Roseau");
    assert_eq!(workbench.current.program_label, "Hello");
    assert_eq!(workbench.current.admitted_parts, 3);
    assert_eq!(workbench.current.current_hosts.len(), 2);
    assert_eq!(workbench.history.entries.len(), 7);
    assert_eq!(
        workbench
            .history
            .entries
            .iter()
            .map(|entry| entry.evidence_sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6, 7]
    );
    assert!(decoded
        .renderer
        .validate_against(&decoded.presentation)
        .is_ok());
    assert_eq!(decoded.temporal_context.len(), 1);
    assert!(decoded.temporal_context[0]
        .relative_time
        .ends_with("seconds ago"));
    assert_eq!(
        decoded.temporal_context[0].source,
        decoded.presentation.temporal_facts[0].source
    );
}

#[test]
fn html_adapter_failure_is_typed_without_mutating_the_source_presentation() {
    let mut snapshot = demonstration_snapshot().unwrap();
    let source_identity = snapshot.presentation.identity.clone();
    let source_play = snapshot.presentation.basis.active_play_id.clone();
    snapshot
        .mark_failed(
            ManifestationFailure::DeliveryFailed,
            conduit_core::SignId::from("patchbay-html/delivery-failed"),
        )
        .unwrap();
    assert_eq!(
        snapshot.renderer.manifestation.lifecycle,
        ManifestationLifecycle::Failed
    );
    assert_eq!(
        snapshot.renderer.manifestation.failure,
        Some(ManifestationFailure::DeliveryFailed)
    );
    assert_eq!(snapshot.presentation.identity, source_identity);
    assert_eq!(snapshot.presentation.basis.active_play_id, source_play);
    assert_eq!(
        snapshot
            .renderer
            .manifestation
            .signs
            .last()
            .unwrap()
            .manifestation_id,
        snapshot.renderer.manifestation.manifestation_id
    );
    assert!(snapshot.encode().is_ok());
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
    value["presentation"]["properties"][base]["value"]["BaseImplementationId"] =
        serde_json::json!({ "not": "an identity string" });
    assert!(matches!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::Malformed(_))
    ));
    value = serde_json::from_slice(&bytes).unwrap();
    value["renderer"]["manifestation"]["presentation_revision"] = 99.into();
    assert_eq!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::InvalidIdentity)
    );
    value = serde_json::from_slice(&bytes).unwrap();
    value["temporal_context"][0]["relative_time"] = "invented age".into();
    assert_eq!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::InvalidIdentity)
    );
    value = serde_json::from_slice(&bytes).unwrap();
    value["workbench"]["history"]["entries"][1]["evidence_sequence"] = 1.into();
    assert_eq!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::InvalidIdentity)
    );
    value = serde_json::from_slice(&bytes).unwrap();
    value["workbench"]["current"]["body_id"] = "body/drifted".into();
    assert_eq!(
        RendererSnapshot::decode(&serde_json::to_vec(&value).unwrap(), 0),
        Err(SnapshotError::InvalidIdentity)
    );
}
