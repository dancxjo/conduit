//! Exact bounded Look stylesheet delivered by the ordinary HTTP server.
use super::request;

#[test]
fn html_theme_sheet_maps_the_shared_identity_and_every_bounded_token() {
    let response = request("/assets/theme.css", "GET");
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: text/css; charset=utf-8"));
    let body = response.split("\r\n\r\n").nth(1).unwrap();
    assert!(body.len() <= conduit_browser_patchbay_workbench::MAX_THEME_CSS_BYTES);
    assert!(body.contains("--conduit-look-identity:\"conduit.look/phosphor@1\""));
    for identity in [
        conduit_face::PHOSPHOR_LOOK_ID,
        conduit_face::EDITORIAL_LOOK_ID,
        conduit_face::ENGINEERING_LOOK_ID,
        conduit_face::FIELD_INSTRUMENT_DAY_LOOK_ID,
        conduit_face::FIELD_INSTRUMENT_NIGHT_LOOK_ID,
        conduit_face::navy_look_id(
            conduit_face::VisualDensity::Compact,
            conduit_face::VisualTypography::Editorial,
        ),
    ] {
        assert!(body.contains(&format!("[data-application-look=\"{identity}\"]")));
    }
    assert_eq!(body.matches("--conduit-look-identity:").count(), 6);
    for (identity, background, ink) in [
        (
            conduit_face::FIELD_INSTRUMENT_DAY_LOOK_ID,
            "#F1F3F2",
            "#222B2D",
        ),
        (
            conduit_face::FIELD_INSTRUMENT_NIGHT_LOOK_ID,
            "#161E21",
            "#EDF3F4",
        ),
    ] {
        let selected = body
            .split(&format!("[data-application-look=\"{identity}\"]{{"))
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert!(selected.contains(&format!("--conduit-background:{background}")));
        assert!(selected.contains(&format!("--conduit-text-primary:{ink}")));
        assert!(selected.contains("--conduit-type-body:18px"));
        assert!(selected.contains("--conduit-type-small:16px"));
        assert!(selected.contains("--conduit-radius-panel:0px"));
        assert!(selected.contains("--conduit-radius-control:2px"));
    }
    assert!(!body.contains("--patchbay-"));
    for (token, color) in [
        ("background", "#05070B"),
        ("surface", "#090D16"),
        ("reading-paper", "#0C121C"),
        ("workbench-canvas", "#05070B"),
        ("bootstrap-surface", "#090D16"),
        ("structure-primary", "#0DD8F6"),
        ("structure-secondary", "#0A1F87"),
        ("text-primary", "#93D2F7"),
        ("text-secondary", "#578EC9"),
        ("emphasis", "#E9A325"),
        ("focus", "#F4C400"),
    ] {
        assert!(body.contains(&format!("--conduit-{token}:{color}")));
    }
    assert!(body.contains("--conduit-font-body:var(--conduit-font-editorial)"));
    assert!(body.contains("--conduit-type-body:16px"));
    assert!(body.contains("--conduit-line-height:150%"));
    assert!(body.contains("--conduit-space-panel:13px"));
    assert!(body.contains("--conduit-radius-panel:9px"));
    assert!(body.contains("--conduit-responsive-breakpoint:720px"));

    let application = request("/assets/app.css", "GET");
    assert!(application.contains("var(--conduit-background)"));
    assert!(application.contains("var(--conduit-focus)"));
    assert!(!application.contains("#08111f"));

    let shared = request("/assets/conduit.css", "GET");
    assert!(shared.starts_with("HTTP/1.1 200 OK"));
    assert!(shared.contains("[data-application-component=\"navigation\"]"));

    let flow = request("/assets/flow.css", "GET");
    assert!(flow.contains(".flow-frontplate"));
    assert!(flow.contains(".react-flow__edge.animated"));
    assert!(flow.contains("@media (prefers-reduced-motion: reduce)"));
    assert!(!application.contains(".flow-frontplate header"));
}
