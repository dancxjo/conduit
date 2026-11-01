//! Checked browser Mask preparation over authored Looks; no live DOM/owner receipt.
use super::*;
use conduit_core::{process_owned_line_offer_with_limits, HostAdvertisement, LinkLimits};
use conduit_face::{
    admit_visual_look, field_instrument_day_look, field_instrument_night_look,
    VisualUserConstraints, FIELD_INSTRUMENT_PATCHBAY_MASK_SOURCE,
    FIELD_INSTRUMENT_TODO_MASK_SOURCE,
};
use conduit_plot::rust_binding::NativeRustBinding;

fn line(
    name: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> conduit_core::LineOffer {
    let mut line = process_owned_line_offer_with_limits(
        name,
        &format!("binding/{name}"),
        "conduit.base/websocket-rfc6455@1".into(),
        "websocket/field-reuse",
        source,
        sink,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 64 * 1024,
            maximum_frame_bytes: 64 * 1024,
            maximum_buffered_bytes: 256 * 1024,
        },
    );
    line.contract = conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0].contract;
    line
}

fn qualify(face: &Face, capture: Option<&std::path::Path>) {
    let browser = crate::installed_browser::membership_advertisement(
        "host/field-reuse".into(),
        "boot/field-reuse".into(),
    );
    let owner = crate::installed_browser::membership_advertisement(
        "owner/field-reuse".into(),
        "owner/field-reuse/boot".into(),
    );
    let sent = line("face", &owner, &browser);
    let returned = line("show", &browser, &owner);
    let interaction = line("interaction", &browser, &owner);
    let mut structured = false;
    let mut contents = None;
    for (source, root) in [
        (
            FIELD_INSTRUMENT_TODO_MASK_SOURCE,
            "todo-mask-flat-field-instrument",
        ),
        (
            FIELD_INSTRUMENT_PATCHBAY_MASK_SOURCE,
            "patchbay-mask-field-instrument",
        ),
    ] {
        let mut document = None;
        let mut previous_ack: Option<(String, String, String)> = None;
        for (theme, expected) in [
            ("day", field_instrument_day_look().unwrap()),
            ("night", field_instrument_night_look().unwrap()),
        ] {
            let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
                &browser,
                &owner,
                &sent,
                &returned,
                &interaction,
                source,
                &format!("{root}-{theme}"),
            )
            .unwrap();
            for scale in [100, 200] {
                let mut mask = OwnerBrowserMask::prepare_planned(
                    HostBasis {
                        body_id: face.basis.body_id.clone().unwrap(),
                        host_id: "host/field-reuse".into(),
                        boot_id: "boot/field-reuse".into(),
                        visual_constraints: Some(component::VisualConstraints {
                            forced_colors: false,
                            reduced_motion: true,
                            minimum_text_scale_percent: scale,
                        }),
                    },
                    face.clone(),
                    planned.clone(),
                    None,
                    Some((
                        sent.admitted_line(),
                        returned.admitted_line(),
                        Some(interaction.admitted_line()),
                    )),
                    1,
                    false,
                )
                .unwrap();
                let component = mask.component.as_ref().unwrap();
                structured |= component.fallback.is_none();
                assert_eq!(
                    component.configuration.look.canonical_value(),
                    expected.clone().encode().unwrap()
                );
                let selected = admit_visual_look(
                    &expected,
                    VisualUserConstraints {
                        forced_colors: false,
                        reduced_motion: true,
                        minimum_text_scale_percent: scale,
                        maximum_supported_text_scale_percent: 200,
                    },
                )
                .unwrap();
                assert_eq!(
                    component.selected_look.canonical_value(),
                    selected.selected.encode().unwrap()
                );
                if let Some(previous) = &document {
                    assert_eq!(
                        &component.configuration.component.source_document_id,
                        previous
                    );
                }
                document = Some(component.configuration.component.source_document_id.clone());
                let view = mask.view();
                if theme == "night" {
                    let (show_id, face_id, face_revision) = previous_ack.as_ref().unwrap();
                    assert!(mask
                        .acknowledge(Acknowledgement {
                            show_id: show_id.clone(),
                            face_id: face_id.clone(),
                            face_revision: face_revision.clone(),
                        })
                        .is_err());
                }
                mask.acknowledge(Acknowledgement {
                    show_id: view.show_id.clone(),
                    face_id: view.face_id.clone(),
                    face_revision: view.face_revision.clone(),
                })
                .unwrap();
                if theme == "day" {
                    previous_ack = Some((view.show_id, view.face_id, view.face_revision));
                }
                let view = mask.view();
                assert_eq!(view.show_state, "available");
                assert!(!view.interactions_admitted);
                assert_eq!(view.face_id, face.identity.as_str());
                assert_eq!(view.face_revision, face.revision.to_string());
                assert_eq!(view.subjects.len(), face.subjects.len());
                assert_eq!(view.actions.len(), face.actions.len());
                assert_eq!(view.relationships.len(), face.relationships.len());
                let projected =
                    serde_json::to_value((&view.subjects, &view.relationships, &view.actions))
                        .unwrap();
                if let Some(previous) = &contents {
                    assert_eq!(&projected, previous);
                }
                contents = Some(projected);
                assert!(
                    serde_json::to_vec(&view).unwrap().len()
                        <= abi::conduit_browser_owner_face_output_capacity()
                );
                if let Some(directory) = capture {
                    // Private, explicitly requested software evidence. This is
                    // not a browser having acknowledged these Shows on a DOM.
                    std::fs::create_dir_all(directory).unwrap();
                    std::fs::write(
                        directory.join(format!("{root}-{theme}-{scale}.json")),
                        serde_json::to_vec(&view).unwrap(),
                    )
                    .unwrap();
                }
                assert_eq!(&mask.face, face);
            }
        }
    }
    assert!(
        structured,
        "at least one checked component must avoid generic fallback"
    );
}

#[test]
fn authored_field_looks_reach_both_planned_browser_components() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/face-waist/domain-derived/open-domain-face.json"
    ))
    .unwrap();
    qualify(&face, None);
}

#[test]
#[ignore = "requires explicitly supplied retained real environmental and Todo Face exports"]
fn retained_real_faces_use_the_same_authored_browser_looks() {
    #[derive(Deserialize)]
    struct RetainedExport {
        face: Face,
    }
    let capture =
        std::env::var_os("CONDUIT_FIELD_BROWSER_CAPTURE_DIR").map(std::path::PathBuf::from);
    for (name, key) in [
        ("environment", "CONDUIT_FIELD_ENVIRONMENT_FACE"),
        ("todo", "CONDUIT_FIELD_TODO_FACE"),
    ] {
        let path = std::env::var_os(key).expect("explicit retained Face path");
        let export: RetainedExport = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let directory = capture.as_ref().map(|path| path.join(name));
        qualify(&export.face, directory.as_deref());
    }
}
