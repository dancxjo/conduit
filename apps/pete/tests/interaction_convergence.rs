use conduit_core::{
    HumanInteractionProposal, InfoBool, InteractionProposalPayload, Quantity, QuantityUnit,
    BOOL_INFO_ID, TEXT_INFO_ID,
};
use conduit_pete::{
    physical_control_surface_projection, quantity, value, InteractionConvergenceApplication,
    PhysicalEvent, PhysicalInput, PhysicalInteractionFailure, PhysicalResourceStatus,
    PicoInteractionSurface, PresenterSource, BROWSER_IMPLEMENTATION_ID, CONTROL_SURFACE_BODY_ID,
    CONTROL_SURFACE_FORM, CONTROL_SURFACE_PLAN_ID, CONTROL_SURFACE_PLAY_ID,
};

fn browser() -> PresenterSource {
    PresenterSource::Browser {
        host_id: "host/browser/chromium".into(),
        boot_id: "boot/browser/chromium/1".into(),
        manifestation_id: "manifestation/browser/instrument/1".into(),
    }
}

fn physical(resource_id: &str, mapping_identity: Option<String>) -> PresenterSource {
    PresenterSource::Physical {
        host_id: "host/pico-w/control-surface".into(),
        boot_id: "boot/pico-w/control-surface/1".into(),
        manifestation_id: "manifestation/ssd1306/instrument/1".into(),
        resource_id: resource_id.into(),
        mapping_identity,
        recursive_composition_identity: "form/control-bank/four-momentary-to-waveform@1".into(),
    }
}

fn proposal(
    contract: &conduit_core::InteractionContract,
    state: &conduit_core::InteractionCurrentState,
    sequence: u64,
    payload: InteractionProposalPayload,
) -> HumanInteractionProposal {
    HumanInteractionProposal::new(contract, state, sequence, payload).unwrap()
}

fn physical_event(
    resource_id: &str,
    generation: u64,
    sequence: u64,
    input: PhysicalInput,
) -> PhysicalEvent {
    PhysicalEvent {
        plan_id: CONTROL_SURFACE_PLAN_ID.into(),
        resource_id: resource_id.into(),
        resource_generation: generation,
        sequence,
        transitions_in_window: 1,
        input,
    }
}

#[test]
fn unchanged_checked_form_and_two_materially_different_presenters_share_meaning() {
    let app = InteractionConvergenceApplication::new().unwrap();
    assert_eq!(
        app.source_document_id().as_str(),
        "cb0dcd832852f396cb3ea376beb888a6b9991660d98de65da3f06bc9c0040693"
    );
    assert_eq!(
        app.checked_form_id().as_str(),
        "ebffdd1f07a8fe360ea35f948ff1f63ecd3bc816d8aa1322b96dfa0162410d60"
    );
    assert_eq!(
        app.expanded_form_id().as_str(),
        "3887b339771251500403daa481137280964bbcdb86a98fdce76b655f0631e681"
    );
    for forbidden in ["dom", "gpio", "widget", "device", "chromium", "pico"] {
        assert!(!CONTROL_SURFACE_FORM.contains(forbidden));
    }
    assert_ne!(
        BROWSER_IMPLEMENTATION_ID,
        conduit_pete::PICO_INTERACTION_IMPLEMENTATION
    );
    let projection = physical_control_surface_projection(&app).unwrap();
    assert_eq!(projection.plan_id, CONTROL_SURFACE_PLAN_ID);
    assert_eq!(projection.choices.len(), 4);
    assert_eq!(projection.maximum_pending_events, 2);
}

#[test]
fn physical_and_browser_choice_scalar_and_action_converge_through_application_state() {
    let mut app = InteractionConvergenceApplication::new().unwrap();
    let mut surface =
        PicoInteractionSurface::prepare(physical_control_surface_projection(&app).unwrap())
            .unwrap();

    let saw = surface
        .propose(physical_event(
            "pico/gpio/4/switch",
            4,
            1,
            PhysicalInput::ChoicePressed {
                resource_id: "pico/gpio/4/switch".into(),
            },
        ))
        .unwrap();
    let saw_receipt = app
        .submit(physical("pico/gpio/4/switch", None), saw)
        .unwrap();
    assert_eq!(
        saw_receipt.resulting_values,
        vec![value("music/waveform@1", b"saw").unwrap()]
    );
    let browser_reflects = surface.manifest(&app.states().waveform, true).unwrap();
    assert_eq!(browser_reflects.values, saw_receipt.resulting_values);
    surface.complete_one();

    let triangle = proposal(
        &app.contracts().waveform,
        &app.states().waveform,
        2,
        InteractionProposalPayload::Values(vec![value("music/waveform@1", b"triangle").unwrap()]),
    );
    let triangle_receipt = app.submit(browser(), triangle).unwrap();
    let physical_reflects = surface.manifest(&app.states().waveform, true).unwrap();
    assert_eq!(physical_reflects.values, triangle_receipt.resulting_values);

    let mapped = surface
        .propose(physical_event(
            "pico/adc/0/potentiometer",
            6,
            3,
            PhysicalInput::ScalarSample {
                sample: 2000,
                prior_sample: Some(1900),
            },
        ))
        .unwrap();
    let mapping_identity = surface.projection().scalar_mapping.mapping_identity.clone();
    let mapped_receipt = app
        .submit(
            physical("pico/adc/0/potentiometer", Some(mapping_identity.clone())),
            mapped,
        )
        .unwrap();
    assert_eq!(
        mapped_receipt.resulting_values,
        vec![quantity(50, QuantityUnit::Percent).unwrap()]
    );
    assert!(
        matches!(mapped_receipt.source, PresenterSource::Physical { mapping_identity: Some(ref id), .. } if id == &mapping_identity)
    );
    surface.complete_one();

    let browser_volume = proposal(
        &app.contracts().volume,
        &app.states().volume,
        4,
        InteractionProposalPayload::Values(vec![quantity(73, QuantityUnit::Percent).unwrap()]),
    );
    app.submit(browser(), browser_volume).unwrap();
    assert_eq!(
        surface.manifest(&app.states().volume, true).unwrap().values,
        vec![quantity(73, QuantityUnit::Percent).unwrap()]
    );

    let physical_panic = surface
        .propose(physical_event(
            "pico/gpio/10/switch",
            4,
            5,
            PhysicalInput::ActionPressed,
        ))
        .unwrap();
    let physical_action = app
        .submit(physical("pico/gpio/10/switch", None), physical_panic)
        .unwrap();
    surface.complete_one();
    let browser_panic = proposal(
        &app.contracts().panic,
        &app.states().panic,
        6,
        InteractionProposalPayload::Activate,
    );
    let browser_action = app.submit(browser(), browser_panic).unwrap();
    assert!(physical_action.action_invoked && browser_action.action_invoked);
    assert_eq!(physical_action.semantic_id, browser_action.semantic_id);
}

