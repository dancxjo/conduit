use super::*;
use crate::native_workset::{self, NativeForm};
use crate::product_journey::WorkspaceForm;
use alloc::vec;
use conduit_body::{Body, BodyWorkset};
use conduit_core::{BootId, HostId, OfferGeneration, SignId};

fn fixture() -> (FrontDoor, JourneyProjection, WorkspaceProjection) {
    let resident = native_workset::resident(NativeForm::MemoryLantern).unwrap();
    let body = Body::born_with_forms(
        BodyWorkset::one(resident.clone()).unwrap(),
        1,
        SignId::from("born"),
    )
    .unwrap();
    let mut journey = super::super::tests::born_projection(body.body_id.clone());
    journey.source_document_id = resident.source_document_id.clone();
    journey.checked_form_id = resident.checked_form_id.clone();
    let workspace = WorkspaceProjection {
        body_id: body.body_id,
        revision: journey.revision,
        forms: vec![WorkspaceForm {
            form: resident.clone(),
            title: "Memory Lantern",
            foreground: true,
        }],
    };
    let door = FrontDoor::new(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(3),
        "profile",
        "build",
        "image",
        resident.source_document_id,
        resident.checked_form_id,
        7,
        true,
    );
    (door, journey, workspace)
}
#[test]
fn body_view_requires_exact_membership_foreground_and_current_host_revision() {
    let (mut door, journey, workspace) = fixture();
    door.observe_body(journey.clone(), workspace.clone())
        .unwrap();
    let revision = door.revision();
    let mut duplicate = workspace.clone();
    duplicate.forms.push(duplicate.forms[0].clone());
    assert!(door.observe_body(journey.clone(), duplicate).is_err());
    let mut wrong = journey.clone();
    wrong.boot_id = "other-boot".into();
    assert!(door.observe_body(wrong, workspace.clone()).is_err());
    let mut stale = journey.clone();
    stale.revision -= 1;
    let mut stale_workspace = workspace.clone();
    stale_workspace.revision -= 1;
    assert!(door.observe_body(stale, stale_workspace).is_err());
    assert_eq!(door.revision(), revision);
}
#[test]
fn empty_and_full_memory_text_manifest_through_the_native_scene() {
    let (mut door, mut journey, workspace) = fixture();
    for value in [alloc::string::String::new(), "a".repeat(256)] {
        journey.result = Some(value);
        door.observe_body(journey.clone(), workspace.clone())
            .unwrap();
        let scene = door.scene(&super::super::tests::Sink).unwrap();
        crate::display::render_scene(&mut super::super::tests::Sink, &scene).unwrap();
    }
}
