use super::*;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

fn drive(session: &mut TourSession, event: Option<KeyEvent>) -> Vec<String> {
    let play = session.active_play_id.as_str().to_owned();
    let mut input = event;
    let mut output = Vec::new();
    for _ in 0..crate::installed_browser::BROWSER_ROUTE_SLOTS {
        let _ = session.poll_effect().unwrap();
        let candidate = session.pending.iter().position(|pending| {
            matches!(pending.effect, engine::BrowserHostEffect::Manifestation(_))
                || (input.is_some()
                    && matches!(pending.effect, engine::BrowserHostEffect::KeyEvent))
        });
        let Some(index) = candidate else { continue };
        let request = session.pending[index].request;
        let placement = session.fragments[0].placements[usize::from(request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        match session.project_pending_effect(index).unwrap() {
            TourHostEffect::KeyEvent(_) => {
                let bytes = input.take().unwrap().encode();
                session
                    .complete_effect(&play, &placement, request.request.0, Some(&bytes))
                    .unwrap();
            }
            TourHostEffect::Manifestation(effect) => {
                let expected_caret = conduit_semantic_catalog::text_face_insertion_caret(
                    &session.fragments[0].placements[usize::from(request.node.0)].configuration,
                )
                .then_some(true);
                assert_eq!(effect.insertion_caret, expected_caret);
                if let Some(text) = effect.text {
                    output.push(text);
                }
                session
                    .complete_effect(&play, &placement, request.request.0, None)
                    .unwrap();
            }
            _ => unreachable!(),
        }
    }
    assert!(
        input.is_none(),
        "standing keyboard accepts every transition"
    );
    assert_eq!(session.active_play_id.as_str(), play);
    output
}
fn press(usage: u8, bits: u8) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::from_bits(bits)).unwrap()
}

#[test]
fn default_authored_layout_dead_keys_reach_retained_browser_text() {
    let (mut session, _) = TourSession::prepare(
        "browser/dead-key-proof",
        "boot/dead-key-proof",
        include_str!("../../../../../plots/memory-lantern/main.conduit"),
        71,
    )
    .unwrap();
    assert_eq!(drive(&mut session, None), [""]);
    select_language(&mut session, "us-intl");
    assert!(drive(&mut session, Some(press(0x34, 0x40))).is_empty());
    assert_eq!(drive(&mut session, Some(press(0x08, 0))), ["é"]);
    assert!(drive(&mut session, Some(press(0x35, 0x42))).is_empty());
    assert_eq!(drive(&mut session, Some(press(0x11, 0x02))), ["éÑ"]);
    assert_eq!(drive(&mut session, Some(press(0x2a, 0))), ["é"]);
    assert_eq!(drive(&mut session, Some(press(0x34, 0))), ["é'"]);
    assert!(drive(&mut session, Some(press(0x34, 0x40))).is_empty());
    assert_eq!(drive(&mut session, Some(press(0x1e, 0))), ["é'´1"]);
}

#[test]
fn source_composes_reusable_layers_and_passes_exact_layout_to_browser_keymap() {
    let source = r#"plot layered_keys {
        base = [{usage:4,shift:false,altgr:false,action:text("a")},{usage:5,shift:false,altgr:false,action:text("b")}]
        accents = [{usage:4,shift:false,altgr:false,action:text("λ")},{usage:6,shift:false,altgr:false,action:text("abcdefgh")},{usage:52,shift:false,altgr:true,action:dead({spacing:"´",combining:769})}]
        layout = {layers:[base,accents],compositions:[]}
        keyboard: input/keyboard
        map: input/keymap(layout=layout)
        message: text/edit(maximum-bytes=256)
        show: face/text("Layered")
        keyboard.key >> map.key
        map.text >> message.fragment
        message.current >> show.text
    }"#;
    let (mut session, _) = TourSession::prepare(
        "browser/authored-layer-proof",
        "boot/authored-layer-proof",
        source,
        72,
    )
    .unwrap();
    assert_eq!(drive(&mut session, None), [""]);
    assert_eq!(drive(&mut session, Some(press(4, 0))), ["λ"]);
    assert_eq!(drive(&mut session, Some(press(5, 0))), ["λb"]);
    assert!(drive(&mut session, Some(press(52, 0x40))).is_empty());
    assert_eq!(drive(&mut session, Some(press(4, 0))), ["λb´λ"]);
    assert_eq!(drive(&mut session, Some(press(6, 0))), ["λb´λabcdefgh"]);
}

