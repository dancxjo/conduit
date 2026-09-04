use super::portable_keyboard::*;
use conduit_core::{
    AdmissionUnit, BaseImplementationId, DeliveryPressurePolicy, EvolutionSemantics,
};
use conduit_human::{
    ChordInfo, ConduitIntlKeymap, CoreChordId, KeyEvent, KeymapDisposition,
    KEY_EVENT_CONFORMANCE_VECTORS,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};
use conduit_std_host::hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll};
use std::collections::BTreeMap;
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

fn queued(input: &mut NativeKeyboardInput, code: KeyCode, state: ElementState) -> KeyEvent {
    input
        .observe(PhysicalKey::Code(code), state, false)
        .unwrap()
}

fn consumed(input: &mut NativeKeyboardInput, code: KeyCode, state: ElementState) -> KeyEvent {
    let value = queued(input, code, state);
    assert_eq!(input.next(), Ok(Some(value)));
    value
}

fn text(disposition: KeymapDisposition) -> Option<String> {
    match disposition {
        KeymapDisposition::Text(value) => {
            Some(String::from_utf8(value.as_bytes().to_vec()).unwrap())
        }
        _ => None,
    }
}

#[test]
fn native_physical_events_match_usb_shared_vectors_byte_for_byte() {
    let mut input = NativeKeyboardInput::new();
    let observed = [
        queued(&mut input, KeyCode::KeyA, ElementState::Pressed),
        queued(&mut input, KeyCode::KeyA, ElementState::Released),
        queued(&mut input, KeyCode::ShiftLeft, ElementState::Pressed),
        queued(&mut input, KeyCode::KeyA, ElementState::Pressed),
        queued(&mut input, KeyCode::KeyA, ElementState::Released),
        queued(&mut input, KeyCode::ShiftLeft, ElementState::Released),
        queued(&mut input, KeyCode::KeyA, ElementState::Pressed),
        queued(&mut input, KeyCode::KeyB, ElementState::Pressed),
    ];
    assert_eq!(
        observed.map(KeyEvent::encode),
        KEY_EVENT_CONFORMANCE_VECTORS.map(|vector| vector.encoded)
    );
}

#[test]
fn native_values_reuse_plain_shift_altgr_compose_and_unicode_semantics() {
    let mut input = NativeKeyboardInput::new();
    let mut keymap = ConduitIntlKeymap::new();
    assert_eq!(
        text(keymap.apply(consumed(&mut input, KeyCode::KeyQ, ElementState::Pressed))),
        Some("q".into())
    );
    consumed(&mut input, KeyCode::KeyQ, ElementState::Released);

    consumed(&mut input, KeyCode::ShiftRight, ElementState::Pressed);
    assert_eq!(
        text(keymap.apply(consumed(&mut input, KeyCode::KeyA, ElementState::Pressed))),
        Some("A".into())
    );
    consumed(&mut input, KeyCode::KeyA, ElementState::Released);
    consumed(&mut input, KeyCode::ShiftRight, ElementState::Released);

    consumed(&mut input, KeyCode::AltRight, ElementState::Pressed);
    assert_eq!(
        text(keymap.apply(consumed(&mut input, KeyCode::KeyA, ElementState::Pressed))),
        Some("æ".into())
    );
    consumed(&mut input, KeyCode::KeyA, ElementState::Released);
    consumed(&mut input, KeyCode::AltRight, ElementState::Released);

    keymap.apply(consumed(
        &mut input,
        KeyCode::SuperRight,
        ElementState::Pressed,
    ));
    keymap.apply(consumed(
        &mut input,
        KeyCode::SuperRight,
        ElementState::Released,
    ));
    keymap.apply(consumed(&mut input, KeyCode::Quote, ElementState::Pressed));
    consumed(&mut input, KeyCode::Quote, ElementState::Released);
    assert_eq!(
        text(keymap.apply(consumed(&mut input, KeyCode::KeyE, ElementState::Pressed))),
        Some("é".into())
    );
    consumed(&mut input, KeyCode::KeyE, ElementState::Released);

    keymap.apply(consumed(
        &mut input,
        KeyCode::SuperRight,
        ElementState::Pressed,
    ));
    keymap.apply(consumed(&mut input, KeyCode::KeyU, ElementState::Pressed));
    consumed(&mut input, KeyCode::KeyU, ElementState::Released);
    keymap.apply(consumed(
        &mut input,
        KeyCode::SuperRight,
        ElementState::Released,
    ));
    for code in [
        KeyCode::Digit0,
        KeyCode::Digit3,
        KeyCode::KeyB,
        KeyCode::KeyB,
    ] {
        keymap.apply(consumed(&mut input, code, ElementState::Pressed));
        consumed(&mut input, code, ElementState::Released);
    }
    assert_eq!(
        text(keymap.apply(consumed(&mut input, KeyCode::Enter, ElementState::Pressed))),
        Some("λ".into())
    );
}

