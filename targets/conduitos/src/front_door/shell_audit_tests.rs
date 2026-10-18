//! Concrete scene/Face differences; unsupported context is deliberately visible.
use super::*;
use crate::{
    native_workset::{self, NativePlot},
    product_journey::{WorkspacePlot, WorkspaceProjection},
};
use alloc::vec::Vec;
use conduit_face::{FaceActionAvailability, FacePropertyValue, GraphicsScene};

struct AuditDisplay;
impl crate::display::PixelTarget for AuditDisplay {
    fn format(&self) -> crate::display::DisplayFormat {
        crate::display::DisplayFormat {
            width: 1024,
            height: 768,
            pitch: 4096,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), crate::display::DisplayError> {
        Ok(())
    }
}
fn scene(door: &FrontDoor) -> GraphicsScene {
    let scene = door.scene(&AuditDisplay).unwrap();
    let face = door.face().unwrap();
    let shown: Vec<_> = scene
        .commands()
        .iter()
        .map(|command| command.payload())
        .filter(|text| !text.is_empty())
        .collect();
    std::println!(
        "SHELL_AUDIT {}",
        serde_json::json!({"face": face, "shown": shown})
    );
    let receipt = crate::display::render_scene(&mut AuditDisplay, &scene).unwrap();
    assert!(receipt.pixels_written > 0);
    scene
}

fn entrance() -> FrontDoor {
    FrontDoor::new(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(3),
        "profile",
        "build",
        "image",
        SourceDocumentId::from("source"),
        CheckedPlotId::from("checked"),
        7,
        true,
    )
}
fn shown(scene: &GraphicsScene, text: &str) -> bool {
    scene
        .commands()
        .iter()
        .any(|command| command.payload().contains(text))
}
fn body(plot: NativePlot) -> FrontDoor {
    let resident = native_workset::resident(plot).unwrap();
    let body = conduit_body::Body::born_with_plots(
        conduit_body::BodyWorkset::one(resident.clone()).unwrap(),
        1,
        conduit_core::SignId::from("born/audit"),
    )
    .unwrap();
    let mut journey = tests::born_projection(body.body_id.clone());
    journey.source_document_id = Some(resident.source_document_id.clone());
    journey.checked_plot_id = Some(resident.checked_plot_id.clone());
    let workspace = WorkspaceProjection {
        body_id: body.body_id,
        revision: journey.revision,
        plots: vec![WorkspacePlot {
            plot: resident,
            title: plot.title(),
            foreground: true,
            input: None,
        }],
    };
    let mut door = entrance();
    door.observe_body(journey, workspace).unwrap();
    door
}
#[test]
fn audit_zero_body_names_face_action_and_native_context_difference() {
    let mut door = entrance();
    let face = door.face().unwrap();
    let scene = scene(&door);
    assert_eq!(face.revision, door.revision());
    assert!(face.basis.body_id.is_none());
    let open = face
        .actions
        .iter()
        .find(|action| action.intent == "conduit.intent/open@1")
        .unwrap();
    assert_eq!(open.name, "Open");
    assert!(shown(&scene, "ENTER OPEN"));
    assert!(shown(&scene, "F9 TOUR"));
    // Captured Face/scene differences are observations, not required gaps.
    let old_revision = door.revision();
    let key = conduit_human::KeyEvent::new(
        40,
        conduit_human::KeyTransition::Pressed,
        conduit_human::KeyModifiers::from_bits(0),
    )
    .unwrap();
    assert_eq!(door.accept(key, old_revision - 1), Err(Error::StaleInput));
    assert_eq!(door.face().unwrap(), face);
}
#[test]
fn audit_home_tiles_are_shared_context_with_exact_current_body_face() {
    let mut door = body(NativePlot::MemoryLantern);
    door.open_home().unwrap();
    let face = door.face().unwrap();
    let scene = scene(&door);
    assert_eq!(face.revision, door.revision());
    assert_eq!(face.basis.body_id, door.journey.as_ref().unwrap().body_id);
    assert!(
        face.subjects
            .iter()
            .any(|subject| subject.identity == "conduitos/shell/home")
    );
    assert!(
        face.properties
            .iter()
            .any(|property| property.name == "view"
                && property.value == FacePropertyValue::Text("launcher".into()))
    );
    for destination in conduit_home_model::HomeDestination::ALL {
        assert!(shown(&scene, destination.label()));
    }
    // Separate shared ApplicationView context exists; no renderer-owned Body.
    door.home_application_view()
        .unwrap()
        .unwrap()
        .validate()
        .unwrap();
}
#[test]
fn audit_creche_shows_shared_birth_actions_without_body_identity() {
    let mut door = entrance();
    door.open_creche("00112233-4455-6677-8899-aabbccddeeff".into(), None)
        .unwrap();
    let face = door.face().unwrap();
    let scene = scene(&door);
    assert_eq!(face.revision, door.revision());
    assert!(face.basis.body_id.is_none());
    assert!(shown(&scene, "Conduit / Crèche"));
    let birth = face
        .actions
        .iter()
        .find(|action| action.intent == "conduit.intent/creche.birth@1")
        .unwrap();
    assert!(shown(&scene, &birth.name));
    assert!(matches!(
        birth.availability,
        FaceActionAvailability::Available
    ));
}
#[test]
fn audit_body_and_patchbay_workspace_label_the_observed_resident_without_diagram_claim() {
    for plot in [NativePlot::MemoryLantern, NativePlot::Patchbay] {
        let door = body(plot);
        let face = door.face().unwrap();
        let scene = scene(&door);
        assert_eq!(face.revision, door.revision());
        assert_eq!(face.basis.body_id, door.journey.as_ref().unwrap().body_id);
        assert!(shown(&scene, "Roseau"));
        assert!(shown(&scene, plot.title()));
        assert!(shown(&scene, "Body lulled"));
        assert!(
            face.actions
                .iter()
                .any(|action| action.intent == "conduit.intent/wake@1"
                    && matches!(action.availability, FaceActionAvailability::Available))
        );
        // An installed Patchbay resident alone is not an observed selected graph.
        assert!(door.patchbay_graph.is_none());
        assert!(
            !face
                .subjects
                .iter()
                .any(|subject| subject.role == FaceRole::Gear)
        );
    }
}

fn home_key(door: &mut FrontDoor, usage: u8) -> HomeInput {
    let event = conduit_human::KeyEvent::new(
        usage,
        conduit_human::KeyTransition::Pressed,
        conduit_human::KeyModifiers::from_bits(0),
    )
    .unwrap();
    door.accept_home(event, door.revision()).unwrap()
}

#[test]
fn home_reading_content_tracks_launcher_plots_and_prompt_without_new_authority() {
    let mut door = body(NativePlot::MemoryLantern);
    let original_actions = door.face().unwrap().actions;
    door.open_home().unwrap();
    let face = door.face().unwrap();
    for destination in conduit_home_model::HomeDestination::ALL {
        assert!(
            face.text
                .iter()
                .any(|text| text.text == destination.label())
        );
    }
    home_key(&mut door, 79);
    home_key(&mut door, 79);
    home_key(&mut door, 40);
    let face = door.face().unwrap();
    for plot in native_workset::inventory() {
        assert!(
            face.text
                .iter()
                .any(|text| text.text == format!("Installed Plot: {}", plot.title()))
        );
    }
    home_key(&mut door, 41);
    // Typing enters the local Prompt; an unavailable command remains visible.
    for usage in [26, 4, 14, 8] {
        home_key(&mut door, usage);
    }
    let face = door.face().unwrap();
    assert!(
        face.properties
            .iter()
            .any(|property| property.name == "command"
                && property.value == FacePropertyValue::Text("wake".into()))
    );
    assert_eq!(home_key(&mut door, 40), HomeInput::Changed);
    let face = door.face().unwrap();
    assert!(
        face.text
            .iter()
            .any(|text| text.text.contains("no lifecycle authority"))
    );
    assert_eq!(face.actions, original_actions);
    assert_eq!(face.basis.body_id, door.journey.as_ref().unwrap().body_id);
}

#[test]
fn resident_face_names_and_describes_the_exact_foreground_plot() {
    for plot in [NativePlot::MemoryLantern, NativePlot::Patchbay] {
        let door = body(plot);
        let face = door.face().unwrap();
        let resident = native_workset::resident(plot).unwrap();
        let subject = face
            .subjects
            .iter()
            .find(|subject| subject.role == FaceRole::Plot)
            .unwrap();
        assert_eq!(subject.name, plot.title());
        assert_eq!(
            subject.identity,
            format!("plot/{}", resident.checked_plot_id.as_str())
        );
        assert!(
            face.text.iter().any(|text| text.subject == subject.identity
                && text.text.contains("Selected resident Plot"))
        );
        assert!(
            !face
                .text
                .iter()
                .any(|text| text.text.contains("OPEN permits inspection only"))
        );
    }
    let face = entrance().face().unwrap();
    assert!(
        face.subjects
            .iter()
            .any(|subject| subject.name == "ConduitOS entrance Plot")
    );
    assert!(
        face.text
            .iter()
            .any(|text| text.text.contains("OPEN permits inspection only"))
    );
}
