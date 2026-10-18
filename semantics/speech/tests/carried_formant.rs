#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
#[path = "common/carried_formant_execution.rs"]
mod execution;
#[path = "common/carried_formant_export.rs"]
mod export;
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
use conduit_composite::KernelCompositeStatus;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId as KernelPort, ValueRef,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    carried_formant::{FrozenCarriedFormant, PreparedOfflinePcm},
    carried_formant_kernel::CarriedFormantBack,
    semantic::*,
};
use conduit_std_host::carried_formant::{CarriedFormantFactory, SOURCE_VALUE_KIND};
use std::sync::Arc;

fn frozen(f: &fixture::Fixture) -> FrozenCarriedFormant {
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    FrozenCarriedFormant::prepare(&original, &f.formant, &f.boundaries).unwrap()
}
fn offline(source: FrozenCarriedFormant) -> PreparedOfflinePcm {
    let digest =
        |bytes: &[u8; 32]| SpeechCarrierDigest::new(fixture::seq(bytes.iter().copied())).unwrap();
    let declaration = SpeechOfflinePcmDeclaration::new(
        fixture::evidence(),
        digest(source.projection_identity()),
        digest(source.source_identity()),
    )
    .unwrap();
    PreparedOfflinePcm::prepare(source, declaration).unwrap()
}