#[test]
fn native_values_reuse_all_reviewed_chord_planes_without_stealing_right_modifiers() {
    for (modifier, key, expected) in [
        (
            KeyCode::ControlLeft,
            KeyCode::KeyG,
            CoreChordId::CancelOrEscape,
        ),
        (KeyCode::AltLeft, KeyCode::KeyP, CoreChordId::Palette),
        (KeyCode::SuperLeft, KeyCode::KeyP, CoreChordId::Plan),
    ] {
        let mut input = NativeKeyboardInput::new();
        consumed(&mut input, modifier, ElementState::Pressed);
        let chord = ChordInfo::from_key_event(consumed(&mut input, key, ElementState::Pressed));
        assert_eq!(chord.map(ChordInfo::chord_id), Some(expected));
    }
    let mut input = NativeKeyboardInput::new();
    consumed(&mut input, KeyCode::AltRight, ElementState::Pressed);
    assert!(
        ChordInfo::from_key_event(consumed(&mut input, KeyCode::KeyE, ElementState::Pressed))
            .is_none()
    );
    let mut input = NativeKeyboardInput::new();
    consumed(&mut input, KeyCode::SuperRight, ElementState::Pressed);
    assert!(
        ChordInfo::from_key_event(consumed(&mut input, KeyCode::KeyP, ElementState::Pressed))
            .is_none()
    );
}

#[test]
fn unchanged_k6_form_plans_to_truthful_native_realization_without_usb_facts() {
    let composition = conduit_std_host::StdHostComposition::minimal()
        .with_text()
        .with_input();
    let host = conduit_std_host::StdHost::new_with_composition(
        conduit_std_host::StdHostConfig {
            host_id: conduit_core::HostId::from("patchbay-native/conformance"),
            boot_id: conduit_core::BootId::from("patchbay-native/boot-1"),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        composition,
    );
    let mut advertisement = host.advertisement().clone();
    append_offer(&mut advertisement).unwrap();
    let model = patchbay_model::PatchbayModel::from_advertisement(advertisement);
    let source = conduitos::keyboard_text_plan::FORM_SOURCE;
    let syntax = conduit_form::parse_syntax_document(source);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let form = conduit_form::expand_canonical_form(&checked, "conduitos-keyboard-upper", &profile)
        .unwrap();
    let hosts = [model.advertisement().clone()];
    let placements = default_expanded_placements(&form, &hosts).unwrap();
    let plan = plan_expanded_canonical_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::KEYBOARD_MAX_QUEUE_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 4);
    let keyboard = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::KEYBOARD_KIND)
        .unwrap();
    assert_eq!(
        keyboard.implementation_id.as_str(),
        NATIVE_KEYBOARD_IMPLEMENTATION
    );
    assert!(keyboard
        .resources
        .iter()
        .any(|binding| binding.class_id.as_str() == WINDOW_INPUT_RESOURCE));
    assert!(fragment.connections.iter().all(|cord| {
        cord.item_capacity == 1
            && (cord.value_kind.as_str() != conduit_human::KEY_EVENT_INFO_ID
                || cord.byte_capacity >= conduit_human::KEY_EVENT_ENCODED_LEN as u32)
    }));
    let encoded = serde_json::to_string(model.advertisement()).unwrap();
    assert!(!encoded.contains("xhci"));
    assert!(!encoded.contains("usb"));
    assert!(!encoded.contains("hid"));
    assert_eq!(form.source_document_id, checked.source_document_id);
}

