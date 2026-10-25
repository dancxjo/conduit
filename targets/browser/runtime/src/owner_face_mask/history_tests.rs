//! Checked browser Mask/renderer preparation over an encoded synthetic recording.
//! Acknowledgement inputs below exercise the real Mask API and kernel; these
//! CPU tests alone do not establish visible DOM effects, live joining or storage.
use super::*;
use conduit_face::{
    FaceBasis, FacePropertyValue as V, HISTORY_COMPONENT_MASK_ROOT, HISTORY_COMPONENT_MASK_SOURCE,
    HISTORY_COMPONENT_RECIPE_ID,
};

#[path = "../../../../../bodies/environment/history/tests/common/mod.rs"]
mod common;
const REVISION: u64 = 9_007_199_254_740_993;
fn history_face() -> Face {
    let mut history = common::prepared(1);
    history
        .read_next(&common::chunk(&[
            (1, 1000, true),
            (2, 2000, false),
            (4, 11000, true),
        ]))
        .unwrap();
    let fragment = history.contribution(common::basis()).unwrap();
    face_from_fragment(fragment)
}
fn face_from_fragment(fragment: conduit_face::FaceFragment) -> Face {
    let body = conduit_body::Body::born(
        "source/browser-history-cpu-fixture".into(),
        common::basis().checked_plot_id,
        1,
        "sign/browser-history-cpu-fixture".into(),
    )
    .unwrap();
    Face::new_with_semantics(
        REVISION,
        FaceBasis {
            body_id: Some(body.body_id),
            wake_id: None,
            source_document_id: Some("source/browser-history-cpu-fixture".into()),
            checked_plot_id: Some(common::basis().checked_plot_id),
            expanded_plot_id: Some("expanded/browser-history-cpu-fixture".into()),
            plan_id: Some(common::basis().plan_id),
            active_play_id: Some(common::basis().active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}
fn prepare(face: Face) -> OwnerBrowserMask {
    prepare_with_look(face, None)
}
fn prepare_with_look(
    face: Face,
    selected_look: Option<&conduit_face::LookInfo>,
) -> OwnerBrowserMask {
    let host_id = HostId::from("host/browser-history-cpu-fixture");
    let boot_id = BootId::from("boot/browser-history-cpu-fixture");
    // Existing installed browser Back definitions with explicit CPU Host IDs.
    // No invented capability/resource offer or transport admission is supplied.
    let host = crate::installed_browser::membership_advertisement(host_id.clone(), boot_id.clone());
    let owner = crate::installed_browser::membership_advertisement(
        "owner/browser-history-cpu-fixture".into(),
        "boot/owner-history-cpu-fixture".into(),
    );
    // Explicit finite carrier fixtures, using the existing offered-Line
    // constructor. These exercise admission; no socket/join is claimed.
    let line = |name: &str,
                source: &conduit_core::HostAdvertisement,
                sink: &conduit_core::HostAdvertisement| {
        let mut line = conduit_core::process_owned_line_offer_with_limits(
            name,
            &format!("binding/{name}"),
            "conduit.base/websocket-rfc6455@1".into(),
            "websocket/history-cpu-fixture",
            source,
            sink,
            conduit_core::LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64 * 1024,
                maximum_frame_bytes: 64 * 1024,
                maximum_buffered_bytes: 256 * 1024,
            },
        );
        line.contract = conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0].contract;
        line
    };
    let sent = line("face", &owner, &host);
    let returned = line("show", &host, &owner);
    let interaction = line("interaction", &host, &owner);
    let planned = if let Some(look) = selected_look {
        conduit_browser_mask_offer::planned_owner_face_show_interaction_mask_with_look(
            &host,
            &owner,
            &sent,
            &returned,
            &interaction,
            HISTORY_COMPONENT_MASK_SOURCE,
            HISTORY_COMPONENT_MASK_ROOT,
            look,
        )
    } else {
        conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
            &host,
            &owner,
            &sent,
            &returned,
            &interaction,
            HISTORY_COMPONENT_MASK_SOURCE,
            HISTORY_COMPONENT_MASK_ROOT,
        )
    }
    .unwrap();
    OwnerBrowserMask::prepare_planned_with_look(
        HostBasis {
            body_id: face.basis.body_id.clone().unwrap(),
            host_id,
            boot_id,
            visual_constraints: Some(component::VisualConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 100,
            }),
        },
        face,
        planned,
        None,
        Some((
            sent.admitted_line(),
            returned.admitted_line(),
            Some(interaction.admitted_line()),
        )),
        1,
        true,
        selected_look,
    )
    .unwrap()
}
fn export<T: serde::Serialize>(name: &str, value: &T) {
    let Some(directory) = std::env::var_os("CONDUIT_HISTORY_BROWSER_PROOF_OUTPUT") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join(name),
        if name == "history-window-face.json" {
            // Keep the bounded cross-Mask input compact; pretty-print whitespace
            // is not part of the Face or the admitted transport payload.
            serde_json::to_vec(value).unwrap()
        } else {
            serde_json::to_vec_pretty(value).unwrap()
        },
    )
    .unwrap();
}
fn assert_pending(mask: &mut OwnerBrowserMask) {
    assert_eq!(mask.show.show.lifecycle, ManifestationLifecycle::Prepared);
    assert!(
        mask.scheduler
            .remote_egress_offer(mask.show_boundary.endpoint, mask.show_boundary.cord)
            .unwrap()
            .is_none()
    );
    assert_eq!(mask.view().show_state, "prepared");
}
#[test]
fn checked_history_view_preserves_exact_face_and_waits_for_matching_show_ack() {
    let face = history_face();
    let original = face.clone();
    let mut mask = prepare(face.clone());
    assert_eq!(mask.face, face);
    assert_eq!(mask.planned.mask.plot_name, HISTORY_COMPONENT_MASK_ROOT);
    assert_pending(&mut mask);
    let prepared = mask.view();
    assert!(
        serde_json::to_vec(&prepared).unwrap().len()
            <= abi::conduit_browser_owner_face_output_capacity()
    );
    assert!(!prepared.interactions_admitted);
    assert!(prepared.actions.is_empty());
    assert_eq!(prepared.face_revision, REVISION.to_string());
    let json = serde_json::to_value(&prepared).unwrap();
    let component = &json["component"];
    assert!(component["fallback"].is_null());
    assert_eq!(component["recipe_identity"], HISTORY_COMPONENT_RECIPE_ID);
    let history = &component["history"];
    assert_eq!(history["face_id"], face.identity.as_str());
    assert_eq!(history["face_revision"], REVISION.to_string());
    assert_eq!(history["subject"], "environment-history");
    assert_eq!(
        history["producer"],
        serde_json::json!(["sensor/test", "boot/test", "plan/sensor", "play/sensor"])
    );
    assert_eq!(history["omitted"], "0");
    let samples = history["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0]["sequence"], "1");
    assert_eq!(samples[0]["boot_ms"], "1000");
    assert_eq!(history["temperature_unit"], "°C");
    assert_eq!(samples[0]["temperature"], "19.800");
    assert_eq!(samples[0]["temperature_source"], "19.8°C");
    assert_eq!(samples[0]["humidity"], "59.000");
    assert_eq!(samples[0]["x"], 32);
    assert_eq!(samples[1]["x"], 88);
    assert_eq!(samples[1]["failure"], "checksum");
    assert!(samples[1]["temperature"].is_null());
    assert!(samples[1]["humidity"].is_null());
    assert!(samples[1]["temperature_y"].is_null());
    assert!(samples[1]["humidity_y"].is_null());
    assert_eq!(samples[2]["x"], 592);
    assert_eq!(samples[2]["gap_before"], serde_json::json!(["3", "3"]));
    assert!(
        history["samples"]
            .as_array()
            .unwrap()
            .iter()
            .all(|sample| sample["sequence"].is_string() && sample["boot_ms"].is_string())
    );
    // Chromium can render the actual pending view and then independently enact
    // visibility/acknowledgement. Do not relabel it as Available for the export.
    export("history-view.json", &prepared);
    export("history-face.json", &face);
    assert!(
        mask.acknowledge(Acknowledgement {
            show_id: prepared.show_id.clone(),
            face_id: prepared.face_id.clone(),
            face_revision: (REVISION + 1).to_string(),
        })
        .is_err()
    );
    assert_pending(&mut mask);
    mask.acknowledge(Acknowledgement {
        show_id: prepared.show_id.clone(),
        face_id: prepared.face_id.clone(),
        face_revision: prepared.face_revision.clone(),
    })
    .unwrap();
    assert_eq!(mask.view().show_state, "available");
    mask.show.validate(&face).unwrap();
    assert_eq!(mask.face, original);
    let available = mask.view();
    assert_eq!(
        serde_json::to_value(&available).unwrap()["component"]["history"],
        *history
    );
    assert!(available.actions.is_empty());
    export("history-acknowledged-view.json", &available);
    assert!(
        mask.acknowledge(Acknowledgement {
            show_id: prepared.show_id,
            face_id: prepared.face_id,
            face_revision: prepared.face_revision,
        })
        .is_err()
    );
}
#[test]
fn malformed_history_retains_generic_face_instead_of_drawing_partial_graph() {
    let mut face = history_face();
    let property = face
        .properties
        .iter_mut()
        .find(|property| {
            property.subject == "environment-history/sample/2"
                && property.name == "sampled-at-boot-ms"
        })
        .unwrap();
    property.value = V::Count(0); // Valid Face property; invalid producer clock order.
    face.reseal_prepared().unwrap();
    let original = face.clone();
    let mut mask = prepare(face);
    assert_pending(&mut mask);
    let view = mask.view();
    let json = serde_json::to_value(&view).unwrap();
    assert!(json["component"]["history"].is_null());
    assert!(
        json["component"]["fallback"]
            .as_str()
            .unwrap()
            .starts_with("history graph refused:")
    );
    assert_eq!(view.subjects.len(), original.subjects.len());
    assert_eq!(view.relationships.len(), original.relationships.len());
    assert_eq!(view.actions.len(), original.actions.len());
    assert_eq!(mask.face, original);
    export("history-fallback-view.json", &view);
}

#[test]
fn full_recording_chunk_has_an_explicit_window_that_fits_the_checked_browser_mask() {
    let mut history = common::prepared(1);
    history
        .read_next(&common::chunk(
            &(1..=64).map(|n| (n, n * 1000, true)).collect::<Vec<_>>(),
        ))
        .unwrap();
    let window = conduit_environment_history::HistoryFaceWindow::prepare(8).unwrap();
    let face = face_from_fragment(
        history
            .contribution_window(common::basis(), window)
            .unwrap(),
    );
    assert_eq!(history.recovered().samples, 64);
    let mut mask = prepare(face.clone());
    assert_pending(&mut mask);
    let view = mask.view();
    let bytes = serde_json::to_vec(&view).unwrap();
    assert!(bytes.len() <= abi::conduit_browser_owner_face_output_capacity());
    let json = serde_json::to_value(&view).unwrap();
    let graph = &json["component"]["history"];
    assert!(json["component"]["fallback"].is_null());
    assert_eq!(graph["omitted"], "56");
    assert_eq!(graph["samples"].as_array().unwrap().len(), 8);
    assert_eq!(graph["samples"][0]["sequence"], "57");
    assert_eq!(graph["samples"][7]["sequence"], "64");
    assert_eq!(mask.face, face);
    export("history-window-face.json", &face);
    export("history-window-view.json", &view);
}

#[test]
fn checked_history_unit_selection_replans_same_face_and_refuses_different_retained_input() {
    use conduit_face::*;
    let face = history_face();
    let mut plan_ids = std::collections::BTreeSet::new();
    let mut coordinates = None;
    for (unit, label, expected_value) in [
        (LookTemperatureUnit::Celsius, "°C", "19.800"),
        (LookTemperatureUnit::Kelvin, "K", "292.950"),
        (LookTemperatureUnit::Fahrenheit, "°F", "67.640"),
    ] {
        let base = editorial_look().unwrap();
        let preferences = LookPreferencesInfo::new(
            LookPreferenceLookOption::None,
            LookPreferenceTemperatureOption::Some(unit.into()),
            1,
        )
        .unwrap();
        let resolved = resolve_look_preferences(&base, Some(&preferences), None).unwrap();
        let mut mask = prepare_with_look(face.clone(), Some(resolved.requested()));
        assert_eq!(mask.face, face);
        plan_ids.insert(mask.planned.plan.plan_id.clone());
        let view = serde_json::to_value(mask.view()).unwrap();
        let history = &view["component"]["history"];
        assert_eq!(history["temperature_unit"], label);
        assert_eq!(history["samples"][0]["temperature"], expected_value);
        assert_eq!(history["samples"][0]["temperature_source"], "19.8°C");
        let points = history["samples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sample| {
                (
                    sample["x"].clone(),
                    sample["temperature_y"].clone(),
                    sample["humidity_y"].clone(),
                )
            })
            .collect::<Vec<_>>();
        if let Some(expected) = &coordinates {
            assert_eq!(&points, expected);
        } else {
            coordinates = Some(points);
        }
        let host = crate::installed_browser::membership_advertisement(
            "host/browser-history-cpu-fixture".into(),
            "boot/browser-history-cpu-fixture".into(),
        );
        assert!(
            conduit_browser_mask_offer::configured_mask_configuration(&host, &mask.planned)
                .is_err()
        );
        let wrong = phosphor_look().unwrap();
        let bare = conduit_browser_mask_offer::planned_mask(
            &host,
            conduit_browser_mask_offer::MASK_SOURCE,
            "browser-graphical",
        )
        .unwrap();
        assert!(
            conduit_browser_mask_offer::configured_mask_configuration_with_look(
                &host,
                &bare,
                Some(resolved.requested())
            )
            .is_err()
        );
        assert!(
            conduit_browser_mask_offer::configured_mask_configuration(&host, &bare)
                .unwrap()
                .is_none()
        );
        assert!(
            conduit_browser_mask_offer::configured_mask_configuration_with_look(
                &host,
                &mask.planned,
                Some(&wrong)
            )
            .is_err()
        );
        let prepared = mask.view();
        assert_pending(&mut mask);
        mask.acknowledge(Acknowledgement {
            show_id: prepared.show_id,
            face_id: prepared.face_id,
            face_revision: prepared.face_revision,
        })
        .unwrap();
        assert_eq!(mask.view().show_state, "available");
        mask.show.validate(&face).unwrap();
        assert_eq!(mask.face, face);
        export(
            &format!("history-unit-{}.json", label.replace('°', "")),
            &view,
        );
    }
    assert_eq!(plan_ids.len(), 3);
}

#[path = "history_owner_tests.rs"]
mod owner_entry;
