//! Inspection starts only from bytes and accepted owner contracts. It has no
//! reference to the producer's original structs or its fixture ID tables.
use super::{commitment, constructors, transport};
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::ipa_constructors::IpaConstructor;
use conduit_speech::{
    correspondence::PreparedSpeechCorrespondence,
    intent_sources::{resolve_intent_sources, IntentSourceMaterial},
    ipa_inventory::PreparedIpaInventory,
    semantic::*,
    syllable_structure::PreparedSyllableStructure,
};

pub const IDENTITY_DOMAIN: &str = "proof/committed-linguistic-carrier@1";
pub fn identity(bytes: &[u8]) -> [u8; 32] {
    conduit_core::semantic_digest(IDENTITY_DOMAIN, bytes)
}
pub fn inspect_original(bytes: &[u8], expected: [u8; 32]) -> Result<Inspection, String> {
    require(bytes.len() <= 2_000_000, "evidence byte bound")?;
    require(
        identity(bytes) == expected,
        "original carrier identity changed",
    )?;
    inspect(bytes)
}

pub struct Inspection {
    pub text: String,
    pub intent: SpeechUtteranceIntent,
    pub phonemes: SpeechPhonemeSequence,
    pub phones: SpeechPhoneSequence,
    pub alignment: SpeechRealizationAlignment,
    pub word_phonemes: Vec<TranslationCorrespondence>,
    pub syllables: Vec<SpeechSyllableSource>,
    pub boundaries: Vec<SpeechPositionedBoundary>,
    pub committed_words: u64,
    pub original: Vec<u8>,
}
fn native<T: NativeRustBinding>(bytes: &[u8]) -> Result<T, String> {
    T::decode(bytes).map_err(|e| format!("Native: {e:?}"))
}
fn many<T: NativeRustBinding>(bytes: &[u8], bound: usize) -> Result<Vec<T>, String> {
    let parts: Vec<Vec<u8>> = postcard::from_bytes(bytes).map_err(|e| format!("packets: {e:?}"))?;
    if parts.len() > bound {
        return Err("component bound".into());
    }
    parts.iter().map(|bytes| native(bytes)).collect()
}
fn require(value: bool, reason: &str) -> Result<(), String> {
    if value {
        Ok(())
    } else {
        Err(reason.into())
    }
}
pub fn inspect(bytes: &[u8]) -> Result<Inspection, String> {
    require(bytes.len() <= 2_000_000, "evidence byte bound")?;
    let p: Vec<Vec<u8>> = postcard::from_bytes(bytes).map_err(|e| format!("envelope: {e:?}"))?;
    require(p.len() == 20, "component count")?;
    let source: LanguageTextRevision = native(&p[0])?;
    let lexical: LanguageLexicalTape = native(&p[1])?;
    let intent: SpeechUtteranceIntent = native(&p[2])?;
    let phonemes: SpeechPhonemeSequence = native(&p[3])?;
    let phones: SpeechPhoneSequence = native(&p[4])?;
    let inventory: SpeechInventory = native(&p[5])?;
    let profile: SpeechIpaNotationProfile = native(&p[6])?;
    let phone_bindings: Vec<SpeechIpaPhoneDefinitionBinding> = many(&p[7], 64)?;
    let phoneme_bindings: Vec<SpeechIpaPhonemeDefinitionBinding> = many(&p[8], 64)?;
    let alignment: SpeechRealizationAlignment = native(&p[9])?;
    let syllables: Vec<SpeechSyllableSource> = many(&p[10], 64)?;
    let boundaries: Vec<SpeechPositionedBoundary> = many(&p[11], 256)?;
    let request: LanguageParserBeamCommitRequest = native(&p[12])?;
    let result: LanguageParserBeamCommitResult = native(&p[13])?;
    let intended: SpeechPhonemicTranscription = native(&p[16])?;
    let phone: SpeechPhoneNotation = native(&p[17])?;
    let phoneme: SpeechPhonemeNotation = native(&p[18])?;
    let word_phonemes: Vec<TranslationCorrespondence> = many(&p[19], 64)?;
    require(
        p[14] == transport::SOURCE.as_bytes(),
        "transport Source changed",
    )?;
    require(
        p[15] == commitment::source().as_bytes(),
        "commitment Source changed",
    )?;
    let rebuilt = prepare_lexical_tape(&source, lexical.profile(), None)
        .map_err(|e| format!("lexical: {e:?}"))?;
    require(
        rebuilt.tape() == &lexical,
        "original lexical source changed",
    )?;
    for hypothesis in [
        request.beam().candidate0(),
        request.beam().candidate1(),
        request.beam().candidate2(),
        request.beam().candidate3(),
    ] {
        require(
            *hypothesis.state().token_count() as usize == lexical.tokens().len(),
            "foreign parser token count",
        )?;
    }
    let basis = request.beam().basis();
    require(
        basis.text() == source.material().identity()
            && basis.source_revision() == source.material().revision(),
        "stale commitment source",
    )?;
    require(
        *request.through() as usize == lexical.tokens().len(),
        "partial commitment",
    )?;
    for token in lexical.tokens().as_slice() {
        require(
            *token.span().end() <= u64::from(source.stable_prefix().unwrap_or(0)),
            "unstable word",
        )?;
    }
    require(
        *result.accepted() && *result.frontier().committed() == *request.through(),
        "uncommitted result",
    )?;
    require(
        request.frontier().basis() == basis,
        "stale commitment frontier",
    )?;
    for candidate in [
        result.candidate0(),
        result.candidate1(),
        result.candidate2(),
        result.candidate3(),
    ] {
        let raw = candidate.state();
        let state = LanguageParserState::new(
            raw.basis().clone(),
            *raw.committed(),
            *raw.depth(),
            *raw.heads(),
            raw.relation0().clone(),
            raw.relation1().clone(),
            raw.relation2().clone(),
            raw.relation3().clone(),
            *raw.stack(),
            *raw.token_count(),
            *raw.unread(),
        )
        .map_err(|e| format!("committed state: {e:?}"))?;
        if *candidate.active() {
            require(
                *state.committed() == *request.through(),
                "active commitment frontier changed",
            )?;
        }
    }
    let admitted = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &phone_bindings,
        &phoneme_bindings,
    )
    .map_err(|e| format!("IPA inventory: {e:?}"))?;
    require(
        constructors::universal(
            IpaConstructor::Phone,
            phone.spelling().get(),
            phone.provenance().clone(),
        ) == p[17],
        "phone constructor changed",
    )?;
    require(
        constructors::scoped(
            IpaConstructor::Phoneme,
            phoneme.definition().notation(),
            phoneme.notation().provenance().clone(),
            &inventory,
            admitted.basis(),
            &phone_bindings,
            &phoneme_bindings,
        ) == p[18],
        "phoneme constructor changed",
    )?;
    let checked = admitted
        .phonemic_from_ipa(
            intended.original().clone(),
            intended.notation().provenance().clone(),
        )
        .map_err(|e| format!("phonemic: {e:?}"))?;
    require(
        checked.transcription() == &intended,
        "original phonemic material changed",
    )?;
    let intended_ids: Vec<_> = intended
        .occurrences()
        .as_slice()
        .iter()
        .filter_map(|occurrence| match occurrence.value() {
            SpeechPhonemicEvent::Phoneme(value) => Some(value.phoneme()),
            _ => None,
        })
        .collect();
    require(
        intended_ids.len() == phonemes.tokens().len(),
        "phoneme coverage",
    )?;
    for (id, token) in intended_ids.into_iter().zip(phonemes.tokens().as_slice()) {
        require(
            matches!(token.phoneme(),PhonemeSpecification::Known(value) if value==id),
            "original phoneme identity changed",
        )?;
    }
    let mut source_words: Vec<Option<LanguageSegmentRef>> = vec![None; phonemes.tokens().len()];
    for relation in &word_phonemes {
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
    PreparedSpeechCorrespondence::prepare(
        &intent,
        &phonemes,
        &phones,
        source.material(),
        &alignment,
        &admitted,
    )
    .map_err(|e| format!("correspondence: {e:?}"))?;
    PreparedSyllableStructure::prepare(
        &intent,
        &phones,
        source.material(),
        &syllables,
        &boundaries,
    )
    .map_err(|e| format!("structure: {e:?}"))?;
    let count = intent
        .events()
        .as_slice()
        .iter()
        .map(|event| match event {
            SpeechUtteranceIntentEvent::Segment(v) => v.sources().len(),
            SpeechUtteranceIntentEvent::Boundary(v) => v.sources().len(),
        })
        .sum();
    resolve_intent_sources(
        &intent,
        &vec![IntentSourceMaterial::Text(source.material()); count],
    )
    .map_err(|e| format!("source: {e:?}"))?;
    let mut seen = [false; 256];
    for syllable in &syllables {
        require(
            syllable.variety() == profile.variety(),
            "syllable Variety changed",
        )?;
        require(
            lexical.tokens().as_slice().iter().any(|token| {
                *token.span().start() == u64::from(*syllable.word().range().start())
                    && *token.span().end() == u64::from(*syllable.word().range().end())
            }),
            "word occurrence changed",
        )?;
        for (reference, index) in syllable
            .syllable()
            .phones()
            .as_slice()
            .iter()
            .zip(syllable.ipa_phone_indices().as_slice())
        {
            let ordinal = *reference.ordinal() as usize;
            require(!seen[ordinal], "duplicate syllable coverage")?;
            seen[ordinal] = true;
            let PhoneSpecification::Known(id) = phones.tokens()[ordinal].phone() else {
                return Err("unresolved phone".into());
            };
            let definition = inventory
                .phones()
                .as_slice()
                .iter()
                .find(|phone| phone.identity() == id)
                .ok_or("missing phone")?;
            let SpeechPhoneticEvent::Phone(value) = syllable.ipa().units()[*index as usize].value()
            else {
                return Err("nonphone IPA".into());
            };
            require(
                value.phone().spelling().get() == definition.ipa(),
                "target phone notation changed",
            )?;
        }
    }
    require(
        seen[..phones.tokens().len()].iter().all(|seen| *seen),
        "missing syllable coverage",
    )?;
    require(
        commitment::execute(&request) == result,
        "commitment execution changed",
    )?;
    require(
        transport::execute(&intent) == intent,
        "checked intent transport changed",
    )?;
    let encode_many = |values: Vec<Vec<u8>>| postcard::to_allocvec(&values).unwrap();
    let encoded = vec![
        source.clone().encode().unwrap(),
        lexical.clone().encode().unwrap(),
        intent.clone().encode().unwrap(),
        phonemes.clone().encode().unwrap(),
        phones.clone().encode().unwrap(),
        inventory.clone().encode().unwrap(),
        profile.clone().encode().unwrap(),
        encode_many(
            phone_bindings
                .iter()
                .cloned()
                .map(|v| v.encode().unwrap())
                .collect(),
        ),
        encode_many(
            phoneme_bindings
                .iter()
                .cloned()
                .map(|v| v.encode().unwrap())
                .collect(),
        ),
        alignment.clone().encode().unwrap(),
        encode_many(
            syllables
                .iter()
                .cloned()
                .map(|v| v.encode().unwrap())
                .collect(),
        ),
        encode_many(
            boundaries
                .iter()
                .cloned()
                .map(|v| v.encode().unwrap())
                .collect(),
        ),
        request.clone().encode().unwrap(),
        result.clone().encode().unwrap(),
        p[14].clone(),
        p[15].clone(),
        intended.encode().unwrap(),
        phone.encode().unwrap(),
        phoneme.encode().unwrap(),
        encode_many(
            word_phonemes
                .iter()
                .cloned()
                .map(|v| v.encode().unwrap())
                .collect(),
        ),
    ];
    require(encoded == p, "re-admitted Native identity changed")?;
    Ok(Inspection {
        text: source.material().text().clone(),
        intent,
        phonemes,
        phones,
        alignment,
        word_phonemes,
        syllables,
        boundaries,
        committed_words: *result.frontier().committed(),
        original: bytes.to_vec(),
    })
}
