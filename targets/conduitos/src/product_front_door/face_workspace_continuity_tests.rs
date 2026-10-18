use super::tests::encounter;
use super::*;
use crate::{
    native_compositor::InputRoute,
    native_workset::{self, NativePlot},
    product_journey::test_support::fixture,
};

#[test]
fn home_keyboard_binding_crosses_acknowledged_show_and_reentry_keeps_the_body() {
    let (mut journey, mut door, mut workspace, mut display) = encounter();
    workspace.relinquish().unwrap();
    // Home's existing fixed layout is qualified at 1024x768, like the audit.
    display.1 = true;
    door.open_home().unwrap();
    let original = journey.projection();
    let provider = crate::product_bases::fixture_surface_provider();
    let mut presenter = FrontDoorPresenter::prepare(
        original.host_id.clone(),
        original.boot_id.clone(),
        original.offer_generation,
        "profile",
        "image",
        provider.entry.base_id.clone(),
        2,
    )
    .unwrap();
    presenter.present(&door, &mut display).unwrap();
    let (ids, offer, _) = fixture();
    let make = MakeRecord {
        build_id: "build",
        ..MakeRecord::legacy()
    };
    // The same keyboard resolver used by the installed product loop.
    for usage in [64, 61] {
        let action =
            crate::product_front_door::input_actions::action_for(usage, &door, &journey).unwrap();
        workspace
            .invoke_from_presenter(
                action,
                &mut door,
                &mut journey,
                &mut presenter,
                &mut display,
                &ids,
                &offer,
                &make,
            )
            .unwrap();
        assert!(door.home_open());
        assert!(!workspace.active());
        assert!(workspace.selected_show.is_none());
        assert!(matches!(
            presenter.route_keyboard().unwrap(),
            InputRoute::Delivered(_)
        ));
        assert_eq!(journey.projection().body_id, original.body_id);
        assert_eq!(journey.projection().born_sign_id, original.born_sign_id);
    }
    assert_eq!(
        journey.status(),
        crate::product_journey::JourneyStatus::Awake
    );
    let awake = journey.projection();
    // An unavailable repeated Wake is a recoverable, durable refusal.
    workspace
        .invoke_from_presenter(
            patchbay_control::PatchbayAction::Wake,
            &mut door,
            &mut journey,
            &mut presenter,
            &mut display,
            &ids,
            &offer,
            &make,
        )
        .unwrap();
    assert_eq!(journey.projection(), awake);
    assert!(
        door.face()
            .unwrap()
            .properties
            .iter()
            .any(|property| property.name == "action-refusal"
                && matches!(&property.value, conduit_face::FacePropertyValue::Text(text) if text.contains("Wake is not available")))
    );
    assert!(
        door.scene(&display)
            .unwrap()
            .commands()
            .iter()
            .any(|command| command.payload().contains("Wake is not available"))
    );
    door.observe_product(&journey).unwrap();
    assert!(
        door.face()
            .unwrap()
            .properties
            .iter()
            .any(|property| property.name == "action-refusal")
    );
    presenter.present(&door, &mut display).unwrap();
    workspace
        .invoke_from_presenter(
            patchbay_control::PatchbayAction::Plan,
            &mut door,
            &mut journey,
            &mut presenter,
            &mut display,
            &ids,
            &offer,
            &make,
        )
        .unwrap();
    assert!(
        !door
            .face()
            .unwrap()
            .properties
            .iter()
            .any(|property| property.name == "action-refusal")
    );
    let awake = journey.projection();
    door.close_home().unwrap();
    let patchbay = native_workset::resident(NativePlot::Patchbay).unwrap();
    journey.select_plot(&patchbay, journey.revision()).unwrap();
    door.observe_product(&journey).unwrap();
    workspace
        .enter(&door, &mut presenter, &mut display)
        .unwrap();
    workspace
        .leave(&door, &mut presenter, &mut display)
        .unwrap();
    door.open_home().unwrap();
    presenter.present(&door, &mut display).unwrap();
    door.close_home().unwrap();
    workspace
        .enter(&door, &mut presenter, &mut display)
        .unwrap();
    assert_eq!(journey.projection().body_id, original.body_id);
    assert_eq!(journey.projection().born_sign_id, original.born_sign_id);
    assert_eq!(journey.projection().active_play_id, awake.active_play_id);
    assert_eq!(journey.status(), awake.status);
}
