//! Independent Native re-admission; no fixture phone/word tables are consulted.
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    carried_formant::{FrozenCarriedFormant, PreparedOfflinePcm},
    carrier_identity::canonical_carrier_bytes,
    correspondence::PreparedSpeechCorrespondence,
    ipa_inventory::PreparedIpaInventory,
    semantic::*,
    syllable_structure::PreparedSyllableStructure,
};
fn native<T: NativeRustBinding>(bytes: &[u8]) -> Result<T, String> {
    T::decode(bytes).map_err(|e| format!("Native: {e:?}"))
}
fn many<T: NativeRustBinding>(bytes: &[u8]) -> Result<Vec<T>, String> {
    let parts: Vec<Vec<u8>> = postcard::from_bytes(bytes).map_err(|e| format!("parts: {e:?}"))?;
    parts.iter().map(|bytes| native(bytes)).collect()
}
fn require(value: bool, reason: &str) -> Result<(), String> {
    if value {
        Ok(())
    } else {
        Err(reason.into())
    }
}
fn next<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], String> {
    let length = number(bytes)?;
    let length = usize::try_from(length).map_err(|_| "section length")?;
    let (head, tail) = bytes.split_at_checked(length).ok_or("section bytes")?;
    *bytes = tail;
    Ok(head)
}
fn number(bytes: &mut &[u8]) -> Result<u64, String> {
    let (head, tail) = bytes.split_at_checked(8).ok_or("section length")?;
    *bytes = tail;
    Ok(u64::from_le_bytes(head.try_into().unwrap()))
}
pub fn inspect(bytes: &[u8], expected: &[u8; 32]) -> Result<PreparedOfflinePcm, String> {
    require(bytes.len() <= 2_000_000, "artifact evidence bound")?;
    require(
        &conduit_core::semantic_digest("proof/carried-formant-artifact@1", bytes) == expected,
        "artifact identity changed",
    )?;
    let p: Vec<Vec<u8>> = postcard::from_bytes(bytes).map_err(|e| format!("packet: {e:?}"))?;
    require(p.len() == 24, "component count")?;
    let source: conduit_language::LanguageTextRevision = native(&p[0])?;
    let intent: SpeechUtteranceIntent = native(&p[1])?;
    let phonemes: SpeechPhonemeSequence = native(&p[2])?;
    let phones: SpeechPhoneSequence = native(&p[3])?;
    let inventory: SpeechInventory = native(&p[4])?;
    let profile: SpeechIpaNotationProfile = native(&p[5])?;
    let alignment: SpeechRealizationAlignment = native(&p[6])?;
    let syllables: Vec<SpeechSyllableSource> = many(&p[7])?;
    let boundaries: Vec<SpeechPositionedBoundary> = many(&p[8])?;
    let words: Vec<TranslationCorrespondence> = many(&p[9])?;
    let formant: SpeechFormantVoiceProfile = native(&p[10])?;
    let boundary_profile: SpeechFormantBoundaryProfile = native(&p[11])?;
    let request: conduit_language::LanguageParserBeamCommitRequest = native(&p[12])?;
    let result: conduit_language::LanguageParserBeamCommitResult = native(&p[13])?;
    let admission: SpeechOfflinePcmAdmission = native(&p[14])?;
    let lexical_profile: conduit_language::LanguageLexicalProfile = native(&p[17])?;
    let lexical: conduit_language::LanguageLexicalTape = native(&p[18])?;
    let rebuilt = conduit_language::lexical::prepare_lexical_tape(&source, &lexical_profile, None)
        .map_err(|e| format!("lexical: {e:?}"))?;
    require(rebuilt.tape() == &lexical, "lexical source differs")?;
    require(
        request.beam().basis().text() == source.material().identity()
            && request.beam().basis().source_revision() == source.material().revision(),
        "stale commitment source",
    )?;
    require(
        *request.through() as usize == lexical.tokens().len(),
        "partial commitment",
    )?;
    require(
        p[19] == super::fixture::commitment_source().as_bytes(),
        "commitment Source differs",
    )?;
    require(
        result == super::fixture::execute_commitment(&request),
        "commitment execution differs",
    )?;
    require(
        *result.accepted() && *result.frontier().committed() == *request.through(),
        "uncommitted result",
    )?;
    require(
        matches!(
            source.finality(),
            conduit_language::LanguageTextFinality::Final
        ),
        "partial text",
    )?;
    for token in lexical.tokens().as_slice() {
        require(
            *token.span().end() <= u64::from(source.stable_prefix().unwrap_or(0)),
            "unstable source",
        )?;
    }
    // Recover exact admitted definition bindings from the canonical shared
    // evidence packet; no IPA display is used to regenerate a phone sequence.
    let mut canonical = p[15].as_slice();
    for _ in 0..8 {
        let _ = next(&mut canonical)?;
    }
    let count = number(&mut canonical)?;
    require(count <= 64, "phone binding bound")?;
    let mut pb = Vec::new();
    for _ in 0..count {
        pb.push(native::<SpeechIpaPhoneDefinitionBinding>(next(
            &mut canonical,
        )?)?);
        let _ = next(&mut canonical)?;
    }
    let count = number(&mut canonical)?;
    require(count <= 64, "phoneme binding bound")?;
    let mut phb = Vec::new();
    for _ in 0..count {
        phb.push(native::<SpeechIpaPhonemeDefinitionBinding>(next(
            &mut canonical,
        )?)?);
        let _ = next(&mut canonical)?;
    }
    require(canonical.is_empty(), "trailing canonical bytes")?;
    let admitted_inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &pb,
        &phb,
    )
    .map_err(|e| format!("inventory: {e:?}"))?;
    let carrier = PreparedSpeechCorrespondence::prepare(
        &intent,
        &phonemes,
        &phones,
        source.material(),
        &alignment,
        &admitted_inventory,
    )
    .map_err(|e| format!("correspondence: {e:?}"))?;
    require(
        canonical_carrier_bytes(&carrier).map_err(|e| format!("canonical: {e:?}"))? == p[15],
        "original carrier changed",
    )?;
    let vocabulary: SpeechModelLinguisticVocabulary = native(&p[21])?;
    let checkpoint: SpeechModelCheckpointProjection = native(&p[22])?;
    use conduit_speech::model_linguistic_projection::*;
    let training = prepare_model_training_input(&carrier, &vocabulary)
        .map_err(|e| format!("model training projection: {e:?}"))?;
    let inference =
        prepare_model_inference_input(&carrier, &vocabulary, &checkpoint, checkpoint.checkpoint())
            .map_err(|e| format!("model inference projection: {e:?}"))?;
    require(
        training.carrier_identity() == inference.projection().carrier_identity(),
        "model source differs",
    )?;
    require(
        training.phone_tokens() == inference.projection().phone_tokens()
            && training.phoneme_tokens() == inference.projection().phoneme_tokens(),
        "model entrances differ",
    )?;
    let terminal = inference.refuse_unsupported_backend();
    require(
        terminal.reason == ModelBackendRefusal::NoAdmittedAcousticBack,
        "unexpected acoustic terminal",
    )?;
    let mut evidence = Vec::new();
    evidence.extend_from_slice(&training.carrier_identity());
    for bytes in [&p[21], &p[22]] {
        evidence.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        evidence.extend_from_slice(bytes);
    }
    evidence.extend_from_slice(b"ConduitVoice acoustic realization");
    evidence.extend_from_slice(b"NoAdmittedAcousticBack");
    require(
        conduit_core::semantic_digest("proof/shared-model-terminal@1", &evidence).as_slice()
            == p[23],
        "model terminal evidence changed",
    )?;
    let mut source_words: Vec<Option<LanguageSegmentRef>> = vec![None; phonemes.tokens().len()];
    for relation in &words {
        let TranslationCorrespondence::Aligned(group) = relation else {
            return Err("unsupported text correspondence".into());
        };
        require(group.sources().len() == 1, "ambiguous source word")?;
        let word = &group.sources()[0];
        conduit_speech::text_admission::resolve_text(word, source.material())
            .map_err(|e| format!("text occurrence: {e:?}"))?;
        let LanguageSegmentRef::Text(span) = word else {
            return Err("non-text word".into());
        };
        require(
            matches!(span.kind(), LanguageTextSegmentKind::Word),
            "nonword source",
        )?;
        require(
            lexical.tokens().as_slice().iter().any(|token| {
                *token.span().start() == u64::from(*span.range().start())
                    && *token.span().end() == u64::from(*span.range().end())
            }),
            "foreign lexical word",
        )?;
        for target in group.targets().as_slice() {
            let resolved = conduit_speech::reference_admission::resolve_phoneme(target, &phonemes)
                .map_err(|e| format!("phoneme source: {e:?}"))?;
            let ordinal = *resolved.checked().basis_match().reference().ordinal() as usize;
            require(
                source_words[ordinal].replace(word.clone()).is_none(),
                "duplicate phoneme source",
            )?;
        }
    }
    require(
        source_words.iter().all(Option::is_some),
        "missing phoneme source",
    )?;
    for group in alignment.correspondences().as_slice() {
        let SpeechRealizationCorrespondence::Aligned(group) = group else {
            return Err("unsupported committed relation".into());
        };
        let mut words = Vec::new();
        for phoneme in group.phonemes().as_slice() {
            let word = source_words
                .get(*phoneme.ordinal() as usize)
                .and_then(Option::as_ref)
                .ok_or("foreign phoneme source")?;
            if !words.contains(word) {
                words.push(word.clone());
            }
        }
        for phone in group.phones().as_slice() {
            let event = intent
                .events()
                .as_slice()
                .iter()
                .find_map(|event| match event {
                    SpeechUtteranceIntentEvent::Segment(value) if value.occurrence() == phone => {
                        Some(value)
                    }
                    _ => None,
                })
                .ok_or("missing source event")?;
            require(
                event.sources().as_slice() == words.as_slice(),
                "phone source correspondence changed",
            )?;
        }
    }
    let structure = PreparedSyllableStructure::prepare(
        &intent,
        &phones,
        source.material(),
        &syllables,
        &boundaries,
    )
    .map_err(|e| format!("structure: {e:?}"))?;
    require(
        structure.syllables().len() == syllables.len() && words.len() == lexical.tokens().len(),
        "structural component count",
    )?;
    let basis: conduit_audio::AudioSampleRateBasis = native(&p[20])?;
    let frozen = FrozenCarriedFormant::prepare(
        &carrier,
        &formant,
        &boundary_profile,
        basis.anchor(),
        &basis,
    )
    .map_err(|e| format!("formant: {e:?}"))?;
    require(
        frozen.projection_bytes() == p[16],
        "projection receipts differ",
    )?;
    PreparedOfflinePcm::prepare(frozen, admission.declared().clone())
        .map_err(|e| format!("offline declaration: {e:?}"))
}
