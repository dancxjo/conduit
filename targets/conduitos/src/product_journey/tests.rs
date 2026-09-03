use super::*;
use crate::{
    keyboard_offer::KeyboardRealization,
    offer::{CpuFeatures, HostOffer},
};
use conduit_body::WakeLifecycle;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use conduit_presentation::PresentationActionAvailability;

fn fixture() -> (BootIdentities, HostOffer<'static>, ProductJourney) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        1_048_576,
    )
    .with_keyboard(
        KeyboardRealization {
            controller_id: [3; 32],
            device_id: [4; 32],
            interface_id: [5; 32],
            endpoint_id: [6; 32],
            report_buffers: 2,
            transition_slots: 8,
            operation_slots: 2,
        },
        "build",
    )
    .unwrap();
    let journey = ProductJourney::new(
        HostId::from(crate::identity::hex(&identities.host)),
        BootId::from(crate::identity::hex(&identities.boot)),
        OfferGeneration(offer.generation),
    )
    .unwrap();
    (identities, offer, journey)
}

fn front_door(journey: &ProductJourney) -> crate::front_door::FrontDoor {
    crate::front_door::FrontDoor::new(
        journey.host_id.clone(),
        journey.boot_id.clone(),
        journey.offer_generation,
        "profile",
        "build",
        "image",
        journey.form.source_document_id.clone(),
        journey.form.checked_form_id.clone(),
        7,
        true,
    )
}

fn target(journey: &ProductJourney, action: JourneyAction) -> String {
    let projection = journey.projection();
    match action {
        JourneyAction::OpenBack | JourneyAction::Birth => {
            format!("form/{}", projection.checked_form_id.as_str())
        }
        JourneyAction::Wake
        | JourneyAction::Plan
        | JourneyAction::Play
        | JourneyAction::Stop
        | JourneyAction::Lull => format!("body/{}", projection.body_id.unwrap().as_str()),
        _ => panic!("unsupported journey test action"),
    }
}

fn invoke(
    journey: &mut ProductJourney,
    action: JourneyAction,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
) -> Result<(), JourneyError> {
    let revision = journey.revision();
    let request = journey.next_request(action, target(journey, action), revision)?;
    journey.apply(request, identities, offer, "build", revision)
}

fn key(usage: u8, transition: KeyTransition) -> KeyEvent {
    KeyEvent::new(usage, transition, KeyModifiers::from_bits(0)).unwrap()
}

fn assert_current_action(front_door: &crate::front_door::FrontDoor, action: JourneyAction) {
    let semantic = front_door
        .resolve_action(action, front_door.revision())
        .unwrap();
    assert_eq!(semantic.intent, action.presentation_intent());
    assert!(matches!(
        semantic.availability,
        PresentationActionAvailability::Available
    ));
}

fn reach_playing(journey: &mut ProductJourney, identities: &BootIdentities, offer: &HostOffer<'_>) {
    for action in [
        JourneyAction::OpenBack,
        JourneyAction::Birth,
        JourneyAction::Wake,
        JourneyAction::Plan,
        JourneyAction::Play,
    ] {
        invoke(journey, action, identities, offer).unwrap();
    }
    assert_eq!(journey.status(), JourneyStatus::Playing);
}

