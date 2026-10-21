//! Execute actual local preference preparation/publication, not a scene mockup.
use super::*;
use conduit_human::{KeyModifiers, KeyTransition};
fn key(usage: u8) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
}

#[test]
fn local_selection_keeps_face_and_draft_while_retiring_each_old_show() {
    let provider = crate::product_bases::fixture_surface_provider();
    let mut arrival = FaceArrival::prepare(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "build",
        provider.entry.base_id.clone(),
        &provider,
        None,
    )
    .unwrap();
    let mut door = FrontDoor::new(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "profile",
        "build",
        "image",
        "source".into(),
        "checked".into(),
        7,
        true,
    );
    door.open_creche("00112233-4455-6677-8899-aabbccddeeff".into(), None)
        .unwrap();
    let mut display = super::super::tests::Display;
    arrival.present_first(&door, &mut display).unwrap();
    let face = door.creche_face(&arrival.basis).unwrap();
    let component = arrival
        .mask
        .component_configuration()
        .unwrap()
        .component
        .clone();
    let recipe = arrival
        .mask
        .component_configuration()
        .unwrap()
        .recipe
        .clone();
    let focus = arrival.mask.scene().unwrap().focused();
    let interaction_sequence = arrival.next_interaction;
    arrival
        .mask
        .key(key(4), interaction_sequence, &mut display)
        .unwrap();
    // An unsubmitted 'a' survives four independent palette/density transitions.
    for (usage, density, palette) in [
        (64, VisualDensity::Compact, VisualPalette::Editorial),
        (63, VisualDensity::Spacious, VisualPalette::Editorial),
        (64, VisualDensity::Spacious, VisualPalette::Phosphor),
        (63, VisualDensity::Compact, VisualPalette::Phosphor),
    ] {
        let prior_show = arrival.mask.show().unwrap().clone();
        assert!(
            arrival
                .select_local_look(key(usage), &door, &mut display)
                .unwrap()
        );
        assert_eq!(arrival.mask.scene().unwrap().face(), &face);
        assert_eq!(door.creche_face(&arrival.basis).unwrap(), face);
        assert_eq!(arrival.mask.scene().unwrap().focused(), focus);
        assert_ne!(arrival.mask.show().unwrap().show_id, prior_show.show_id);
        assert_ne!(
            arrival.mask.show().unwrap().show.plan_id,
            prior_show.show.plan_id
        );
        assert_eq!(
            arrival.mask.component_configuration().unwrap().component,
            component
        );
        assert_eq!(
            arrival.mask.component_configuration().unwrap().recipe,
            recipe
        );
        let expression = &arrival.mask.visual_expression().unwrap().expression;
        assert_eq!((expression.density, expression.palette), (density, palette));
        assert_eq!(arrival.next_interaction, interaction_sequence);
        let frame = arrival.mask.scene().unwrap().frame().unwrap();
        assert!(frame.scene.commands().iter().any(|command| command.kind
            == conduit_face::GraphicsCommandKind::Text
            && command.payload().contains('a')
            && command.payload().starts_with("Draft")));
    }
    let submitted_show = arrival.mask.show().unwrap().show_id.clone();
    let input = arrival
        .mask
        .key(key(40), interaction_sequence + 1, &mut display)
        .unwrap();
    let crate::native_face_mask::NativeFaceMaskInput::Submitted { correlation, .. } = input else {
        panic!("retained draft must submit")
    };
    assert_eq!(correlation.interaction.action_id, "creche.name");
    assert_eq!(correlation.interaction.arguments[0].value, b"a");
    assert_eq!(correlation.interaction.show_id, submitted_show.as_str());
}
