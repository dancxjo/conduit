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
    require(p.len() == 20, "component count")?;
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
    let frozen = FrozenCarriedFormant::prepare(&carrier, &formant, &boundary_profile)
        .map_err(|e| format!("formant: {e:?}"))?;
    require(
        frozen.projection_bytes() == p[16],
        "projection receipts differ",
    )?;
    PreparedOfflinePcm::prepare(frozen, admission.declared().clone())
        .map_err(|e| format!("offline declaration: {e:?}"))
}