fn select_language(session: &mut TourSession, identity: &str) {
    for _ in 0..crate::installed_browser::BROWSER_ROUTE_SLOTS {
        let _ = session.poll_effect().unwrap();
        let pending = (0..session.pending.len()).find_map(|index| {
            match session.project_pending_effect(index).unwrap() {
                TourHostEffect::ApplicationEvent(effect) if effect.choice.is_some() => Some(index),
                _ => None,
            }
        });
        if let Some(index) = pending {
            let request = session.pending[index].request;
            let placement = session.fragments[0].placements[usize::from(request.node.0)]
                .placement_id
                .as_str()
                .to_owned();
            let play = session.active_play_id.as_str().to_owned();
            session
                .complete_effect(
                    &play,
                    &placement,
                    request.request.0,
                    Some(identity.as_bytes()),
                )
                .unwrap();
            return;
        }
    }
    panic!("saved choice source remains armed");
}

#[test]
fn saved_language_identity_switches_actual_layout_without_restarting_the_message() {
    let (mut session, _) = TourSession::prepare(
        "browser/layout-switch-proof",
        "boot/layout-switch-proof",
        include_str!("../../../../../plots/memory-lantern/main.conduit"),
        73,
    )
    .unwrap();
    assert_eq!(drive(&mut session, None), [""]);
    select_language(&mut session, "german");
    assert_eq!(drive(&mut session, Some(press(0x1c, 0))), ["z"]);
    select_language(&mut session, "french");
    assert_eq!(drive(&mut session, Some(press(0x04, 0))), ["zq"]);
    select_language(&mut session, "us-intl");
    assert!(drive(&mut session, Some(press(0x34, 0x40))).is_empty());
    select_language(&mut session, "us");
    assert_eq!(drive(&mut session, Some(press(0x04, 0))), ["zqa"]);
}

#[test]
fn reusable_plot_forwards_typed_layout_and_keeps_instances_pending_state_independent() {
    let source = |base: &str| {
        format!(
            r#"plot wrapped_keys {{
        base = [{{usage:8,shift:false,altgr:false,action:text("{base}")}}]
        overlay = [{{usage:52,shift:false,altgr:true,action:dead({{spacing:"´",combining:769}})}}]
        layout = {{layers:[base,overlay],compositions:[]}}
        editor: reusable_editor(layout=layout)
    }}
    plot reusable_editor (
        layout: KeymapLayout
    ) {{
        keyboard: input/keyboard
        map: input/keymap(layout=layout)
        message: text/edit(maximum-bytes=256)
        show: face/text("Wrapped layout")
        keyboard.key >> map.key
        map.text >> message.fragment
        message.current >> show.text
    }}"#
        )
    };
    let (mut first, _) = TourSession::prepare(
        "browser/reusable-layout-first",
        "boot/reusable-layout-first",
        &source("e"),
        74,
    )
    .unwrap();
    let (mut second, _) = TourSession::prepare(
        "browser/reusable-layout-second",
        "boot/reusable-layout-second",
        &source("a"),
        75,
    )
    .unwrap();
    assert_eq!(drive(&mut first, None), [""]);
    assert_eq!(drive(&mut second, None), [""]);
    assert!(drive(&mut first, Some(press(52, 0x40))).is_empty());
    assert_eq!(drive(&mut second, Some(press(8, 0))), ["a"]);
    assert!(drive(&mut second, Some(press(52, 0x40))).is_empty());
    assert_eq!(drive(&mut first, Some(press(8, 0))), ["é"]);
    assert_eq!(drive(&mut second, Some(press(8, 0))), ["aá"]);
}