#[test]
fn original_multiword_intent_executes_through_installed_factory_plan_and_play() {
    let f = fixture::Fixture::new();
    let commitment = fixture::execute_commitment(&fixture::commit_request());
    assert!(*commitment.accepted());
    assert_eq!(*commitment.frontier().committed(), 2);
    let lexical =
        conduit_language::lexical::prepare_lexical_tape(&f.source, &f.lexical_profile, None)
            .unwrap();
    assert_eq!(lexical.tape().tokens().len(), 2);
    let request = fixture::commit_request();
    assert_eq!(
        request.beam().basis().text(),
        f.source.material().identity()
    );
    assert_eq!(
        request.beam().basis().source_revision(),
        f.source.material().revision()
    );
    let structure = conduit_speech::syllable_structure::PreparedSyllableStructure::prepare(
        &f.intent,
        &f.phones,
        f.source.material(),
        &f.syllables,
        &f.positioned_boundaries,
    )
    .unwrap();
    assert_eq!(structure.syllables().len(), 2);
    let realized = conduit_speech::intent_realization::prepare_intent_realization(
        &f.intent,
        &f.definitions,
        &f.formant,
        &f.boundaries,
    )
    .unwrap();
    let mut renderer = realized.renderer().unwrap();
    let mut expected_pcm = Vec::new();
    let mut block = [0_i16; 128];
    while !renderer.is_complete() {
        let count = renderer.render(&mut block).unwrap();
        for sample in &block[..count] {
            expected_pcm.extend_from_slice(&sample.to_le_bytes());
        }
    }
    let source = frozen(&f);
    let identity = *source.source_identity();
    let projection = *source.projection_identity();
    let projection_bytes = source.projection_bytes().to_vec();
    let original = source.original_bytes().to_vec();
    assert_eq!(source.total_frames(), 4000);
    assert_eq!(source.timing_receipts().len(), 7);
    assert_eq!(
        source
            .control_receipts()
            .iter()
            .filter(|(cycle, _)| cycle.is_some())
            .count(),
        5
    );
    let admitted = offline(source);
    let admission_bytes = admitted.admission().clone().encode().unwrap();
    let factory = CarriedFormantFactory::new(admitted, 7).unwrap();
    assert_eq!(factory.source().original_bytes(), original);
    let (mut kernel, plan) = execution::prepare(factory);
    let plays = kernel.start().unwrap().clone();
    let input = ValuePayload {
        value_kind: kind_id(SOURCE_VALUE_KIND),
        encoded: identity.to_vec(),
    };
    kernel.admit_input(&port_id("source"), 0, &input).unwrap();
    kernel.close_input(&port_id("source")).unwrap();
    let mut output = ValuePayload {
        value_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        encoded: Vec::with_capacity(285),
    };
    let mut pcm = Vec::new();
    let mut next_frame = 0;
    let mut complete = false;
    for _ in 0..4000 {
        let status = kernel.step().unwrap();
        if let Some(sequence) = kernel.output_into(&port_id("audio"), &mut output).unwrap() {
            let (header, bytes) =
                conduit_audio::PcmFrameHeader::decode_frame(&output.encoded).unwrap();
            assert_eq!(header.start_frame(), next_frame);
            assert_eq!(header.sample_rate_hz(), 8000);
            assert_eq!(header.clock_id(), 7);
            next_frame += u64::from(header.frame_count());
            pcm.extend_from_slice(bytes);
            kernel.complete_output(&port_id("audio"), sequence).unwrap();
        }
        if status == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    assert!(complete);
    assert_eq!(next_frame, 4000);
    assert_eq!(pcm.len(), 8000);
    assert_eq!(pcm, expected_pcm);
    assert_eq!(
        SpeechUtteranceIntent::decode(&f.intent.clone().encode().unwrap()).unwrap(),
        f.intent
    );
    assert!(pcm
        .as_chunks::<2>()
        .0
        .iter()
        .any(|pair| i16::from_le_bytes(*pair) != 0));
    assert!(kernel.signs().values().all(|signs| !signs.is_empty()));
    export::retain(export::Export {
        f: &f,
        original: &original,
        identity: &identity,
        projection: &projection,
        projection_bytes: &projection_bytes,
        admission: &admission_bytes,
        plan: &plan,
        plays: &plays,
        signs: &kernel.signs(),
        pcm: &pcm,
    });
    eprintln!("carried-formant source={identity:?} projection={projection:?} plan={:?} plays={plays:?} PCM-frames={next_frame}", plan.plan_id);
}

#[test]
fn rejected_transaction_pressure_and_cancellation_preserve_original_source() {
    let source = Arc::new(offline(frozen(&fixture::Fixture::new())));
    let identity = source.source().source_identity().to_vec();
    let original = source.source().original_bytes().to_vec();
    let mut back = CarriedFormantBack::prepare(source, 7).unwrap();
    let reference = ValueRef {
        slot: 0,
        generation: 7,
        byte_len: 32,
    };
    let bytes = StepInputBytes::test_frame([Some(identity.as_slice())], None);
    let mut blocked = StepIo::test_frame([Some(reference)], [false], [None], None, 3);
    assert_eq!(back.step(&mut blocked, &bytes), StepOutcome::Await);
    let mut ready = StepIo::test_frame([Some(reference)], [false], [Some(285)], None, 3);
    assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Progress);
    assert_eq!(back.rendered_frames(), 0);
    assert!(!ready.test_consumed(KernelPort(0)));
    let proposed = StepBack::<1>::prepared_output(&back, KernelPort(0))
        .unwrap()
        .to_vec();
    // Reject the transaction, then repeat: no duplicate cursor advance or loss.
    let mut retry = StepIo::test_frame([Some(reference)], [false], [Some(285)], None, 3);
    assert_eq!(back.step(&mut retry, &bytes), StepOutcome::Progress);
    assert_eq!(
        StepBack::<1>::prepared_output(&back, KernelPort(0)),
        Some(proposed.as_slice())
    );
    StepBack::<1>::step_committed(&mut back);
    assert_eq!(back.rendered_frames(), 128);
    assert_eq!(back.source().original_bytes(), original);
    StepBack::<1>::cancel(&mut back);
    assert_eq!(StepBack::<1>::prepared_output(&back, KernelPort(0)), None);
    assert!(matches!(
        back.step(&mut retry, &bytes),
        StepOutcome::Fail(_)
    ));
    assert_eq!(back.rendered_frames(), 128);
}

#[test]
fn foreign_source_and_projection_declaration_refuse_before_pcm() {
    let source = frozen(&fixture::Fixture::new());
    let original = *source.source_identity();
    let digest =
        |bytes: &[u8; 32]| SpeechCarrierDigest::new(fixture::seq(bytes.iter().copied())).unwrap();
    let wrong =
        SpeechOfflinePcmDeclaration::new(fixture::evidence(), digest(&[0; 32]), digest(&original))
            .unwrap();
    assert!(PreparedOfflinePcm::prepare(source, wrong).is_err());
    let source = Arc::new(offline(frozen(&fixture::Fixture::new())));
    let mut back = CarriedFormantBack::prepare(source, 7).unwrap();
    let wrong = [0_u8; 32];
    let bytes = StepInputBytes::test_frame([Some(wrong.as_slice())], None);
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 32,
    };
    let mut io = StepIo::test_frame([Some(reference)], [false], [Some(285)], None, 3);
    assert!(matches!(back.step(&mut io, &bytes), StepOutcome::Fail(_)));
    assert_eq!(StepBack::<1>::prepared_output(&back, KernelPort(0)), None);
    assert_eq!(back.rendered_frames(), 0);
}