#[test]
fn boolean_relative_and_text_asymmetry_remain_truthful() {
    let mut app = InteractionConvergenceApplication::new().unwrap();
    let sustain = proposal(
        &app.contracts().sustain,
        &app.states().sustain,
        1,
        InteractionProposalPayload::Values(vec![
            value(BOOL_INFO_ID, &InfoBool::TRUE.encode()).unwrap()
        ]),
    );
    let sustain_receipt = app.submit(browser(), sustain).unwrap();
    assert!(!sustain_receipt.action_invoked);

    let relative = proposal(
        &app.contracts().transpose_relative,
        &app.states().transpose_relative,
        2,
        InteractionProposalPayload::Relative(quantity(1, QuantityUnit::One).unwrap()),
    );
    app.submit(browser(), relative).unwrap();

    let name = proposal(
        &app.contracts().name,
        &app.states().name,
        3,
        InteractionProposalPayload::Values(vec![value(TEXT_INFO_ID, b"Still Conduit").unwrap()]),
    );
    app.submit(browser(), name).unwrap();
    assert_eq!(
        app.states().name.current[0].canonical_bytes,
        b"Still Conduit"
    );

    let surface =
        PicoInteractionSurface::prepare(physical_control_surface_projection(&app).unwrap())
            .unwrap();
    let offers = surface.offers(&PhysicalResourceStatus {
        available_resource_ids: vec![
            "pico/gpio/10/switch".into(),
            "pico/gpio/2/switch".into(),
            "pico/gpio/3/switch".into(),
            "pico/gpio/4/switch".into(),
            "pico/gpio/5/switch".into(),
            "pico/adc/0/potentiometer".into(),
            "pico/i2c/ssd1306".into(),
        ],
    });
    assert!(offers.action && offers.scalar && offers.presentation);
    assert_eq!(offers.choice_option_identities.len(), 4);
    assert_eq!(
        app.contracts().name.family,
        conduit_core::InteractionFamily::Text {
            maximum_bytes: 32,
            allow_empty: false
        }
    );
}

#[test]
fn either_presenter_can_disappear_without_forwarding_or_erasing_the_other() {
    let mut browser_only = InteractionConvergenceApplication::new().unwrap();
    let browser_choice = proposal(
        &browser_only.contracts().waveform,
        &browser_only.states().waveform,
        1,
        InteractionProposalPayload::Values(vec![value("music/waveform@1", b"pulse").unwrap()]),
    );
    assert!(browser_only.submit(browser(), browser_choice).is_ok());

    let mut physical_only = InteractionConvergenceApplication::new().unwrap();
    let mut surface = PicoInteractionSurface::prepare(
        physical_control_surface_projection(&physical_only).unwrap(),
    )
    .unwrap();
    let physical_choice = surface
        .propose(physical_event(
            "pico/gpio/3/switch",
            4,
            1,
            PhysicalInput::ChoicePressed {
                resource_id: "pico/gpio/3/switch".into(),
            },
        ))
        .unwrap();
    assert!(physical_only
        .submit(physical("pico/gpio/3/switch", None), physical_choice)
        .is_ok());

    let stale = surface.propose(PhysicalEvent {
        plan_id: "plan/after-resource-replacement".into(),
        ..physical_event("pico/gpio/10/switch", 4, 2, PhysicalInput::ActionPressed)
    });
    assert!(matches!(
        stale,
        Err(PhysicalInteractionFailure::StalePlan { .. })
    ));
    assert_eq!(
        browser_only.source_document_id(),
        physical_only.source_document_id()
    );
    assert_eq!(
        browser_only.checked_form_id(),
        physical_only.checked_form_id()
    );
}

#[test]
fn evidence_correlates_form_body_plan_play_and_recursive_vs_direct_seams() {
    let mut app = InteractionConvergenceApplication::new().unwrap();
    let direct = proposal(
        &app.contracts().panic,
        &app.states().panic,
        1,
        InteractionProposalPayload::Activate,
    );
    let receipt = app.submit(browser(), direct).unwrap();
    assert_eq!(receipt.body_id, CONTROL_SURFACE_BODY_ID);
    assert_eq!(receipt.plan_id, CONTROL_SURFACE_PLAN_ID);
    assert_eq!(receipt.play_id, CONTROL_SURFACE_PLAY_ID);
    assert!(matches!(receipt.source, PresenterSource::Browser { .. }));

    let decoded =
        Quantity::decode(&quantity(73, QuantityUnit::Percent).unwrap().canonical_bytes).unwrap();
    assert_eq!(decoded, Quantity::new(73, QuantityUnit::Percent));
}
