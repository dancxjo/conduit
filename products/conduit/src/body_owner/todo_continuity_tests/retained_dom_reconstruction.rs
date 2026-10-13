//! Explicit software reconstruction of a retained owner's actual DOM transition.
//! No owner mutation, native gesture, scanout, delivered speech or human proof.
use conduit_presentation::{
    plan_face_utterances, render_linear_presentation, FaceUtteranceProvenance, GraphicsClipClass,
    Presentation, PresentationRole,
};
use serde_json::Value;

#[path = "recorded_owner_face.rs"]
mod recorded_owner_face;

#[path = "retained_look_reconstruction.rs"]
mod retained_look_reconstruction;

fn evidence(variable: &str) -> Value {
    let path = std::path::PathBuf::from(std::env::var_os(variable).expect(variable));
    assert!(
        path.is_absolute(),
        "explicit evidence path must be absolute"
    );
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn occurrence(value: &Value, body: &str) -> Presentation {
    let face = recorded_owner_face::decode(&value["owner"]).unwrap();
    assert_eq!(face.basis.body_id.as_ref().unwrap().as_str(), body);
    assert!(face.basis.active_play_id.is_none());
    assert_eq!(value["view"]["body_id"], body);
    assert_eq!(value["view"]["face_id"], face.identity.as_str());
    assert_eq!(value["view"]["face_revision"], face.revision.to_string());
    assert_eq!(
        value["owner"]["presentation_revision_decimal"],
        face.revision.to_string()
    );
    assert_eq!(value["view"]["show_state"], "available");
    assert_eq!(value["view"]["interactions_admitted"], true);
    assert!(!value["view"]["show_id"].as_str().unwrap().is_empty());
    for key in ["expression", "density"] {
        assert!(!value["dom"]["look"][key].as_str().unwrap().is_empty());
    }
    face
}

fn pending(face: &Presentation, id: &str) -> bool {
    !face
        .properties
        .iter()
        .find(|property| property.subject == id && property.name == "complete")
        .map(|property| {
            serde_json::to_value(&property.value).unwrap()["Flag"]
                .as_bool()
                .unwrap()
        })
        .unwrap()
}

fn reconstruct(face: &Presentation, observed: &Value, remaining: usize) {
    let items: Vec<_> = face
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Item)
        .collect();
    assert_eq!(items.len(), 3);
    assert_eq!(
        items
            .iter()
            .filter(|item| pending(face, &item.identity))
            .count(),
        remaining
    );
    let status = face
        .text
        .iter()
        .find(|text| text.subject == "todo/status")
        .unwrap();
    assert_eq!(
        status.text,
        format!("{remaining} things left · {} completed", 3 - remaining)
    );
    assert_eq!(observed["dom"]["count"], status.text);
    for (prefix, is_pending) in [("Things left in ", true), ("Completed items in ", false)] {
        let expected: Vec<_> = items
            .iter()
            .filter(|item| pending(face, &item.identity) == is_pending)
            .map(|item| item.name.as_str())
            .collect();
        let lists = observed["dom"]["lists"].as_array().unwrap();
        let actual: Vec<_> = lists
            .iter()
            .find(|list| list["label"].as_str().unwrap().starts_with(prefix))
            .map(|list| {
                list["names"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|name| name.as_str().unwrap())
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(actual, expected, "DOM semantic item order");
    }
    let linear = render_linear_presentation(face).unwrap();
    assert_eq!(linear.presentation_id, face.identity);
    assert_eq!(linear.revision, face.revision);
    let lines = linear.lines.join("\n");
    let aural = plan_face_utterances(face).unwrap();
    assert_eq!(aural.source_face_identity, face.identity.as_str());
    assert_eq!(aural.source_face_revision, face.revision);
    let spoken = aural
        .clauses
        .iter()
        .map(|clause| clause.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut native =
        conduitos::native_face_scene::NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    let mut documents = Vec::new();
    for details in [false, true] {
        native.show_details(details);
        let mut rendered = String::new();
        for _ in 0..native.pages() {
            for command in native.frame().unwrap().scene.commands() {
                assert_eq!(command.clip_class(), GraphicsClipClass::FullyVisible);
                rendered.push_str(command.payload());
                rendered.push('\n');
            }
            native.turn_page(true);
        }
        documents.push(rendered);
    }
    assert!(documents[0].contains(&status.text));
    assert!(lines.contains(&status.text));
    assert!(spoken.contains(&status.text));
    for item in &items {
        assert!(documents[1].contains(&item.name));
        if pending(face, &item.identity) {
            assert!(documents[0].contains(&item.name));
        }
        assert!(lines.contains(&item.name));
        assert!(spoken.contains(&item.name));
        assert!(aural
            .clauses
            .iter()
            .any(|clause| matches!(&clause.provenance,
            FaceUtteranceProvenance::Subject(source) if source.identity() == &item.identity)));
    }
    // Order is checked within the ordered subject clauses rather than prose substring offsets.
    let aural_items: Vec<_> = aural
        .clauses
        .iter()
        .filter_map(|clause| match &clause.provenance {
            FaceUtteranceProvenance::Subject(source)
                if items.iter().any(|item| item.identity == *source.identity()) =>
            {
                Some(source.identity())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        aural_items,
        items.iter().map(|item| &item.identity).collect::<Vec<_>>()
    );
    for action in &face.actions {
        assert!(lines.contains(&action.name));
        assert!(aural
            .clauses
            .iter()
            .any(|clause| matches!(&clause.provenance,
            FaceUtteranceProvenance::Action(source) if source.identity() == &action.identity)));
    }
    let pending_positions: Vec<_> = items
        .iter()
        .filter(|item| pending(face, &item.identity))
        .map(|item| documents[0].find(&item.name).unwrap())
        .collect();
    assert!(
        pending_positions.windows(2).all(|pair| pair[0] < pair[1]),
        "native primary must retain pending item order"
    );
    let linear_positions: Vec<_> = items
        .iter()
        .map(|item| lines.find(&item.name).unwrap())
        .collect();
    assert!(
        linear_positions.windows(2).all(|pair| pair[0] < pair[1]),
        "linear reader must retain item order"
    );
    assert_eq!(native.presentation(), face);
}

#[test]
#[ignore = "requires exact-source retained DOM exchange and retained verified owner receipt"]
fn actual_retained_dom_completion_reconstructs_native_linear_and_utterance_meaning() {
    let record = evidence("CONDUIT_RETAINED_DOM_EXCHANGE");
    let expected_source =
        std::env::var("CONDUIT_RETAINED_DOM_SOURCE_SHA").expect("explicit source SHA");
    assert_eq!(expected_source.len(), 40);
    assert!(expected_source.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(record["source_relation"], "exact-source");
    assert_eq!(record["handbook_ui_source_clean"], true);
    for key in [
        "owner_source_commit",
        "browser_runtime_source_commit",
        "handbook_ui_source_commit",
    ] {
        assert_eq!(record[key], expected_source);
    }
    retained_look_reconstruction::verify(&record);
    let exchange = &record["retained_dom_completion"];
    let body = record["body_id"].as_str().unwrap();
    let before = occurrence(&exchange["before"], body);
    let after = occurrence(&exchange["after"], body);
    assert_ne!(before.identity, after.identity);
    assert_ne!(before.revision, after.revision);
    assert_ne!(
        exchange["before"]["view"]["show_id"],
        exchange["after"]["view"]["show_id"]
    );
    assert_eq!(
        exchange["before"]["dom"]["look"]["expression"],
        exchange["after"]["dom"]["look"]["expression"]
    );
    assert_eq!(
        exchange["before"]["dom"]["look"]["density"],
        exchange["after"]["dom"]["look"]["density"]
    );
    let item_ids = |face: &Presentation| {
        face.subjects
            .iter()
            .filter(|subject| subject.role == PresentationRole::Item)
            .map(|subject| (subject.identity.clone(), subject.name.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(item_ids(&before), item_ids(&after));
    assert_eq!(
        before.relationships, after.relationships,
        "completion must preserve membership meaning"
    );
    for mutation in exchange["mutations"].as_array().unwrap() {
        let projected = occurrence(&mutation["after"], body);
        let documentary = &mutation["receipt"];
        assert_eq!(
            documentary["schema"],
            "conduit.proof/todo-observed-mutation@1"
        );
        assert_eq!(documentary["body_id"], body);
        assert_eq!(
            documentary["resulting_face_id"],
            projected.identity.as_str()
        );
        assert_eq!(
            documentary["resulting_face_revision"],
            projected.revision.to_string()
        );
        assert_eq!(
            documentary["initiating_show_id"],
            mutation["before"]["view"]["show_id"]
        );
        assert_eq!(
            documentary["initiating_face_id"],
            mutation["before"]["view"]["face_id"]
        );
        assert_eq!(
            documentary["initiating_face_revision"],
            mutation["before"]["view"]["face_revision"]
        );
        for key in ["write_terminal_sign", "read_terminal_sign"] {
            assert!(projected
                .basis
                .sign_ids
                .iter()
                .any(|id| id.as_str() == documentary[key]["sign_id"].as_str().unwrap()));
        }
        assert_eq!(
            mutation["before"]["dom"]["look"]["expression"],
            mutation["after"]["dom"]["look"]["expression"]
        );
        assert_eq!(
            mutation["before"]["dom"]["look"]["density"],
            mutation["after"]["dom"]["look"]["density"]
        );
    }
    let completion = exchange["mutations"].as_array().unwrap().last().unwrap();
    assert_eq!(completion["label"], "dom-complete");
    assert_eq!(completion["before"], exchange["before"]);
    assert_eq!(completion["after"], exchange["after"]);
    let action_id = completion["action"]["identity"].as_str().unwrap();
    let action = before
        .actions
        .iter()
        .find(|action| action.identity == action_id)
        .unwrap();
    assert!(action.availability.is_available());
    assert_eq!(completion["action"]["target"], action.target);
    assert_eq!(completion["action"]["intent"], "todo/complete@1");
    assert!(pending(&before, &action.target));
    assert!(!pending(&after, &action.target));
    let reopened_action = exchange["after"]["view"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|current| current["target"] == action.target && current["intent"] == "todo/reopen@1")
        .unwrap();
    assert_eq!(reopened_action["availability"], "available");
    assert!(after.actions.iter().any(|current| current.identity
        == reopened_action["identity"].as_str().unwrap()
        && current.target == action.target
        && current.availability.is_available()));
    for (id, _) in item_ids(&before)
        .iter()
        .filter(|(id, _)| id != &action.target)
    {
        assert_eq!(pending(&before, id), pending(&after, id));
    }
    let retained = evidence("CONDUIT_RETAINED_DOM_VERIFIED_RECEIPT");
    assert_eq!(retained["source_commit"], expected_source);
    assert_eq!(
        recorded_owner_face::decode(&exchange["after_stale_owner"]).unwrap(),
        after,
        "actual observation before browser cleanup must preserve exact post-stale Face"
    );
    assert_eq!(
        exchange["after_stale_owner"]["presentation_revision_decimal"],
        after.revision.to_string()
    );
    assert_eq!(
        retained["after_stale_owner_face"],
        exchange["after_stale_owner"]["presentation"]
    );
    // Browser departure can retire a route and reproject identity/revision. Keep that
    // later observation separate from the actual live post-stale observation.
    if let Some(closed) = retained.get("owner_after_browser_close_face") {
        let closed: Presentation = serde_json::from_value(closed.clone()).unwrap();
        closed.validate().unwrap();
        assert_eq!(closed.basis.body_id, after.basis.body_id);
        let mut expected = serde_json::to_value(&after).unwrap();
        let mut observed = serde_json::to_value(closed).unwrap();
        for key in ["identity", "revision"] {
            expected.as_object_mut().unwrap().remove(key);
            observed.as_object_mut().unwrap().remove(key);
        }
        assert_eq!(
            observed, expected,
            "browser cleanup may reproject identity but must preserve domain meaning"
        );
    }
    let receipt = &retained["last_execution"];
    assert_eq!(receipt["schema"], "conduit.todo/verified-read-receipt@1");
    assert_eq!(receipt["verified"], true);
    assert_eq!(receipt["body_id"], body);
    assert_eq!(receipt["read_terminal"], "Completed");
    let write = &receipt["write"];
    assert_eq!(write["body_id"], body);
    assert_eq!(write["terminal"], "Completed");
    assert_eq!(
        receipt["restored_fore_sha256"],
        write["committed_fore_sha256"]
    );
    assert_eq!(
        completion["receipt"]["committed_fore_sha256"],
        receipt["restored_fore_sha256"]
    );
    assert_eq!(write["initiating_action"]["action_id"], action_id);
    assert_eq!(
        write["initiating_action"]["face_id"],
        before.identity.as_str()
    );
    assert_eq!(
        write["initiating_action"]["show_id"],
        exchange["before"]["view"]["show_id"]
    );
    assert_eq!(
        completion["receipt"]["initiating_face_revision"],
        before.revision.to_string()
    );
    assert_eq!(
        completion["receipt"]["resulting_face_id"],
        after.identity.as_str()
    );
    assert_eq!(
        completion["receipt"]["resulting_face_revision"],
        after.revision.to_string()
    );
    for (sign, play) in [
        (&write["terminal_sign"], &write["play"]),
        (&receipt["read_terminal_sign"], &receipt["read_play"]),
    ] {
        assert_eq!(sign["active_play_id"], play["active_play_id"]);
        assert!(after
            .basis
            .sign_ids
            .iter()
            .any(|id| id.as_str() == sign["sign_id"].as_str().unwrap()));
    }
    assert_ne!(
        write["play"]["active_play_id"],
        receipt["read_play"]["active_play_id"]
    );
    assert_eq!(exchange["stale_return"]["refused"], true);
    assert_eq!(
        exchange["stale_return"]["message"],
        "owner Face interaction refused (-5)"
    );
    reconstruct(&before, &exchange["before"], 3);
    reconstruct(&after, &exchange["after"], 2);
}
