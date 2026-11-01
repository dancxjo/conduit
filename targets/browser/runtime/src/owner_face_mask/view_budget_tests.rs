//! Full component-row view budget with retained canonical startup custody.
use super::*;
use conduit_face::{FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject};

#[test]
fn twenty_unicode_rows_and_actions_fit_the_view_without_duplicate_look_bytes() {
    let body = Body::born(
        SourceDocumentId::from("source/view-budget"),
        CheckedPlotId::from("checked/view-budget"),
        1,
        SignId::from("sign/view-budget"),
    )
    .unwrap();
    let base = BodyFace::project(
        &body,
        None,
        1,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face;
    let mut subjects = base.subjects;
    let mut relationships = base.relationships;
    let mut actions = base.actions;
    subjects.push(FaceSubject {
        identity: "todo/list".into(),
        role: FaceRole::Collection,
        name: "View budget".into(),
    });
    for index in 0..20 {
        let identity = format!("todo/item/task-{index}");
        subjects.push(FaceSubject {
            identity: identity.clone(),
            role: FaceRole::Item,
            name: "É".repeat(32),
        });
        relationships.push(FaceRelationship {
            source: "todo/list".into(),
            target: identity.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        for verb in ["complete", "remove"] {
            actions.push(FaceAction {
                identity: format!("todo.{verb}.task-{index}"),
                intent: format!("todo/{verb}@1"),
                target: identity.clone(),
                name: verb.into(),
                arguments: vec![],
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability: FaceActionAvailability::Available,
            });
        }
    }
    let face = Face::new_with_semantics_and_temporal(
        base.revision,
        base.basis,
        subjects,
        relationships,
        base.properties,
        base.text,
        actions,
        base.disclosures,
        base.temporal_references,
        base.temporal_facts,
    )
    .unwrap();
    let host_id = HostId::from("host/view-budget");
    let boot_id = BootId::from("boot/view-budget");
    let host = crate::installed_browser::membership_advertisement(host_id.clone(), boot_id.clone());
    let owner = crate::installed_browser::membership_advertisement(
        HostId::from("owner/view-budget"),
        BootId::from("owner/view-budget/boot"),
    );
    let line = |name: &str,
                source: &conduit_core::HostAdvertisement,
                sink: &conduit_core::HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            name,
            &format!("binding/{name}"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "websocket/view-budget",
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
    let face_line = line("face/view-budget", &owner, &host);
    let returned = line("show/view-budget", &host, &owner);
    let interaction = line("interaction/view-budget", &host, &owner);
    for look in [
        conduit_face::ComponentMaskLook::Compact,
        conduit_face::ComponentMaskLook::Spacious,
        conduit_face::ComponentMaskLook::CompactEditorial,
        conduit_face::ComponentMaskLook::SpaciousPhosphor,
    ] {
        let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
            &host,
            &owner,
            &face_line,
            &returned,
            &interaction,
            conduit_face::TODO_COMPONENT_MASK_SOURCE,
            conduit_face::todo_component_mask_root(
                look,
                conduit_face::TodoComponentRecipe::Grouped,
            ),
        )
        .unwrap();
        let mask = OwnerBrowserMask::prepare_planned(
            HostBasis {
                body_id: body.body_id.clone(),
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                visual_constraints: Some(component::VisualConstraints {
                    forced_colors: true,
                    reduced_motion: true,
                    minimum_text_scale_percent: 200,
                    ..Default::default()
                }),
            },
            face.clone(),
            planned,
            None,
            Some((
                face_line.admitted_line(),
                returned.admitted_line(),
                Some(interaction.admitted_line()),
            )),
            1,
            false,
        )
        .unwrap();
        let component = mask.component.as_ref().unwrap();
        assert!(!component.configuration.look.canonical_value().is_empty());
        let retained = serde_json::to_value(&component.configuration).unwrap();
        assert!(retained["look"]["canonical_value"].is_array());
        let view = serde_json::to_value(mask.view()).unwrap();
        assert_eq!(view["schema"], "conduit.browser/owner-face-mask@2");
        assert_eq!(
            view["component"]["configuration"]["look"]["kind"],
            "retained-startup"
        );
        assert!(view["component"]["configuration"]["look"]
            .get("canonical_value")
            .is_none());
        assert_eq!(
            view["component"]["selected_look"],
            serde_json::to_value(&component.selected_look).unwrap()
        );
        assert_eq!(
            view["actions"].as_array().unwrap().len(),
            face.actions.len()
        );
        assert_eq!(
            view["subjects"].as_array().unwrap().len(),
            face.subjects.len()
        );
        let bytes = serde_json::to_vec(&view).unwrap();
        eprintln!("20 long rows {look:?}: {} bytes", bytes.len());
        assert!(bytes.len() <= abi::conduit_browser_owner_face_output_capacity());
    }
}