#[test]
fn pressure_focus_cancellation_closure_and_mapping_refusals_are_distinct() {
    assert_eq!(
        conduit_human::KEY_EVENT_DELIVERY_CONTRACT,
        conduit_semantic_catalog::reviewed_delivery_contract(&conduit_core::KindId::from(
            conduit_human::KEY_EVENT_INFO_ID,
        ))
        .unwrap()
    );
    assert_eq!(
        conduit_human::KEY_EVENT_DELIVERY_CONTRACT.pressure_policy,
        DeliveryPressurePolicy::PreserveOrder
    );
    assert_eq!(
        conduit_human::KEY_EVENT_DELIVERY_CONTRACT.evolution,
        EvolutionSemantics::Occurrence
    );
    assert_eq!(
        conduit_human::KEY_EVENT_DELIVERY_CONTRACT.admission_unit,
        AdmissionUnit::Value
    );
    let mut pressure = NativeKeyboardInput::new();
    for code in [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
    ] {
        queued(&mut pressure, code, ElementState::Pressed);
    }
    assert_eq!(
        pressure.observe(
            PhysicalKey::Code(KeyCode::KeyI),
            ElementState::Pressed,
            false
        ),
        Err(NativeKeyboardFailure::QueuePressure)
    );
    assert_eq!(pressure.next(), Err(NativeKeyboardFailure::QueuePressure));

    let mut focus = NativeKeyboardInput::new();
    focus.focus_lost();
    assert_eq!(focus.next(), Err(NativeKeyboardFailure::FocusLost));
    let mut cancelled = NativeKeyboardInput::new();
    cancelled.cancel();
    assert_eq!(cancelled.next(), Err(NativeKeyboardFailure::Cancelled));
    let mut closed = NativeKeyboardInput::new();
    closed.close();
    assert_eq!(closed.next(), Err(NativeKeyboardFailure::Closed));
    let mut unknown = NativeKeyboardInput::new();
    assert_eq!(
        unknown.observe(
            PhysicalKey::Code(KeyCode::F24),
            ElementState::Pressed,
            false
        ),
        Err(NativeKeyboardFailure::UnsupportedPhysicalKey)
    );
    assert_eq!(
        unknown.observe(
            PhysicalKey::Code(KeyCode::KeyA),
            ElementState::Pressed,
            true
        ),
        Err(NativeKeyboardFailure::RepeatedPlatformEvent)
    );
}

#[test]
fn focus_regain_reopens_only_focus_loss_and_discards_stale_input_state() {
    let mut keyboard = NativeKeyboardInput::new();
    let mut reader = keyboard.reader();
    keyboard
        .observe(
            PhysicalKey::Code(KeyCode::KeyA),
            ElementState::Pressed,
            false,
        )
        .unwrap();

    keyboard.focus_lost();
    assert_eq!(
        reader.poll_next(),
        HostedKeyboardPoll::Failed(NativeKeyboardFailure::FocusLost as u16)
    );

    keyboard.focus_gained();
    assert_eq!(reader.poll_next(), HostedKeyboardPoll::Pending);
    keyboard
        .observe(
            PhysicalKey::Code(KeyCode::KeyA),
            ElementState::Pressed,
            false,
        )
        .unwrap();
    assert!(matches!(reader.poll_next(), HostedKeyboardPoll::Event(_)));

    let mut pressure = NativeKeyboardInput::new();
    for code in [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
    ] {
        pressure
            .observe(PhysicalKey::Code(code), ElementState::Pressed, false)
            .unwrap();
    }
    assert_eq!(
        pressure.observe(
            PhysicalKey::Code(KeyCode::KeyI),
            ElementState::Pressed,
            false,
        ),
        Err(NativeKeyboardFailure::QueuePressure)
    );
    pressure.focus_gained();
    assert_eq!(
        pressure.observe(
            PhysicalKey::Code(KeyCode::KeyJ),
            ElementState::Pressed,
            false,
        ),
        Err(NativeKeyboardFailure::QueuePressure)
    );
}
