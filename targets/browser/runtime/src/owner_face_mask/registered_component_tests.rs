//! Registered component over a bounded synthetic node and an oversized domain fixture.
//! CPU Show/interaction evidence, not a live owner or browser execution receipt.
use super::*;
use conduit_core::process_owned_line_offer_with_limits;
use conduit_face::{
    patchbay_component_mask_root, ComponentMaskLook, PATCHBAY_COMPONENT_MASK_SOURCE,
    PATCHBAY_DEFAULT_MASK_ROOT,
};

#[test]
fn registered_patchbay_looks_preserve_node_and_refuse_oversized_domain_face() {
    let oversized: Face = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    let host_id = HostId::from("host/registered-patchbay");
    let boot_id = BootId::from("boot/registered-patchbay");
    let browser =
        crate::installed_browser::membership_advertisement(host_id.clone(), boot_id.clone());
    let owner = crate::installed_browser::membership_advertisement(
        "owner/registered-patchbay".into(),
        "owner/registered-patchbay/boot".into(),
    );
    let line = |name: &str,
                source: &conduit_core::HostAdvertisement,
                sink: &conduit_core::HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            name,
            &format!("binding/{name}"),
            conduit_core::BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "websocket/registered-patchbay",
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
    let sent = line("face", &owner, &browser);
    let returned = line("show", &browser, &owner);
    let interaction = line("interaction", &browser, &owner);
    let face = bounded_node_fixture();
    for (name, root, forced) in [
        (
            "phosphor",
            patchbay_component_mask_root(ComponentMaskLook::Compact),
            false,
        ),
        (
            "editorial",
            patchbay_component_mask_root(ComponentMaskLook::CompactEditorial),
            false,
        ),
        ("default", PATCHBAY_DEFAULT_MASK_ROOT, false),
        (
            "phosphor-forced",
            patchbay_component_mask_root(ComponentMaskLook::Compact),
            true,
        ),
    ] {
        let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
            &browser,
            &owner,
            &sent,
            &returned,
            &interaction,
            PATCHBAY_COMPONENT_MASK_SOURCE,
            root,
        )
        .unwrap();
        let oversized_result = OwnerBrowserMask::prepare_planned(
            HostBasis {
                body_id: oversized.basis.body_id.clone().unwrap(),
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                visual_constraints: None,
            },
            oversized.clone(),
            planned.clone(),
            None,
            Some((
                sent.admitted_line(),
                returned.admitted_line(),
                Some(interaction.admitted_line()),
            )),
            1,
            false,
        );
        match oversized_result {
            Err(reason) => assert_eq!(reason, "browser Mask Face exceeds its sealed Fore bound"),
            Ok(_) => panic!("oversized complete domain fixture exceeded its admitted carrier"),
        }
        let mut mask = OwnerBrowserMask::prepare_planned(
            HostBasis {
                body_id: face.basis.body_id.clone().unwrap(),
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                visual_constraints: Some(component::VisualConstraints {
                    forced_colors: forced,
                    reduced_motion: true,
                    minimum_text_scale_percent: if forced { 200 } else { 100 },
                    ..Default::default()
                }),
            },
            face.clone(),
            planned,
            None,
            Some((
                sent.admitted_line(),
                returned.admitted_line(),
                Some(interaction.admitted_line()),
            )),
            1,
            true,
        )
        .unwrap();
        assert_eq!(mask.face, face);
        let component = mask.component.as_ref().unwrap();
        assert!(component.fallback.is_none(), "{:?}", component.fallback);
        assert_eq!(
            component.recipe_identity.as_deref(),
            Some("conduit.face/patchbay-nodes@1")
        );
        assert!(component.fallback.is_none(), "{:?}", component.fallback);
        for (subject, region) in face.subjects.iter().zip(&component.subject_regions) {
            assert_eq!(
                *region,
                match subject.role {
                    conduit_face::FaceRole::Gear => Some(0),
                    conduit_face::FaceRole::Port => Some(1),
                    conduit_face::FaceRole::Cord => Some(2),
                    _ => None,
                }
            );
        }
        let prepared = mask.view();
        assert_eq!(prepared.subjects.len(), face.subjects.len());
        assert_eq!(prepared.actions.len(), face.actions.len());
        assert_eq!(prepared.relationships.len(), face.relationships.len());
        mask.acknowledge(Acknowledgement {
            show_id: prepared.show_id.clone(),
            face_id: prepared.face_id.clone(),
            face_revision: prepared.face_revision.clone(),
        })
        .unwrap();
        let view = mask.view();
        let bytes = serde_json::to_vec(&view).unwrap();
        eprintln!("registered Patchbay {name}: {} bytes", bytes.len());
        assert!(bytes.len() <= abi::conduit_browser_owner_face_output_capacity());
        eprintln!(
            "REGISTERED_PATCHBAY_{}={}",
            name.to_uppercase(),
            String::from_utf8(bytes).unwrap()
        );
        let action = face
            .actions
            .iter()
            .find(|action| action.availability == conduit_face::FaceActionAvailability::Available)
            .unwrap();
        let emitted = mask
            .interact(ProposedInteraction {
                show_id: prepared.show_id,
                face_id: prepared.face_id,
                face_revision: prepared.face_revision,
                action_id: action.identity.clone(),
                target: action.target.clone(),
                arguments: vec![],
                sequence: 1,
            })
            .unwrap();
        assert_eq!(emitted.interaction.action_id, action.identity);
        assert_eq!(emitted.interaction.target, action.target);
    }
}

// This Face deliberately claims no producing Source/Plan/Play. It is not a
// filtered version of the oversized source-derived domain fixture.
fn bounded_node_fixture() -> Face {
    use conduit_face::*;
    let body = conduit_body::Body::born(
        conduit_core::SourceDocumentId::from("source/registered-node-fixture"),
        conduit_core::CheckedPlotId::from("checked/registered-node-fixture"),
        1,
        SignId::from("sign/registered-node-fixture"),
    )
    .unwrap();
    let subjects = vec![
        FaceSubject {
            identity: "fixture/gear".into(),
            role: FaceRole::Gear,
            name: "Echo — café 日本語".into(),
        },
        FaceSubject {
            identity: "fixture/input".into(),
            role: FaceRole::Port,
            name: "Message in".into(),
        },
        FaceSubject {
            identity: "fixture/output".into(),
            role: FaceRole::Port,
            name: "Message out".into(),
        },
    ];
    let disclosures = subjects
        .iter()
        .map(|subject| FaceDisclosure {
            subject: subject.identity.clone(),
            level: FaceDisclosureLevel::Primary,
        })
        .collect();
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: Some(body.body_id),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        subjects,
        vec![
            FaceRelationship {
                source: "fixture/gear".into(),
                target: "fixture/input".into(),
                kind: FaceRelationshipKind::Contains,
            },
            FaceRelationship {
                source: "fixture/gear".into(),
                target: "fixture/output".into(),
                kind: FaceRelationshipKind::Contains,
            },
        ],
        vec![],
        vec![],
        vec![FaceAction {
            identity: "fixture/inspect".into(),
            intent: "conduit.intent/inspect@1".into(),
            target: "fixture/gear".into(),
            name: "Inspect Echo node".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        disclosures,
    )
    .unwrap()
}