#[test]
fn exact_seed_birth_wake_plan_play_input_result_and_lull_are_distinct() {
    let (identities, offer, mut journey) = fixture();
    let mut front_door = front_door(&journey);
    let initial = journey.projection();
    invoke(&mut journey, JourneyAction::OpenBack, &identities, &offer).unwrap();
    let opened = journey.projection();
    assert_eq!(opened.status, JourneyStatus::FormOpened);
    assert!(opened.body_id.is_none() && opened.plan_id.is_none());
    assert_eq!(opened.checked_form_id, initial.checked_form_id);
    front_door.observe_journey(opened.clone()).unwrap();
    assert!(front_door.presentation().unwrap().basis.body_id.is_none());
    assert_current_action(&front_door, JourneyAction::Birth);

    invoke(&mut journey, JourneyAction::Birth, &identities, &offer).unwrap();
    let born = journey.projection();
    assert_eq!(born.status, JourneyStatus::BornLulled);
    assert!(born.body_id.is_some() && born.part_id.is_some());
    assert!(born.born_sign_id.is_some());
    assert!(born.wake_id.is_none() && born.plan_id.is_none());
    front_door.observe_journey(born.clone()).unwrap();
    let born_presentation = front_door.presentation().unwrap();
    assert!(born_presentation.basis.body_id.is_none());
    assert_current_action(&front_door, JourneyAction::Wake);
    assert!(
        born_presentation
            .subjects
            .iter()
            .any(|subject| subject.role == conduit_presentation::PresentationRole::Body)
    );
    assert_eq!(
        invoke(&mut journey, JourneyAction::Birth, &identities, &offer),
        Err(JourneyError::AlreadyBorn)
    );

    invoke(&mut journey, JourneyAction::Wake, &identities, &offer).unwrap();
    let awake = journey.projection();
    assert!(awake.wake_id.is_some() && awake.plan_id.is_none());
    front_door.observe_journey(awake.clone()).unwrap();
    assert_eq!(
        front_door.presentation().unwrap().basis.body_id,
        awake.body_id
    );
    assert_current_action(&front_door, JourneyAction::Plan);
    assert_eq!(
        invoke(&mut journey, JourneyAction::Play, &identities, &offer),
        Err(JourneyError::InvalidTransition)
    );
    invoke(&mut journey, JourneyAction::Plan, &identities, &offer).unwrap();
    let planned = journey.projection();
    assert!(planned.plan_id.is_some() && planned.active_play_id.is_none());
    front_door.observe_journey(planned.clone()).unwrap();
    let planned_presentation = front_door.presentation().unwrap();
    assert_eq!(planned_presentation.basis.plan_id, planned.plan_id);
    assert!(planned_presentation.basis.active_play_id.is_none());
    assert_current_action(&front_door, JourneyAction::Play);
    invoke(&mut journey, JourneyAction::Play, &identities, &offer).unwrap();
    let playing = journey.projection();
    assert!(playing.active_play_id.is_some());
    assert_ne!(
        playing.plan_id.as_ref().unwrap().as_str(),
        playing.active_play_id.as_ref().unwrap().as_str()
    );
    front_door.observe_journey(playing.clone()).unwrap();
    assert_eq!(
        front_door.presentation().unwrap().basis.active_play_id,
        playing.active_play_id
    );
    assert_current_action(&front_door, JourneyAction::Stop);
    journey
        .accept_play_input(key(4, KeyTransition::Pressed))
        .unwrap();
    journey
        .accept_play_input(key(4, KeyTransition::Released))
        .unwrap();
    let result = journey.projection();
    assert_eq!(result.status, JourneyStatus::ResultVisible);
    assert_eq!(result.result.as_deref(), Some("A"));
    assert!(result.input_sign_id.is_some() && result.result_sign_id.is_some());
    front_door.observe_journey(result.clone()).unwrap();
    assert!(
        front_door
            .presentation()
            .unwrap()
            .properties
            .iter()
            .any(|property| property.name == "semantic-result")
    );
    let body_id = result.body_id.clone();
    invoke(&mut journey, JourneyAction::Lull, &identities, &offer).unwrap();
    let lulled = journey.projection();
    assert_eq!(lulled.status, JourneyStatus::Lulled);
    assert_eq!(lulled.body_id, body_id);
    assert_eq!(
        journey.wake.as_ref().unwrap().lifecycle,
        WakeLifecycle::Lulled
    );
}

