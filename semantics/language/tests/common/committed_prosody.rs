//! Supplied admitted symbolic graphs, not learned parsing or live playback.
#[allow(dead_code)]
#[path = "parser_kernel.rs"]
mod kernel;
#[path = "parser_revision_fixture.rs"]
mod parser;
#[path = "prosody.rs"]
pub mod prosody;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
pub fn source() -> String {
    [
        parser::source(),
        include_str!("../../discourse.conduit").into(),
        include_str!("../../prosody.conduit").into(),
        include_str!("../../dependency_commitment.conduit").into(),
        include_str!("../../prosody_commitment.conduit").into(),
    ]
    .join("\n")
}
pub struct Prepared {
    source: conduitos::protocol_source::PreparedProtocolSource,
    artifact: conduit_core::ArtifactId,
}
impl Prepared {
    pub fn new() -> Self {
        use conduitos::protocol_source::{
            PreparedProtocolSource, ProtocolSourcePackage, ProtocolSpecializationRequest,
            ProtocolValueReference,
        };
        let composite =
            conduit_language::dependency_commitment::committed_dependency_execution_source();
        let (imports, plots): (Vec<_>, Vec<_>) = composite
            .lines()
            .partition(|line| line.starts_with("with "));
        let package = ProtocolSourcePackage::compile(
            format!("{}\n{}\n{}", imports.join("\n"), source(), plots.join("\n")),
            &[ProtocolSpecializationRequest::Zip {
                left: ProtocolValueReference {
                    type_name: "LanguageDependencyExecutionContext".into(),
                    maximum_bytes:
                        conduit_language::dependency_commitment::DEPENDENCY_CONTEXT_MAXIMUM_BYTES,
                },
                right: ProtocolValueReference {
                    type_name: "LanguageParserBeamCommitResult".into(),
                    maximum_bytes:
                        conduit_language::dependency_commitment::DEPENDENCY_RESULT_MAXIMUM_BYTES,
                },
            }],
        )
        .unwrap();
        let digest = conduit_core::semantic_digest(
            "language/committed-prosody-package@1",
            &serde_json::to_vec(&package).unwrap(),
        );
        let artifact = conduit_core::ArtifactId::from(format!(
            "language/prosody/{}",
            digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ));
        Self {
            source: PreparedProtocolSource::prepare(package).unwrap(),
            artifact,
        }
    }
    pub fn play(&self, entry: &str, encoded: Vec<u8>) -> Vec<u8> {
        use conduit_core::ValuePayload;
        use conduit_kernel::scheduler::RemoteIngressOutcome;
        let mut run = kernel::Execution::prepare_source(&self.source, entry, &self.artifact);
        let input = run.kernel.definition().boundary.input_fronts[0]
            .external_port
            .clone();
        let output = run.kernel.definition().boundary.output_fronts[0]
            .external_port
            .clone();
        run.kernel.start().unwrap();
        let value = ValuePayload {
            value_kind: input.value_kind,
            encoded,
        };
        assert!(matches!(
            run.kernel.admit_input(&input.port_id, 0, &value).unwrap(),
            RemoteIngressOutcome::Accepted { .. }
        ));
        let mut result = ValuePayload {
            value_kind: output.value_kind,
            encoded: Vec::with_capacity(65536),
        };
        assert!((0..4000).any(|_| {
            run.step();
            run.kernel
                .output_into(&output.port_id, &mut result)
                .unwrap()
                .is_some()
        }));
        run.kernel.complete_output(&output.port_id, 0).unwrap();
        result.encoded
    }
}
pub fn symbolic(profile: &LanguageProsodyProfile) -> LanguageProsodySymbolicProfile {
    LanguageProsodySymbolicProfile::new(
        profile.clone(),
        LanguageProsodyDurationIntent::Lengthen,
        LanguageProsodyFocusIntent::Focused,
        LanguageProsodyRateIntent::Slower,
    )
    .unwrap()
}
pub fn request(
    f: &prosody::Fixture,
    through: u64,
    cancelled: bool,
    pressure: bool,
) -> LanguageParserBeamCommitRequest {
    let fact = f.discourse.fact();
    let basis = LanguageParserBasis::new(
        fact.analysis().clone(),
        fact.source().material().revision().clone(),
        fact.source().material().identity().clone(),
    )
    .unwrap();
    let count = f.lexical.tape().tokens().len() as u64;
    let head = match fact.basis().governor() {
        LanguageDependencyHead::Token(t) => *t.token().ordinal(),
        _ => unreachable!(),
    };
    let dep = *fact.basis().dependent().token().ordinal();
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let vocative = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Vocative,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let other = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let mut heads = [5, 5, 5, 5, 4];
    let mut rels = [other.clone(), other.clone(), other.clone(), other.clone()];
    let mut stack = [4, 4, 4, 4, 4];
    for i in 0..count as usize {
        heads[i] = if i as u64 == head { 4 } else { head };
        rels[i] = if i as u64 == head {
            root.clone()
        } else if i as u64 == dep {
            vocative.clone()
        } else {
            other.clone()
        };
        stack[i + 1] = i as u64;
    }
    let state = LanguageParserState::new(
        basis.clone(),
        0,
        count + 1,
        heads,
        rels[0].clone(),
        rels[1].clone(),
        rels[2].clone(),
        rels[3].clone(),
        stack,
        count,
        count,
    )
    .unwrap();
    let candidate =
        |active, id| LanguageParserHypothesis::new(active, id, 0, state.clone()).unwrap();
    let beam = LanguageParserAdmittedBeam::new(
        basis.clone(),
        candidate(true, 1),
        candidate(true, 2),
        candidate(false, 3),
        candidate(false, 4),
    )
    .unwrap();
    LanguageParserBeamCommitRequest::new(
        beam,
        cancelled,
        LanguageParserRawFrontier::new(basis, 0, count).unwrap(),
        pressure,
        through,
    )
    .unwrap()
}
pub fn result(
    prepared: &Prepared,
    request: &LanguageParserBeamCommitRequest,
) -> LanguageParserBeamCommitResult {
    LanguageParserBeamCommitResult::decode(&prepared.play(
        "language-parser-beam-commit",
        request.clone().encode().unwrap(),
    ))
    .unwrap()
}
