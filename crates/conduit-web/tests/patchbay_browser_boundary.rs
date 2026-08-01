use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn browser_has_no_private_panel_parser_or_semantic_inference() {
    let root = root();
    assert!(
        !root.join("tour/public/patchbay-view-adapter.js").exists(),
        "the independent browser .panel parser must remain absent"
    );
    let renderer = std::fs::read_to_string(root.join("tour/public/patchbay-renderer.js")).unwrap();
    let faceplate =
        std::fs::read_to_string(root.join("tour/public/patchbay-faceplate.js")).unwrap();
    let highlighter =
        std::fs::read_to_string(root.join("tour/public/panel-highlighter.js")).unwrap();
    let tour = std::fs::read_to_string(root.join("tour/public/tour.js")).unwrap();
    for forbidden in [
        "parsePanelToViewModel",
        "new RegExp",
        "sourceText.split",
        ".replace(pattern",
    ] {
        assert!(
            !renderer.contains(forbidden),
            "renderer contains private semantic logic `{forbidden}`"
        );
    }
    for forbidden in [
        "184 msg/s",
        "kind.includes(\"literal\")",
        "kind.includes(\"http",
        "kind.includes(\"file-",
    ] {
        assert!(
            !faceplate.contains(forbidden),
            "faceplate contains invented fact or node-kind inference `{forbidden}`"
        );
    }
    assert!(renderer.contains("viewModel.topology?.expanded_nodes"));
    for forbidden in ["cordEndpoint", "selector === \".in\"", "(?:in|out)"] {
        assert!(
            !highlighter.contains(forbidden),
            "source highlighter contains name-based direction inference `{forbidden}`"
        );
    }
    assert!(highlighter.contains("panelSourceMetadata(textarea.value)"));
    assert!(renderer.contains("dataset.projection"));
    assert!(tour.contains("patchbay_open_session"));
    assert!(tour.contains("patchbay_apply_transaction"));
    assert!(!tour.contains("patchbay_replace_source"));
    assert!(!tour.contains("patchbay_move_node"));
}

#[test]
fn checked_wasm_bridge_exports_revisioned_session_operations() {
    let root = root();
    let declarations = std::fs::read_to_string(root.join("tour/public/conduit_web.d.ts")).unwrap();
    for operation in [
        "panel_source_metadata",
        "patchbay_open_session",
        "patchbay_session_view",
        "patchbay_apply_transaction",
        "patchbay_start_exact_run",
        "patchbay_pump_exact_run",
        "patchbay_read_exact_evidence",
        "patchbay_attach_exact_watch",
        "patchbay_detach_exact_watch",
        "patchbay_read_exact_watch",
        "patchbay_advance_exact_run",
        "patchbay_notify_host_operation",
        "patchbay_cancel_exact_run",
        "patchbay_snapshot_exact_run",
        "patchbay_dispose_exact_run",
    ] {
        assert!(
            declarations.contains(operation),
            "generated WASM bridge omits `{operation}`"
        );
    }
}