#[test]
fn stale_wrong_and_out_of_order_control_requests_refuse() {
    let (identities, offer, mut journey) = fixture();
    let stale = JourneyRequest {
        request_id: "request/stale".into(),
        presentation_id: "presentation/current".into(),
        presentation_revision: 0,
        action_id: "action/open/current".into(),
        action: JourneyAction::OpenBack,
        target_identity: journey.projection().checked_form_id.as_str().into(),
    };
    assert_eq!(
        journey.apply(stale, &identities, &offer, "build", 1),
        Err(JourneyError::StalePresentation)
    );
    let wrong = JourneyRequest {
        request_id: "request/wrong".into(),
        presentation_id: "presentation/current".into(),
        presentation_revision: 1,
        action_id: "action/open/current".into(),
        action: JourneyAction::OpenBack,
        target_identity: "seed/wrong".into(),
    };
    assert_eq!(
        journey.apply(wrong, &identities, &offer, "build", 1),
        Err(JourneyError::WrongTarget)
    );
    let seed = format!("form/{}", journey.projection().checked_form_id.as_str());
    let born_without_open = JourneyRequest {
        request_id: "request/born".into(),
        presentation_id: "presentation/current".into(),
        presentation_revision: 1,
        action_id: "action/birth/current".into(),
        action: JourneyAction::Birth,
        target_identity: seed,
    };
    assert_eq!(
        journey.apply(born_without_open, &identities, &offer, "build", 1),
        Err(JourneyError::FormNotOpened)
    );
    assert!(journey.body.is_none());

    let wake_without_body = JourneyRequest {
        request_id: "request/wake".into(),
        presentation_id: "presentation/current".into(),
        presentation_revision: 1,
        action_id: "action/wake/current".into(),
        action: JourneyAction::Wake,
        target_identity: "body/absent".into(),
    };
    assert_eq!(
        journey.apply(wake_without_body, &identities, &offer, "build", 1),
        Err(JourneyError::BodyAbsent)
    );
}

#[test]
fn missing_current_keyboard_offer_refuses_plan_before_kernel_admission() {
    let (identities, offer, mut journey) = fixture();
    for action in [
        JourneyAction::OpenBack,
        JourneyAction::Birth,
        JourneyAction::Wake,
    ] {
        invoke(&mut journey, action, &identities, &offer).unwrap();
    }
    let absent = HostOffer::new(
        &identities,
        "build",
        offer.cpu_features,
        offer.runtime_arena_bytes,
    );
    assert_eq!(
        invoke(&mut journey, JourneyAction::Plan, &identities, &absent),
        Err(JourneyError::Plan(PreparationError::PlacementRejected))
    );
    assert!(journey.plan.is_none() && journey.kernel.is_none());
}

#[test]
fn device_loss_and_stop_remove_the_consumer_and_reject_late_values() {
    let (identities, offer, mut journey) = fixture();
    for action in [
        JourneyAction::OpenBack,
        JourneyAction::Birth,
        JourneyAction::Wake,
        JourneyAction::Plan,
    ] {
        invoke(&mut journey, action, &identities, &offer).unwrap();
    }
    journey.input_lost().unwrap();
    assert_eq!(journey.status(), JourneyStatus::Stopped);
    assert!(journey.projection().active_play_id.is_none() && journey.result.is_none());

    let (identities, offer, mut journey) = fixture();
    reach_playing(&mut journey, &identities, &offer);
    journey.input_lost().unwrap();
    assert_eq!(journey.status(), JourneyStatus::Stopped);
    assert!(
        !journey
            .accept_play_input(key(4, KeyTransition::Pressed))
            .unwrap()
    );

    let (identities, offer, mut journey) = fixture();
    reach_playing(&mut journey, &identities, &offer);
    invoke(&mut journey, JourneyAction::Stop, &identities, &offer).unwrap();
    assert!(
        !journey
            .accept_play_input(key(4, KeyTransition::Pressed))
            .unwrap()
    );
    assert!(journey.result.is_none() && journey.result_sign_id.is_none());
}
