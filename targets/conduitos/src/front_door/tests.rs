use super::*;
use crate::display::PixelTarget;
use crate::product_journey::{JourneyProjection, JourneyStatus};

fn door() -> FrontDoor {
    FrontDoor::new(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(3),
        "profile:one",
        "build:one",
        "image:one",
        SourceDocumentId::from("source"),
        CheckedPlotId::from("checked"),
        7,
        false,
    )
}

fn key(usage: u8) -> KeyEvent {
    KeyEvent::new(
        usage,
        conduit_human::KeyTransition::Pressed,
        conduit_human::KeyModifiers::from_bits(0),
    )
    .unwrap()
}

pub(super) fn born_projection(body_id: conduit_body::BodyId) -> JourneyProjection {
    JourneyProjection {
        status: JourneyStatus::Lulled,
        revision: 9,
        source_document_id: Some(SourceDocumentId::from("source")),
        checked_plot_id: Some(CheckedPlotId::from("checked")),
        expanded_plot_id: Some(conduit_core::ExpandedPlotId::from("expanded")),
        host_id: HostId::from("host"),
        boot_id: BootId::from("boot"),
        offer_generation: OfferGeneration(3),
        body_id: Some(body_id),
        friendly_name: Some("Roseau".into()),
        born_sign_id: None,
        fulfilled_sign_id: None,
        workload_revision: Some(0),
        workload_sign_id: None,
        lull_sign_id: None,
        workload_capacity_available: false,
        part_id: None,
        wake_id: None,
        wake_sign_id: None,
        plan_id: None,
        partition_plan_id: None,
        plan_sign_id: None,
        active_play_id: None,
        play_sign_id: None,
        gear_ids: vec![],
        port_ids: vec![],
        cord_ids: vec![],
        input_sign_id: None,
        loss_kind: None,
        loss_sign_id: None,
        result_sign_id: None,
        result: None,
        result_omitted_bytes: 0,
        input_count: 0,
        kernel_sign_gap: None,
        last_request_id: None,
        mask: None,
    }
}

fn body_id(sequence: u64) -> conduit_body::BodyId {
    conduit_body::Body::born(
        SourceDocumentId::from("source"),
        CheckedPlotId::from("checked"),
        sequence,
        conduit_core::SignId::from("sign/born"),
    )
    .unwrap()
    .body_id
}

pub(super) struct Sink;

impl PixelTarget for Sink {
    fn format(&self) -> crate::display::DisplayFormat {
        crate::display::DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2_560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }

    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        Ok(())
    }
}

#[test]
fn entrance_is_a_zero_body_portable_presentation_and_finite_scene() {
    let door = door();
    let presentation = door.presentation().unwrap();
    assert!(presentation.basis.body_id.is_none());
    assert!(presentation.basis.plan_id.is_none());
    assert_eq!(presentation.subjects[1].role, PresentationRole::Plot);
    assert_eq!(presentation.actions.len(), 2);
    assert!(
        presentation
            .actions
            .iter()
            .any(|action| action.intent == "conduit.intent/open@1")
    );
    assert!(presentation.actions.iter().any(|action| {
        action.intent == "conduit.intent/birth@1"
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Unavailable { .. }
            )
    }));
    let scene = door.scene(&Sink).unwrap();
    let receipt = crate::display::render_scene(&mut Sink, &scene).unwrap();
    assert_eq!(receipt.commands, 8);
    assert!(receipt.pixels_written > 0);
}

#[test]
fn open_is_inert_and_details_are_progressive() {
    let mut door = door();
    assert!(!door.plot_open);
    assert!(!door.selected_subject.starts_with("plot/"));
    assert!(door.accept(key(TAB), 1).unwrap());
    assert!(door.accept(key(ENTER), 2).unwrap());
    assert!(door.plot_open);
    assert_eq!(door.revision(), 3);
    assert!(door.presentation().unwrap().basis.body_id.is_none());
    assert!(door.accept(key(F2), 3).unwrap());
    let details = door.presentation().unwrap();
    assert!(door.exact_details_open());
    assert!(
        details
            .properties
            .iter()
            .any(|property| property.name == "profile-id")
    );
    assert!(details.basis.body_id.is_none());
}

#[test]
fn stale_input_and_capacity_are_explicit_without_body_transition() {
    let mut door = door();
    assert_eq!(door.accept(key(ENTER), 0), Err(Error::StaleInput));
    assert_eq!(
        door.resolve_action(patchbay_control::PatchbayAction::OpenBack, 0),
        Err(Error::StaleAction)
    );
    assert_eq!(
        door.resolve_action(patchbay_control::PatchbayAction::Birth, door.revision()),
        Err(Error::ActionUnavailable)
    );
    door.revision = u64::MAX;
    assert_eq!(door.accept(key(ENTER), u64::MAX), Err(Error::Presentation));
    assert!(door.presentation().unwrap().basis.body_id.is_none());
}

#[test]
fn bounded_lifecycle_surface_resolves_revision_and_fulfillment_transitions() {
    let mut door = door();
    let mut projection = born_projection(body_id(2));
    projection.status = JourneyStatus::QuiescentAwaitingInput;
    projection.workload_capacity_available = true;
    door.observe_journey(projection.clone()).unwrap();
    assert_eq!(door.presentation().unwrap().actions.len(), 8);
    assert!(
        door.resolve_action(patchbay_control::PatchbayAction::AdmitPlot, door.revision())
            .is_ok()
    );

    projection.status = JourneyStatus::Lulled;
    projection.workload_capacity_available = false;
    door.observe_journey(projection).unwrap();
    assert_eq!(door.presentation().unwrap().actions.len(), 8);
    assert!(
        door.resolve_action(patchbay_control::PatchbayAction::Fulfill, door.revision())
            .is_ok()
    );
}

