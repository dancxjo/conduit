//! Exact owner Face, Show, and typed-action behavior in the browser Mask.

use super::*;
use conduit_body::Body;
use conduit_core::{
    kind_id, process_owned_line_offer_with_limits, BaseImplementationId, CheckedPlotId,
    CheckedValueContract, LineOffer, LinkLimits, SignId, SourceDocumentId, ValueConstraint,
};
use conduit_presentation::{
    Face, FaceActionArgument, FaceContext, FaceFocus, PresentationAction,
    PresentationActionAvailability, PresentationDisclosureLevel, UTF8_TEXT_VALUE_KIND,
};

#[test]
fn exact_owner_face_runs_the_browser_mask_and_acknowledges_its_show() {
    let body = Body::born(
        SourceDocumentId::from("source/owner-face-mask"),
        CheckedPlotId::from("checked/owner-face-mask"),
        1,
        SignId::from("sign/born-owner-face-mask"),
    )
    .unwrap();
    let revision = 9_007_199_254_740_993;
    let face = Face::project(
        &body,
        None,
        revision,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .presentation;
    let basis = HostBasis {
        body_id: body.body_id.clone(),
        host_id: HostId::from("host/browser-owner-face"),
        boot_id: BootId::from("boot/browser-owner-face"),
    };
    let mut mask = OwnerBrowserMask::prepare_component_fixture(basis, face, 1, false).unwrap();
    let prepared = mask.view();
    assert_eq!(prepared.face_revision, revision.to_string());
    assert_eq!(prepared.show_state, "prepared");
    assert!(!prepared.interactions_admitted);
    assert!(mask
        .acknowledge(Acknowledgement {
            show_id: prepared.show_id.clone(),
            face_id: prepared.face_id.clone(),
            face_revision: prepared.face_revision.clone(),
        })
        .is_ok());
    assert_eq!(mask.view().show_state, "available");
    assert!(mask
        .acknowledge(Acknowledgement {
            show_id: prepared.show_id,
            face_id: prepared.face_id,
            face_revision: prepared.face_revision,
        })
        .is_err());
}

#[test]
fn selected_carrier_lines_survive_browser_lowering_and_bound_show() {
    let body = Body::born(
        SourceDocumentId::from("source/selected-owner-face"),
        CheckedPlotId::from("checked/selected-owner-face"),
        1,
        SignId::from("sign/selected-owner-face"),
    )
    .unwrap();
    let face = Face::project(
        &body,
        None,
        4,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .presentation;
    let basis = HostBasis {
        body_id: body.body_id,
        host_id: HostId::from("host/selected-browser"),
        boot_id: BootId::from("boot/selected-browser"),
    };
    let browser = crate::installed_browser::membership_advertisement(
        basis.host_id.clone(),
        basis.boot_id.clone(),
    );
    let mut owner = browser.clone();
    owner.host_id = HostId::from("host/selected-owner");
    owner.boot_id = BootId::from("boot/selected-owner");
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 64 * 1024,
        maximum_buffered_bytes: 256 * 1024,
        maximum_frame_bytes: 64 * 1024,
    };
    let line = |id: &str,
                source: &conduit_core::HostAdvertisement,
                sink: &conduit_core::HostAdvertisement|
     -> LineOffer {
        let mut offered = process_owned_line_offer_with_limits(
            id,
            &format!("binding/{id}"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "websocket/selected",
            source,
            sink,
            limits,
        );
        offered.contract = conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0].contract;
        offered
    };
    let face_line = line("face/selected", &owner, &browser);
    let return_line = line("return/selected", &browser, &owner);
    let planned = conduit_browser_mask_offer::planned_owner_face_show_mask(
        &browser,
        &owner,
        &face_line,
        &return_line,
        conduit_browser_mask_offer::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    assert!(OwnerBrowserMask::prepare_planned(
        HostBasis {
            body_id: basis.body_id.clone(),
            host_id: basis.host_id.clone(),
            boot_id: basis.boot_id.clone(),
        },
        face.clone(),
        planned.clone(),
        None,
        Some((return_line.admitted_line(), face_line.admitted_line())),
        1,
        false,
    )
    .is_err());
    let mut mask = OwnerBrowserMask::prepare_planned(
        basis,
        face,
        planned,
        None,
        Some((face_line.admitted_line(), return_line.admitted_line())),
        1,
        false,
    )
    .unwrap();
    let prepared = mask.view();
    mask.acknowledge(Acknowledgement {
        show_id: prepared.show_id,
        face_id: prepared.face_id,
        face_revision: prepared.face_revision,
    })
    .unwrap();
    assert_eq!(mask.view().show_state, "available");
    assert!(
        serde_json::to_vec(&mask.show).unwrap().len()
            <= conduit_presentation::MAX_OWNER_FACE_RESPONSE_BYTES
    );
}

#[test]
fn current_browser_mask_emits_one_exact_typed_clock_interaction() {
    let body = Body::born(
        SourceDocumentId::from("source/owner-browser-action"),
        CheckedPlotId::from("checked/owner-browser-action"),
        1,
        SignId::from("sign/born-owner-browser-action"),
    )
    .unwrap();
    let revision = 9_007_199_254_740_993;
    let face = Face::project(
        &body,
        None,
        revision,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .presentation;
    let target = face.subjects[0].identity.clone();
    let action = PresentationAction {
        identity: "body/action/change-clock-interval/1".into(),
        intent: "conduit.intent/change-clock-interval@1".into(),
        target: target.clone(),
        name: "Change clock interval".into(),
        arguments: vec![FaceActionArgument {
            name: "clock/interval-ms".into(),
            value_name: "Clock interval".into(),
            contract: CheckedValueContract::new(
                kind_id(UTF8_TEXT_VALUE_KIND),
                4,
                vec![ValueConstraint::CanonicalMembership {
                    members: vec![b"250".to_vec(), b"500".to_vec()],
                    negated: false,
                }],
            )
            .unwrap(),
        }],
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: PresentationActionAvailability::Available,
    };
    let face = Presentation::new_with_semantics_and_temporal(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        vec![action.clone()],
        face.disclosures,
        face.temporal_references,
        face.temporal_facts,
    )
    .unwrap();
    let basis = HostBasis {
        body_id: body.body_id,
        host_id: HostId::from("host/browser-owner-action"),
        boot_id: BootId::from("boot/browser-owner-action"),
    };
    let mut mask = OwnerBrowserMask::prepare_component_fixture(basis, face, 1, true).unwrap();
    let prepared = mask.view();
    assert_eq!(prepared.actions[0].arguments[0].choices, ["250", "500"]);
    assert_eq!(
        prepared.actions[0].arguments[0].value_kind,
        UTF8_TEXT_VALUE_KIND
    );
    assert_eq!(prepared.actions[0].arguments[0].maximum_bytes, 4);
    mask.acknowledge(Acknowledgement {
        show_id: prepared.show_id.clone(),
        face_id: prepared.face_id.clone(),
        face_revision: prepared.face_revision.clone(),
    })
    .unwrap();
    let proposed = |revision: &str, interval: &str| ProposedInteraction {
        show_id: prepared.show_id.clone(),
        face_id: prepared.face_id.clone(),
        face_revision: revision.into(),
        action_id: action.identity.clone(),
        target: target.clone(),
        arguments: vec![ProposedArgument {
            name: "clock/interval-ms".into(),
            value: interval.into(),
        }],
        sequence: 1,
    };
    assert!(mask.interact(proposed("9007199254740992", "500")).is_err());
    assert!(mask
        .interact(proposed(&prepared.face_revision, "3000"))
        .is_err());
    let mut wrong_argument = proposed(&prepared.face_revision, "500");
    wrong_argument.arguments[0].name = "clock/other".into();
    assert!(mask.interact(wrong_argument).is_err());
    let emitted = mask
        .interact(proposed(&prepared.face_revision, "500"))
        .unwrap();
    assert_eq!(emitted.interaction.face_revision, revision);
    assert_eq!(emitted.interaction.arguments[0].value, b"500");
    assert_eq!(emitted.show.show_id.as_str(), prepared.show_id);
    assert!(mask
        .interact(proposed(&prepared.face_revision, "250"))
        .is_err());
}
