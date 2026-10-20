#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
use conduit_audio::*;
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};
use conduit_speech::{
    carried_formant::FrozenCarriedFormant,
    complete_carrier::{CompleteCarrierAcoustics, PreparedCompleteSpeechCarrier},
    fargan_carrier_epochs::prepare_carrier_epochs,
    fargan_endpoint50::{native::SpeechFarganEndpointCycles50, prepare_fargan_endpoint50},
    semantic::SpeechFundamentalCycle,
};
#[test]
fn endpoint_admits_only_actual_complete_epochs_and_retains_executed_source_receipts() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    let evidence = f.complete_evidence();
    let structure = f.structure();
    let complete = PreparedCompleteSpeechCarrier::prepare(
        &original,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &evidence.request,
        &evidence.result,
        CompleteCarrierAcoustics::not_supplied(),
    )
    .unwrap();
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let basis =
        AudioSampleRateBasis::new(anchor.clone(), AudioFrameQuantization::Floor, 8000).unwrap();
    let frozen =
        FrozenCarriedFormant::prepare(&complete, &f.formant, &f.boundaries, &anchor, &basis)
            .unwrap();
    let epochs = prepare_carrier_epochs(&complete, &frozen, 50).unwrap();
    let admitted = prepare_fargan_endpoint50(&epochs).unwrap();
    assert!(core::ptr::eq(admitted.epochs(), &epochs));
    assert_eq!(
        admitted.epochs().source().original_bytes(),
        complete.canonical_bytes()
    );
    assert_eq!(admitted.cycles().len(), 50);
    assert_eq!(admitted.steps().len(), 51);
    let gate =
        PortableExpressionProgram::from_canonical_hex(admitted.source_program_hex()).unwrap();
    assert_eq!(
        gate.evaluate(admitted.gate_input_canonical()).unwrap(),
        admitted.gate_output_canonical()
    );
    assert_eq!(admitted.gate_output_canonical(), [1]);
    let step = PortableExpressionProgram::from_canonical_hex(admitted.step_program_hex()).unwrap();
    for (epoch, cycle) in epochs.epochs().iter().zip(admitted.cycles()) {
        assert_eq!(epoch.period().unwrap().projection().input().cycle(), cycle);
        assert!(epoch.blockers().is_empty());
    }
    for receipt in admitted.steps() {
        assert_eq!(
            step.evaluate(receipt.input_canonical()).unwrap(),
            receipt.output_canonical()
        );
        assert!(receipt.descriptor().feature_source_index() < &50);
        assert!(receipt.descriptor().period_source_index() < &50);
    }
    // Equal rounded periods do not admit a different unreduced original ratio.
    for mismatch in [0usize, 1, 49] {
        let mut cycles: [SpeechFundamentalCycle; 50] =
            admitted.cycles().to_vec().try_into().unwrap();
        let cycle = &cycles[mismatch];
        cycles[mismatch] =
            SpeechFundamentalCycle::new(cycle.denominator() * 2, cycle.numerator_seconds() * 2)
                .unwrap();
        let input = SpeechFarganEndpointCycles50::new(cycles)
            .unwrap()
            .encode()
            .unwrap();
        assert_eq!(gate.evaluate(&input).unwrap(), [0]);
    }
}
