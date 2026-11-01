//! Refusals through the real delivery adapter and canonical pre-Birth editor.
use super::{PatchbayHtmlServer, ServerError};
use conduit_core::SignId;
use conduit_patchbay_workbench::PlotCandidate;
use serde_json::{json, Value};

fn server() -> PatchbayHtmlServer {
    let plot = PlotCandidate::from_source(
        "Admission", "work/admission.conduit",
        "plot admission {\n literal: text/literal(\"seed\")\n show: face/text(\"Message\")\n scalar: scalar/literal(value = 500000)\n literal >> show\n}\n",
        "authoring refusal proof", SignId::from("proof/authoring"), 1,
    ).unwrap();
    let mut server = PatchbayHtmlServer::bind_front_door_with_plots_ephemeral(vec![plot]).unwrap();
    let action =
        server
            .snapshot
            .face
            .actions
            .iter()
            .find(|action| {
                action.name == "Open"
                    && server.snapshot.face.subjects.iter().any(|subject| {
                        subject.identity == action.target && subject.name == "Admission"
                    })
            })
            .unwrap();
    let input = json!({"face_id":server.snapshot.face.identity,"face_revision":server.snapshot.face.revision,
        "kind":"invoke","action_id":action.identity});
    server
        .apply_interaction(&serde_json::to_vec(&input).unwrap())
        .unwrap();
    server
}
fn request(server: &PatchbayHtmlServer, operation: &str, primary: &str) -> Value {
    let authoring = server.snapshot.authoring.as_ref().unwrap();
    json!({"face_id":server.snapshot.face.identity,"face_revision":server.snapshot.face.revision,
        "kind":"edit","edit":{"source_document_id":authoring.source_document_id,
        "source_revision":authoring.source_revision,"expanded_plot_id":authoring.expanded_plot_id,
        "operation":operation,"primary":primary}})
}
fn source(server: &PatchbayHtmlServer) -> String {
    server
        .zero_body_front_door
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .opened_plot_document()
        .unwrap()
        .source
}
fn port(server: &PatchbayHtmlServer, kind: &str, direction: &str) -> String {
    use conduit_face::FacePropertyValue::{Identity, Text};
    let face = &server.snapshot.face;
    let gear = &face
        .properties
        .iter()
        .find(|property| property.name == "kind-id" && property.value == Identity(kind.into()))
        .unwrap()
        .subject;
    let endpoint = face
        .relationships
        .iter()
        .find(|relation| {
            relation.source == *gear
                && face.properties.iter().any(|property| {
                    property.subject == relation.target
                        && property.name == "direction"
                        && property.value == Text(direction.into())
                })
        })
        .unwrap();
    let Identity(identity) = &face
        .properties
        .iter()
        .find(|property| property.subject == endpoint.target && property.name == "semantic-id")
        .unwrap()
        .value
    else {
        panic!("exact Port identity");
    };
    identity.clone()
}

#[test]
fn stale_forged_unknown_and_incompatible_edits_leave_the_exact_source_unchanged() {
    let mut server = server();
    let original = source(&server);
    let basis = server.snapshot.authoring.clone();
    let face = server.snapshot.face.clone();
    let valid = request(&server, "place-gear", "text/literal");
    let mut stale_face = valid.clone();
    stale_face["face_revision"] = json!(0);
    let mut stale_source = valid.clone();
    stale_source["edit"]["source_revision"] = json!(999);
    let mut stale_document = valid.clone();
    stale_document["edit"]["source_document_id"] = json!("source/retired");
    let mut stale_graph = valid.clone();
    stale_graph["edit"]["expanded_plot_id"] = json!("expanded/retired");
    let mut incompatible = request(
        &server,
        "connect-ports",
        &port(&server, "scalar/literal", "outgoing"),
    );
    incompatible["edit"]["secondary"] = json!(port(&server, "face/text", "receiving"));
    let response = server
        .apply_interaction(&serde_json::to_vec(&incompatible).unwrap())
        .unwrap();
    let response: Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(
        response["interaction"]["last_disposition"],
        "Refused(IncompatiblePorts)"
    );
    assert_eq!(source(&server), original);
    let mut invalid_configuration = request(&server, "configure-gear", "gear/admission/literal");
    invalid_configuration["edit"]["key"] = json!("value");
    invalid_configuration["edit"]["value"] = json!({"U64": 42});
    for input in [
        stale_face,
        stale_source,
        stale_document,
        stale_graph,
        request(&server, "place-gear", "kind/unknown"),
        request(&server, "remove-gear", "gear/admission/retired"),
        incompatible,
        invalid_configuration,
    ] {
        let response = server
            .apply_interaction(&serde_json::to_vec(&input).unwrap())
            .unwrap();
        let response: Value = serde_json::from_slice(&response).unwrap();
        assert!(response["interaction"]["last_disposition"]
            .as_str()
            .unwrap()
            .starts_with("Refused"));
        assert_eq!(source(&server), original);
        assert_eq!(server.snapshot.authoring, basis);
        assert_eq!(server.snapshot.face, face);
    }
    let mut forged = valid.clone();
    forged["authority"] = json!("invented");
    let mut malformed = valid.clone();
    malformed["edit"]["primary"] = json!({"not":"an identity"});
    for input in [
        forged,
        malformed,
        request(&server, "unsupported-operation", "text/literal"),
    ] {
        assert!(matches!(
            server.apply_interaction(&serde_json::to_vec(&input).unwrap()),
            Err(ServerError::InvalidRequest)
        ));
        assert_eq!(source(&server), original);
        assert_eq!(server.snapshot.authoring, basis);
    }
    let response = server
        .apply_interaction(&serde_json::to_vec(&valid).unwrap())
        .unwrap();
    let response: Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(response["interaction"]["last_disposition"], "Succeeded");
    assert_ne!(source(&server), original);
    assert!(server.snapshot.face.basis.body_id.is_none());
}

#[test]
fn refresh_preserves_navigation_and_refuses_an_equal_revision_foreign_owner() {
    let mut left = server();
    let right = server();
    assert_eq!(left.snapshot.face.revision, right.snapshot.face.revision);
    assert_ne!(left.snapshot.face.identity, right.snapshot.face.identity);
    let before = left.snapshot.clone();
    left.refresh_front_door().unwrap();
    assert_eq!(left.snapshot.face, before.face);
    assert_eq!(left.snapshot.navigation, before.navigation);
    left.zero_body_front_door = right.zero_body_front_door.clone();
    assert!(
        matches!(left.refresh_front_door(), Err(ServerError::Interaction(reason)) if reason == "StaleRevision")
    );
    assert_eq!(left.snapshot.face, before.face);
    assert_eq!(left.snapshot.navigation, before.navigation);
}
