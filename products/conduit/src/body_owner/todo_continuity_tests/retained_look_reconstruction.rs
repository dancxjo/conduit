//! Evidence checks for three actual browser Looks of one retained canonical Face.
//! Route custody and recorded DOM observations, not native/audio/human enactment.
use serde_json::Value;

fn text(value: &Value) -> &str {
    let value = value.as_str().expect("required recorded identity/wording");
    assert!(!value.is_empty());
    value
}

pub(super) fn verify(record: &Value) {
    let comparison = &record["retained_look_selection"];
    let source = &comparison["source"];
    let canonical = super::recorded_owner_face::decode(source).unwrap();
    let body = text(&record["body_id"]);
    assert_eq!(canonical.basis.body_id.as_ref().unwrap().as_str(), body);
    let observations = comparison["observations"].as_array().unwrap();
    assert_eq!(observations.len(), 3);
    let baseline = &observations[0];
    let mut shows = std::collections::BTreeSet::new();
    for (index, look) in ["generic", "compact", "spacious"].into_iter().enumerate() {
        let observation = &observations[index];
        assert_eq!(observation["look"], look);
        let view = &observation["view"];
        assert_eq!(view["schema"], "conduit.browser/owner-face-mask@1");
        assert_eq!(view["body_id"], body);
        assert_eq!(view["face_id"], canonical.identity.as_str());
        assert_eq!(view["face_revision"], canonical.revision.to_string());
        assert_eq!(
            source["presentation_revision_decimal"],
            canonical.revision.to_string()
        );
        assert_eq!(view["show_state"], "available");
        assert_eq!(view["interactions_admitted"], true);
        assert!(shows.insert(text(&view["show_id"])));
        for key in [
            "mask_plot_id",
            "mask_expanded_plot_id",
            "mask_plan_id",
            "mask_play_id",
        ] {
            text(&view[key]);
        }
        // Route Plan is carrier authority, distinct from the Mask realization Plan.
        text(&view["route"]["plan_id"]);
        assert_eq!(view["route"]["mask_host_id"], record["browser_host_id"]);
        assert_eq!(view["route"]["mask_boot_id"], record["browser_boot_id"]);
        assert_eq!(
            view["route"]["owner_host_id"],
            source["advertisement"]["host_id"]
        );
        assert_eq!(
            view["route"]["owner_boot_id"],
            source["advertisement"]["boot_id"]
        );
        let subjects = view["subjects"].as_array().unwrap();
        assert_eq!(subjects.len(), canonical.subjects.len());
        for (actual, expected) in subjects.iter().zip(&canonical.subjects) {
            assert_eq!(actual["identity"], expected.identity);
            assert_eq!(actual["name"], expected.name);
            assert_eq!(actual["role"], format!("{:?}", expected.role));
            let wording: Vec<_> = canonical
                .text
                .iter()
                .filter(|word| word.subject == expected.identity)
                .map(|word| word.text.as_str())
                .collect();
            assert_eq!(actual["text"], serde_json::to_value(wording).unwrap());
            let disclosure = canonical
                .disclosures
                .iter()
                .find(|entry| entry.subject == expected.identity)
                .map(|entry| format!("{:?}", entry.level))
                .unwrap_or_else(|| "ExactProvenance".into());
            assert_eq!(actual["disclosure"], disclosure);
        }
        let actions = view["actions"].as_array().unwrap();
        assert_eq!(actions.len(), canonical.actions.len());
        for (actual, expected) in actions.iter().zip(&canonical.actions) {
            assert_eq!(actual["identity"], expected.identity);
            assert_eq!(actual["target"], expected.target);
            assert_eq!(actual["intent"], expected.intent);
            assert_eq!(actual["name"], expected.name);
            assert_eq!(actual["disclosure"], format!("{:?}", expected.disclosure));
            let availability = serde_json::to_value(&expected.availability).unwrap();
            let kind = if availability == "Available" {
                "available"
            } else if availability.get("Refused").is_some() {
                "refused"
            } else {
                "unavailable"
            };
            assert_eq!(actual["availability"], kind);
            let arguments = actual["arguments"].as_array().unwrap();
            assert_eq!(arguments.len(), expected.arguments.len());
            for (actual_argument, source_argument) in arguments.iter().zip(&expected.arguments) {
                assert_eq!(actual_argument["name"], source_argument.name);
                assert_eq!(actual_argument["value_name"], source_argument.value_name);
                assert_eq!(
                    actual_argument["value_kind"],
                    source_argument.contract.value_kind.as_str()
                );
                assert_eq!(
                    actual_argument["maximum_bytes"],
                    source_argument.contract.maximum_bytes
                );
                if expected.identity == "todo.add" {
                    assert!(actual_argument["choices"].as_array().unwrap().is_empty());
                }
            }
            if kind != "available" {
                let reason = availability
                    .get(if kind == "refused" {
                        "Refused"
                    } else {
                        "Unavailable"
                    })
                    .unwrap();
                assert_eq!(actual["reason_code"], reason["reason_code"]);
                assert_eq!(actual["explanation"], reason["explanation"]);
            }
        }
        let relationships = view["relationships"].as_array().unwrap();
        assert_eq!(relationships.len(), canonical.relationships.len());
        for (actual, expected) in relationships.iter().zip(&canonical.relationships) {
            assert_eq!(actual["source"], expected.source);
            assert_eq!(actual["target"], expected.target);
            assert_eq!(actual["kind"], format!("{:?}", expected.kind));
        }
        // Full projections compare disclosure, bounded arguments, flags and wording as well.
        for key in ["subjects", "actions", "relationships"] {
            assert_eq!(view[key], baseline["view"][key]);
        }
        let dom = &observation["dom"];
        assert_eq!(dom["lists"], baseline["dom"]["lists"]);
        let items: Vec<_> = canonical
            .subjects
            .iter()
            .filter(|subject| subject.role == conduit_presentation::PresentationRole::Item)
            .collect();
        let lists = dom["lists"].as_array().unwrap();
        for (prefix, pending) in [("Things left in ", true), ("Completed items in ", false)] {
            let expected_names: Vec<_> = items
                .iter()
                .filter(|item| super::pending(&canonical, &item.identity) == pending)
                .map(|item| item.name.as_str())
                .collect();
            let matching: Vec<_> = lists
                .iter()
                .filter(|list| text(&list["label"]).starts_with(prefix))
                .collect();
            assert!(matching.len() <= 1);
            if !expected_names.is_empty() {
                assert_eq!(matching.len(), 1);
            }
            let observed_names: Vec<_> = matching
                .first()
                .map(|list| list["names"].as_array().unwrap().iter().map(text).collect())
                .unwrap_or_default();
            assert_eq!(
                observed_names, expected_names,
                "source-derived DOM grouping and item order"
            );
        }
        // An empty semantic group may retain or omit its empty DOM list.
        // Additional groups would invent membership beyond this projection.
        assert!(lists.iter().all(|list| {
            let label = text(&list["label"]);
            label.starts_with("Things left in ") || label.starts_with("Completed items in ")
        }));
        assert_eq!(dom["status"], baseline["dom"]["status"]);
        let status = canonical
            .text
            .iter()
            .find(|text| text.subject == "todo/status")
            .unwrap();
        assert_eq!(dom["status"], status.text);
        let rendered = text(&dom["text"]);
        for subject in &canonical.subjects {
            if format!("{:?}", subject.role) != "Status" {
                assert!(rendered.contains(&subject.name));
            }
        }
        let controls = dom["forms"].as_array().unwrap();
        for control in controls {
            let action = actions
                .iter()
                .find(|action| action["identity"] == control["action_id"])
                .unwrap();
            let target_name = subjects
                .iter()
                .find(|subject| subject["identity"] == action["target"])
                .map(|subject| text(&subject["name"]))
                .unwrap();
            assert!(
                text(&control["name"]) == text(&action["name"])
                    || text(&control["name"]) == format!("{} {target_name}", text(&action["name"]))
            );
            assert_eq!(control["disabled"], action["availability"] != "available");
            assert_eq!(control["visible"], true);
            if action["availability"] != "available" {
                assert_eq!(control["disabled"], true);
            }
        }
        for action in actions
            .iter()
            .filter(|action| action["availability"] == "available")
        {
            assert!(controls
                .iter()
                .any(|control| control["action_id"] == action["identity"]));
        }
        let focus = &dom["keyboard_focus"];
        let focused_action = actions
            .iter()
            .find(|action| action["identity"] == focus["action_id"])
            .unwrap();
        assert_eq!(focused_action["availability"], "available");
        assert_eq!(focus["accessible_name"], focused_action["name"]);
        assert_eq!(focused_action["identity"], "todo.add");
        let input_argument = &focused_action["arguments"][0];
        assert_eq!(focus["input"]["type"], "text");
        assert_eq!(
            focus["input"]["maximum_length"],
            input_argument["maximum_bytes"]
        );
        assert_eq!(
            focus["input"]["labels"],
            serde_json::json!([input_argument["value_name"]])
        );
        for key in ["focused", "enabled", "visible"] {
            assert_eq!(focus[key], true);
        }
        if index == 0 {
            assert!(view.get("component").is_none());
            for key in ["expression", "density", "columns"] {
                assert!(dom["component_dataset"][key].is_null());
            }
            assert!(observation["recipe"].is_null());
            assert!(observation["retirement"].is_null());
        } else {
            assert_eq!(observation["recipe"], "flat");
            text(&view["component"]["expression"]["expression"]["identity"]);
            assert_eq!(
                view["component"]["expression"]["expression"]["density"],
                look
            );
            assert_eq!(
                dom["component_dataset"]["expression"],
                view["component"]["expression"]["expression"]["identity"]
            );
            assert_eq!(dom["component_dataset"]["density"], look);
            assert_eq!(
                dom["component_dataset"]["columns"],
                view["component"]["maximum_columns"]
                    .as_u64()
                    .unwrap()
                    .to_string()
            );
            let retirement = &observation["retirement"];
            assert_eq!(
                retirement["retired_show_id"],
                observations[index - 1]["view"]["show_id"]
            );
            assert_eq!(retirement["current_show_id"], view["show_id"]);
            assert_eq!(retirement["refusal"]["refused"], true);
            assert_eq!(
                retirement["after_refusal_owner_face"],
                source["presentation"]
            );
            assert_eq!(
                retirement["refusal"]["message"],
                "owner Face interaction refused (-5)"
            );
        }
    }
    let compact = observations[1]["dom"]["geometry"]["collection_padding_px"]
        .as_f64()
        .unwrap();
    let spacious = observations[2]["dom"]["geometry"]["collection_padding_px"]
        .as_f64()
        .unwrap();
    assert!(compact.is_finite() && compact > 0.0 && spacious.is_finite() && spacious > compact);
    // No owner mutation lies between the comparisons and the retained helper's initial Face.
    assert_eq!(
        source["presentation"],
        record["retained_dom_completion"]["initial"]["presentation"]
    );
}
