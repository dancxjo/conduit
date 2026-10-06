use crate::*;
use alloc::{string::String, vec};

fn document() -> PatchbayWorkspace {
    serde_json::from_str(include_str!(
        "../../../../proof/browser/fixtures/patchbay-workspace.json"
    ))
    .unwrap()
}

#[test]
fn workspace_shared_browser_native_document_round_trips_without_runtime_truth() {
    let document = document();
    document.validate().unwrap();
    let encoded = serde_json::to_string(&document).unwrap();
    assert!(encoded.len() < MAX_WORKSPACE_BYTES);
    assert_eq!(document, serde_json::from_str(&encoded).unwrap());
    assert_eq!(document.layouts.len(), 2);
    assert_eq!(
        document.layout_for_basis(&document.basis).unwrap().name,
        "Teaching"
    );
    assert!(!encoded.contains("plan_id"));
    let mut changed = document.clone();
    changed.active_layout = "Vertical".into();
    assert_eq!(changed.basis, document.basis);
    assert_ne!(
        changed.layout_for_basis(&changed.basis).unwrap().positions,
        document.layouts[0].positions
    );
}

#[test]
fn workspace_orphans_and_changed_basis_are_explicit_and_retained() {
    let document = document();
    let subjects = vec![String::from("gear/source")];
    let correlation = document.correlate(&document.basis, &subjects).unwrap();
    assert!(correlation.basis_matches);
    assert!(correlation
        .orphaned_subjects
        .contains(&String::from("gear/sink")));
    let mut basis = document.basis.clone();
    basis.checked_plot_id = "checked/renamed".into();
    assert!(!document.correlate(&basis, &subjects).unwrap().basis_matches);
    assert_eq!(
        document.layout_for_basis(&basis),
        Err(WorkspaceError::ChangedBasis)
    );
    assert_eq!(document, self::document());
}

#[test]
fn workspace_refuses_future_unknown_duplicate_and_unbounded_state() {
    let mut value = serde_json::to_value(document()).unwrap();
    value["plan_id"] = "invented/live-truth".into();
    assert!(serde_json::from_value::<PatchbayWorkspace>(value).is_err());
    let mut document = document();
    document.schema = "conduit.patchbay.workspace/v2".into();
    assert_eq!(document.validate(), Err(WorkspaceError::UnsupportedSchema));
    document.schema = PATCHBAY_WORKSPACE_SCHEMA.into();
    document.layouts[0].positions[0].x = 32768;
    assert_eq!(document.validate(), Err(WorkspaceError::InvalidGeometry));
    document.layouts[0].positions[0].x = 0;
    let duplicate = document.layouts[0].positions[0].clone();
    document.layouts[0].positions.push(duplicate);
    assert_eq!(document.validate(), Err(WorkspaceError::DuplicateIdentity));
}

#[test]
fn workspace_routes_have_only_bounded_geometry_and_strict_nested_fields() {
    let mut document = document();
    document.layouts[0].routes[0]
        .points
        .push(WorkspacePoint { x: 1, y: 2 });
    assert_eq!(document.validate(), Err(WorkspaceError::InvalidRoute));
    let mut value = serde_json::to_value(self::document()).unwrap();
    value["layouts"][0]["routes"][0]["host"] = "not-editor-state".into();
    assert!(serde_json::from_value::<PatchbayWorkspace>(value).is_err());
    document = self::document();
    document.layouts[0].notes[0].text = "é".repeat(1025);
    assert_eq!(document.validate(), Err(WorkspaceError::BoundExceeded));
}

#[test]
fn workspace_migration_maps_only_exact_legacy_subjects_and_drops_selection() {
    let legacy: LegacyFlowPresentation = serde_json::from_str(
        r#"{
        "schema":"conduit.patchbay.flow-presentation/v1",
        "workspaceIdentity":"exact/workspace",
        "nodes":[{"id":"renderer/wrapper","position":{"x":-1.9,"y":2.8},"selected":true}],
        "viewport":{"x":0,"y":0,"zoom":1}
    }"#,
    )
    .unwrap();
    let basis = document().basis;
    let mapping = vec![("renderer/wrapper".into(), "gear/source".into())];
    let migrated = legacy
        .migrate("exact/workspace", basis.clone(), &mapping)
        .unwrap();
    assert_eq!(
        migrated.layouts[0].positions[0],
        WorkspacePosition {
            subject: "gear/source".into(),
            x: -1,
            y: 2
        }
    );
    assert_eq!(
        migrated,
        legacy
            .migrate("exact/workspace", basis.clone(), &mapping)
            .unwrap()
    );
    assert_eq!(
        legacy.migrate("changed/workspace", basis.clone(), &mapping),
        Err(WorkspaceError::ChangedBasis)
    );
    assert_eq!(
        legacy.migrate("exact/workspace", basis, &[]),
        Err(WorkspaceError::UnmappedLegacySubject)
    );
}
