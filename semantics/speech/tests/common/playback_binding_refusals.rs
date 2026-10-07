use conduit_kernel::PortId;
use conduit_speech::{
    native_playback_back::*, native_playback_contract as contract, playback_basis::*,
};
pub fn check(tape: &PreparedSpeechPlaybackTape<'_>, other_tape: &PreparedSpeechPlaybackTape<'_>) {
    let plan = super::graph::plan(tape);
    let mut gear = plan.fragments[0]
        .placements
        .iter()
        .find(|value| value.kind_id.as_str() == contract::KIND)
        .unwrap()
        .clone();
    gear.configuration[0].value = contract::basis_configuration(other_tape.basis()).unwrap();
    assert!(matches!(
        NativeSpeechPlaybackBack::prepare::<1>(&gear, tape, PortId(0), PortId(0)),
        Err(NativePlaybackPreparationRefusal::Configuration)
    ));
    use conduit_plot::rust_binding::NativeRustBinding;
    let text = tape
        .basis()
        .links()
        .linguistic_bases()
        .iter()
        .next()
        .unwrap()
        .material();
    let profile = conduit_language::LanguageTextRevision::semantic_type()
        .unwrap()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    gear.configuration[0].value = conduit_core::ConfigurationValue::Structured(
        conduit_core::StructuredConfigurationValue::new(profile, text.clone().encode().unwrap())
            .unwrap(),
    );
    assert!(matches!(
        NativeSpeechPlaybackBack::prepare::<1>(&gear, tape, PortId(0), PortId(0)),
        Err(NativePlaybackPreparationRefusal::Configuration)
    ));
}
