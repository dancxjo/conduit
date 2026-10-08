{
    use conduit_language::{committed_discourse::*, committed_prosody::*};
    use conduit_speech::{linguistic_prosody::*, pitch_trajectory::*};
    use std::io::Write;
    let discourse = prepare_committed_vocative_fact("committed/addressee".into(), &case.lexical, &commitments[1], LinguisticDerivationProvenance::deterministic_rule("language/committed-vocative-discourse".into(), "actual-retained-commit/1".into()).unwrap()).unwrap();
    let rich = prepare_rich_prosody_from_committed_vocative(&case.lexical, 2, &discourse, case.rich.requested().profile()).unwrap();
    assert!(core::ptr::eq(rich.discourse().committed(), &commitments[1]));
    let basis = |i: usize| if *witnesses[i].word_position() == 1 { LinguisticProsodyBasis::Rich(rich.prepared()) } else { LinguisticProsodyBasis::Fallback(&case.fallback[0]) };
    let bindings = segments.iter().enumerate().map(|(i, segment)| {
        let b = basis(i);
        SpeechLinguisticProsodyBinding::new(b.choice().clone(), case.voice.identity().clone(), case.lexical.tape().source().material().language().clone(), b.profile().identity().clone(), language::speech_provenance(), SpeechLinguisticProsodyRealization::new(SpeechDurationSpecification::known(100, if *witnesses[i].word_position() == 1 { 3 } else { 0 }).unwrap(), SpeechBoundarySpecification::Known(if *witnesses[i].word_position() == 1 { SpeechBoundaryKind::Phrase } else { SpeechBoundaryKind::Word }), segment.prosody().clone()).unwrap()).unwrap()
    }).collect::<Vec<_>>();
    let trajectories = segments.iter().enumerate().map(|(i, segment)| {
        let SpeechDurationSpecification::Known(d) = segment.prosody().duration() else { panic!() };
        let SpeechCycleSpecification::Known(c) = segment.prosody().fundamental_cycle() else { panic!() };
        SpeechLinearPitchTrajectory::new(SpeechExactDuration::new(*d.denominator(), *d.numerator_seconds()).unwrap(), SpeechFundamentalCycle::new(if *witnesses[i].word_position() == 1 { 300 } else { 200 }, 1).unwrap(), SpeechFundamentalCycle::new(*c.denominator(), *c.numerator_seconds()).unwrap()).unwrap()
    }).collect::<Vec<_>>();
    let pitch_segments = segments.iter().map(|s| SpeechPlannedSegmentIntent::new(s.occurrence().clone(),s.phone().clone(),s.phoneme().clone(),s.prosody().clone(),s.provenance().clone(),s.sources().clone(),s.stress().clone(),s.word_position().clone()).unwrap()).collect::<Vec<_>>();
    let pitches = pitch_segments.iter().enumerate().map(|(i, segment)| prepare_linguistic_pitch(basis(i), &bindings[i], segment, &trajectories[i]).unwrap()).collect::<Vec<_>>();
    let rich_plans = gestures.iter().zip(&plans).zip(&pitches).map(|((gesture, plan), pitch)| prepare_linguistic_greeting_renderer(gesture, plan, pitch).unwrap()).collect::<Vec<_>>();
    // Every segment and both endpoints are prepared before the first sample.
    let mut samples = Vec::<i16>::new();
    let mut programs = Vec::<String>::new();
    let mut records = std::env::var_os("CONDUIT_LINGUISTIC_GREETING_PITCH_FRAMES").map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    for plan in &rich_plans {
        let mut cursor = plan.cursor();
        while let Some(frame) = plan.next(&mut cursor).unwrap() {
            let rendered = frame.rendered().rendered();
            samples.push(i16::try_from(rendered.sample()).unwrap());
            if let Some(file) = records.as_mut() {
                file.write_all(&u32::try_from(samples.len() - 1).unwrap().to_le_bytes()).unwrap();
                let admission = frame.pitch().admission().clone().encode().unwrap();
                for bytes in [admission.as_slice(), rendered.dsp_input_canonical(), rendered.dsp_output_canonical()] {
                    file.write_all(&u32::try_from(bytes.len()).unwrap().to_le_bytes()).unwrap();
                    file.write_all(bytes).unwrap();
                }
                let executions = frame.pitch().executions().iter().chain(rendered.executions()).chain(frame.rendered().target_selection_and_reset_executions()).collect::<Vec<_>>();
                file.write_all(&u32::try_from(executions.len()).unwrap().to_le_bytes()).unwrap();
                for execution in executions {
                    let index = programs.iter().position(|p| p == execution.source_program_hex()).unwrap_or_else(|| { programs.push(execution.source_program_hex().into()); programs.len() - 1 });
                    file.write_all(&u32::try_from(index).unwrap().to_le_bytes()).unwrap();
                    for bytes in [execution.input_canonical(), execution.output_canonical()] {
                        file.write_all(&u32::try_from(bytes.len()).unwrap().to_le_bytes()).unwrap();
                        file.write_all(bytes).unwrap();
                    }
                }
            }
        }
    }
    if let Some(mut file) = records { file.flush().unwrap(); }
    assert_eq!(samples.len(), 32000);
    assert!(samples.iter().any(|s| *s != 0));
    if let Some(path) = std::env::var_os("CONDUIT_LINGUISTIC_GREETING_OUTPUT") {
        let evidence = serde_json::json!({"scope":"Original committed shared IPA carrier with exact committed vocative to rich pitch; reviewed 200-to-300Hz cycle trajectory repeated per Travis segment; boundary/pause and prominence are retained offered metadata, not yet realized; no continuous coarticulation, neural, played or attended proof", "original_utterance_intent_bytes":composite.clone().encode().unwrap(),"original_commitments":material["commitments"],"committed_discourse":discourse.fact().fact().clone().encode().unwrap(),"rich_prosody_request":rich.prepared().requested().clone().encode().unwrap(),"rich_prosody_accepted":rich.prepared().accepted().clone().encode().unwrap(),"linguistic_pitch_admissions":pitches.iter().map(|p|p.accepted().clone().encode().unwrap()).collect::<Vec<_>>(),"rate_hz":16000,"frames":samples.len(),"programs":programs});
        std::fs::write(path, serde_json::to_vec(&evidence).unwrap()).unwrap();
    }
    if let Some(path) = std::env::var_os("CONDUIT_LINGUISTIC_GREETING_WAV") {
        let bytes = u32::try_from(samples.len() * 2).unwrap();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF"); wav.extend_from_slice(&(36 + bytes).to_le_bytes()); wav.extend_from_slice(b"WAVEfmt "); wav.extend_from_slice(&16u32.to_le_bytes()); wav.extend_from_slice(&1u16.to_le_bytes()); wav.extend_from_slice(&1u16.to_le_bytes()); wav.extend_from_slice(&16000u32.to_le_bytes()); wav.extend_from_slice(&32000u32.to_le_bytes()); wav.extend_from_slice(&2u16.to_le_bytes()); wav.extend_from_slice(&16u16.to_le_bytes()); wav.extend_from_slice(b"data"); wav.extend_from_slice(&bytes.to_le_bytes());
        for sample in &samples { wav.extend_from_slice(&sample.to_le_bytes()); }
        std::fs::write(path, wav).unwrap();
    }
}