#[test]
fn releases_and_unrelated_keys_do_not_act() {
    let mut door = door();
    let release = KeyEvent::new(
        ENTER,
        conduit_human::KeyTransition::Released,
        conduit_human::KeyModifiers::from_bits(0),
    )
    .unwrap();
    assert!(!door.accept(release, 1).unwrap());
    assert!(!door.accept(key(4), 1).unwrap());
}

#[test]
fn connectivity_is_projected_through_the_current_body_presentation() {
    let mut door = door();
    let body_id = body_id(1);
    door.observe_journey(born_projection(body_id.clone()))
        .unwrap();
    door.observe_connectivity(ConnectivityProjection {
        line_id: "line:usb:one".into(),
        status: ConnectivityStatus::ValueVisible,
        value: Some("HELLO USB LINE".into()),
        body_id: body_id.clone(),
    })
    .unwrap();

    let presentation = door.presentation().unwrap();
    assert!(presentation.subjects.iter().any(|subject| {
        subject.identity == "line:usb:one" && subject.role == PresentationRole::Line
    }));
    assert!(presentation.relationships.iter().any(|relationship| {
        relationship.target == "line:usb:one"
            && relationship.kind == PresentationRelationshipKind::Connects
    }));
    assert!(presentation.properties.iter().any(|property| {
        property.subject == "line:usb:one"
            && property.name == "received-value"
            && property.value == PresentationPropertyValue::Text("HELLO USB LINE".into())
    }));
}

#[test]
fn connectivity_refuses_an_absent_or_different_body() {
    let mut door = door();
    let line = ConnectivityProjection {
        line_id: "line:usb:one".into(),
        status: ConnectivityStatus::Current,
        value: None,
        body_id: body_id(1),
    };
    assert_eq!(
        door.observe_connectivity(line.clone()),
        Err(Error::Presentation)
    );

    door.observe_journey(born_projection(body_id(2))).unwrap();
    assert_eq!(door.observe_connectivity(line), Err(Error::Presentation));
}

#[test]
fn resting_and_fulfilled_faces_keep_body_identity_without_live_execution() {
    let mut door = door();
    let body = body_id(3);
    let mut projection = born_projection(body.clone());
    for status in [JourneyStatus::Lulled, JourneyStatus::Fulfilled] {
        projection.status = status;
        door.observe_journey(projection.clone()).unwrap();
        let face = door.presentation().unwrap();
        face.validate().unwrap();
        assert_eq!(face.basis.body_id.as_ref(), Some(&body));
        assert!(face.properties.iter().any(|property| {
            property.name == "current-body"
                && property.value == PresentationPropertyValue::Identity(body.as_str().into())
        }));
        assert!(face.basis.wake_id.is_none());
        assert!(face.basis.plan_id.is_none());
        assert!(face.basis.active_play_id.is_none());
    }
}

#[test]
fn selected_patchbay_graph_becomes_exact_face_subjects_for_the_native_mask() {
    use conduit_core::{
        GearId, KindIdentity, PortDescriptor, PortDirection, PortTemporal, kind_id, port_id,
    };
    use patchbay_graph::{PatchbayCord, PatchbayGear, PatchbayGraph, PatchbayPort};

    let port = |identity: &str, gear: &str, direction| PatchbayPort {
        identity: identity.into(),
        gear_id: GearId::from(gear),
        descriptor: PortDescriptor {
            port_id: port_id(identity),
            value_kind: kind_id("text/value"),
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        },
        value_contract: None,
    };
    let gear = |identity: &str, direction, port_identity: &str| PatchbayGear {
        identity: identity.into(),
        gear_id: GearId::from(identity),
        kind_id: kind_id("text/transform"),
        kind_contract_revision: KindIdentity::from("text/transform@1"),
        source_plot: "example".into(),
        plot_path: vec!["example".into()],
        inputs: if direction == PortDirection::Input {
            vec![port(port_identity, identity, direction)]
        } else {
            vec![]
        },
        outputs: if direction == PortDirection::Output {
            vec![port(port_identity, identity, direction)]
        } else {
            vec![]
        },
        controls: vec![],
    };
    let mut door = door();
    let body = body_id(9);
    door.observe_journey(born_projection(body)).unwrap();
    door.patchbay_graph = Some(PatchbayGraph {
        source_document_id: SourceDocumentId::from("source/graph"),
        checked_plot_id: CheckedPlotId::from("checked/graph"),
        expanded_plot_id: conduit_core::ExpandedPlotId::from("expanded/graph"),
        plot_name: "Connected example".into(),
        front_inputs: vec![],
        front_outputs: vec![],
        compositions: vec![],
        gears: vec![
            gear("source", PortDirection::Output, "source/out"),
            gear("sink", PortDirection::Input, "sink/in"),
        ],
        cords: vec![PatchbayCord {
            identity: "source-to-sink".into(),
            source_port: "source/out".into(),
            sink_port: "sink/in".into(),
            value_kind: kind_id("text/value"),
            temporal: PortTemporal::Value,
        }],
    });
    let face = door.presentation().unwrap();
    #[cfg(feature = "native-compositor")]
    {
        let scene =
            crate::native_face_scene::NativeFaceScene::prepare(face.clone(), 800, 600).unwrap();
        assert!(scene.has_diagram());
    }
    assert!(face.relationships.iter().any(|relation| {
        relation.source == "cord/source-to-sink"
            && relation.target == "port/sink/in"
            && relation.kind == PresentationRelationshipKind::Connects
    }));
    assert!(face.properties.iter().any(|property| {
        property.subject == "port/source/out"
            && property.name == "direction"
            && property.value == PresentationPropertyValue::Text("outgoing".into())
    }));
}
