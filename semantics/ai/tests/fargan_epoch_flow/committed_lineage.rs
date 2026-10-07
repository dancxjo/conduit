//! Host-only complete Source law admission and mechanical receipt correlation.
//! No linguistic or pronunciation policy executes in this Rust fixture.
use super::interface::admit_retained_session_native;
use super::*;
use sha2::{Digest, Sha256};
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("exact retained record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn sequence(value: &StructuredInfoValue) -> &[StructuredInfoValue] {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("exact retained collection")
    };
    values
}
fn scalar(value: &StructuredInfoValue) -> u64 {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("exact scalar")
    };
    u64::from_le_bytes(bytes.try_into().unwrap())
}
fn bytes(json: &serde_json::Value) -> Vec<u8> {
    serde_json::from_value(json.clone()).unwrap()
}
fn same(left: &StructuredInfoValue, right: &StructuredInfoValue) {
    assert_eq!(
        left.canonical_bytes().unwrap(),
        right.canonical_bytes().unwrap()
    );
}
fn program(document: &CheckedSyntaxDocument, name: &str) -> PortableExpressionProgram {
    let expanded =
        expand_canonical_plot_for_authoring(document, name, &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("checked Source program")
    };
    PortableExpressionProgram::from_canonical_hex(hex).unwrap()
}
#[test]
#[ignore = "exact private committed handoff and full Language/Speech Source closures"]
fn full_committed_native_acoustic_lineage_replays_source_roles_and_retains_pitch_basis() {
    let _ = admit_shared_basis();
}

pub(super) struct RetainedCommittedBasis {
    pub document: CheckedSyntaxDocument,
    pub receipt: serde_json::Value,
    pub encoded: Vec<u8>,
    pub committed: StructuredInfoValue,
    pub language_source: Vec<u8>,
    pub speech_source: Vec<u8>,
}

pub(super) fn admit_shared_basis() -> RetainedCommittedBasis {
    let language =
        std::fs::read(std::env::var("CONDUIT_COMMITTED_DEPENDENCY_SOURCE").unwrap()).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&language)),
        "e5bb22f0dfe06d4d958e05939bb3b787e77593cc4f9807d581cb014c5c5fbea1"
    );
    let directory =
        std::path::PathBuf::from(std::env::var("CONDUIT_COMMITTED_SPEECH_SOURCE").unwrap());
    let speech = std::fs::read(directory.join("semantic-source.conduit")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&speech)),
        "ba0a53558e279447b4cd116d2739ab0ccc3d0c97fe144d1cfa6e7a393a1bcd5b"
    );
    let started = std::time::Instant::now();
    let mut document = check_syntax_document(
        &parse_syntax_document(&String::from_utf8(language.clone()).unwrap()),
        &StartupCatalog::new(),
    )
    .unwrap();
    let mut imports = StartupCatalog::new();
    for ty in &document.native_types {
        imports
            .insert_structured_type(&ty.name, ty.value_type.clone())
            .unwrap();
    }
    let speech_document = check_syntax_document(
        &parse_syntax_document(&String::from_utf8(speech.clone()).unwrap()),
        &imports,
    )
    .unwrap();
    document
        .native_types
        .extend(speech_document.native_types.iter().cloned());
    let checked_in = started.elapsed();
    let encoded =
        std::fs::read(std::env::var("CONDUIT_COMMITTED_SPEECH_HANDOFF").unwrap()).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&encoded)),
        "d4be97a0c8350c72df496cae35f817dd617e449157dbe6a25e5dbda79eee2c89"
    );
    let receipt: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    let committed = admit_retained_session_native(
        &document,
        "LanguageParserCommittedDependencyAdmission",
        &bytes(&receipt["graph_receipt"]["committed_dependency_admission_bytes"]),
    )
    .unwrap();
    same(
        field(&committed, "admission"),
        &admit_retained_session_native(
            &document,
            "LanguageParserStableDependencyAdmission",
            &bytes(&receipt["graph_receipt"]["dependency_admission_bytes"]),
        )
        .unwrap(),
    );
    let admission = field(&committed, "admission");
    let query = field(field(admission, "fact"), "query");
    let beam = field(query, "beam");
    let dependent = usize::try_from(scalar(field(query, "dependent"))).unwrap();
    let tape = field(field(beam, "lexical"), "tape");
    let token = &sequence(field(tape, "tokens"))[dependent];
    let analysis = field(field(beam, "basis"), "analysis_revision");
    let role_request = admit_retained_session_native(
        &document,
        "SpeechTextTokenRoleRequest",
        &bytes(&receipt["token_participation"][0]["request_bytes"]),
    )
    .unwrap();
    same(field(&role_request, "basis"), field(admission, "arc"));
    same(field(&role_request, "analysis"), analysis);
    same(field(&role_request, "source"), field(tape, "source"));
    same(field(&role_request, "token"), token);
    assert_eq!(
        scalar(field(&role_request, "choice")),
        scalar(&sequence(field(field(beam, "candidate0"), "choices"))[dependent])
    );
    let role_program = program(&speech_document, "speech/text-token-role");
    let exported = PortableExpressionProgram::from_canonical_hex(
        &std::fs::read_to_string(directory.join("text_token_role_program.hex")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        role_program, exported,
        "full checked Source program matches producer closure"
    );
    assert_eq!(
        role_program
            .evaluate(&role_request.canonical_bytes().unwrap())
            .unwrap(),
        bytes(&receipt["token_participation"][0]["result_bytes"])
    );
    let playback = admit_retained_session_native(
        &document,
        "SpeechPlaybackBasis",
        &bytes(&receipt["playback_basis_bytes"]),
    )
    .unwrap();
    assert_eq!(
        field(&playback, "intent").canonical_bytes().unwrap(),
        bytes(&receipt["shared_realization_handoff"]["utterance_intent_bytes"])
    );
    assert_eq!(
        field(&playback, "voice_profile").canonical_bytes().unwrap(),
        bytes(&receipt["shared_realization_handoff"]["voice_profile_bytes"])
    );
    let links = field(&playback, "links");
    let bases = sequence(field(links, "linguistic_bases"));
    assert_eq!(bases.len(), 1);
    let base = &bases[0];
    same(field(base, "material"), field(tape, "source"));
    same(field(base, "token"), field(token, "identity"));
    let StructuredInfoValueShape::Leaf(hex) = field(base, "linguistic_evidence").shape() else {
        panic!("canonical hex evidence")
    };
    let text = std::str::from_utf8(hex).unwrap();
    let rich_bytes: Vec<u8> = text
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let rich = admit_retained_session_native(&document, "LanguageRichProsodyAccepted", &rich_bytes)
        .unwrap();
    let rich_request = field(&rich, "request");
    same(field(rich_request, "source"), field(tape, "source"));
    same(field(rich_request, "token"), token);
    same(
        field(field(rich_request, "discourse"), "basis"),
        field(admission, "arc"),
    );
    same(
        field(field(rich_request, "discourse"), "analysis"),
        analysis,
    );
    let shared = &receipt["shared_realization_handoff"];
    let intent = field(&playback, "intent");
    let events = sequence(field(intent, "events"));
    for occurrence in sequence(field(links, "occurrences")) {
        let event = usize::try_from(scalar(field(occurrence, "event"))).unwrap();
        let prosody = field(occurrence, "prosody");
        same(field(prosody, "requested"), field(&rich, "choice"));
        let StructuredInfoValueShape::Variant { tag, payload } = events[event].shape() else {
            panic!("native event")
        };
        assert_eq!(tag, "segment");
        same(field(field(prosody, "pitch"), "segment"), payload);
        let pitch = shared["pitch_admissions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["event"].as_u64() == Some(event as u64))
            .unwrap();
        assert_eq!(
            field(prosody, "pitch").canonical_bytes().unwrap(),
            bytes(&pitch["pitch_admission_bytes"])
        );
    }
    eprintln!("complete committed acoustic lineage readmitted and Source token role replayed; Sourcecheck={checked_in:?}, {} nativeTypes, {}phone pitch receipts; whole Nativeintent/profile retained; no neural waveform claim",document.native_types.len(),sequence(field(links,"occurrences")).len());
    RetainedCommittedBasis {
        document,
        receipt,
        encoded,
        committed,
        language_source: language,
        speech_source: speech,
    }
}
