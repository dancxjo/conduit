//! Existing SourceGraph supplies actual checked Plan/Play; no score/model owner.
use super::{causal_prefix_inputs::*, parser_selection_io::*, parser_selection_values::*};
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduitos::protocol_source::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub struct Graphs {
    graphs: BTreeMap<&'static str, SourceGraph>,
    pub identities: Value,
}
impl Graphs {
    pub fn prepare() -> Self {
        let raw = language_source();
        let package = ProtocolSourcePackage::compile(raw.clone(), &[]).unwrap();
        let artifact = conduit_core::ArtifactId::from(format!(
            "language/causal-source-parity/{}",
            digest(&serde_json::to_vec(&package).unwrap())
        ));
        let prepared = PreparedProtocolSource::prepare(package).unwrap();
        let mut graphs = BTreeMap::new();
        let mut entries = BTreeMap::new();
        for entry in [
            "language-parser-initialize",
            "language-parser-revision-rebase",
            "language-parser-score-proposal",
            "language-parser-transition",
            "language-parser-scorer-model-features",
            "language-parser-decode-complete",
        ] {
            let expansion = prepared.expand(entry).unwrap();
            entries.insert(entry,json!({"source":expansion.expanded.source_document_id,"checked":expansion.expanded.checked_plot_id,"expanded":expansion.expanded.expanded_plot_id,"programs":expansion.expanded.gears.iter().map(|g|&g.configuration).collect::<Vec<_>>()}));
            graphs.insert(entry, SourceGraph::from_source(&prepared, entry, &artifact));
        }
        Self {
            graphs,
            identities: json!({"source_semantic_digest":digest(raw.as_bytes()),"source_document":conduit_plot::parse_syntax_document(&raw).source_document_id(),"artifact":artifact,"entries":entries}),
        }
    }
    pub fn invoke<T: NativeRustBinding, U: NativeRustBinding>(
        &mut self,
        store: &mut Store,
        entry: &'static str,
        input: T,
    ) -> (U, Value) {
        let input = input.into_structured().unwrap();
        let input_id = store.blob(&input.canonical_bytes().unwrap());
        let graph = self.graphs.get_mut(entry).unwrap();
        let output = graph.run_value(&input);
        let output_id = store.blob(&output.canonical_bytes().unwrap());
        let evidence = json!({"entry":entry,"sequence":graph.invocations()-1,"input":input_id,"output":output_id});
        store.observation(evidence.clone());
        (U::from_structured(output).unwrap(), evidence)
    }
    pub fn complete(&mut self, state: &LanguageParserState) -> bool {
        self.graphs
            .get_mut("language-parser-decode-complete")
            .unwrap()
            .run_bool(state.clone())
    }
}
pub fn profile(value: &Value) -> LanguageLexicalProfile {
    let entries = value.as_object().unwrap().iter().map(|(surface, codes)| {
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter(codes.as_array().unwrap().iter().map(|code| {
                LanguageLexicalCandidate::new(surface.clone(), BoundedSequence::new(), pos(u(code)))
                    .unwrap()
            }))
            .unwrap(),
            surface.clone(),
        )
        .unwrap()
    });
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        digest(&serde_json::to_vec(value).unwrap()),
        LanguageId::new("language/en".into()).unwrap(),
        LinguisticDerivationProvenance::deterministic_rule(
            "supervised-causal-train-schedule@1".into(),
            "1".into(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn available(lexical: &PreparedLexicalTape, n: usize) -> LanguageParserAvailableLexical {
    assert_eq!(lexical.tape().tokens().len(), n);
    LanguageParserAvailableLexical::new(lexical.tape().clone(), n as u64).unwrap()
}
pub fn numeric(state: &LanguageParserState) -> LanguageParserNumericState {
    LanguageParserNumericState::new(
        state.basis().clone(),
        *state.committed(),
        *state.depth(),
        *state.heads(),
        state.relation0().clone(),
        state.relation1().clone(),
        state.relation2().clone(),
        state.relation3().clone(),
        *state.stack(),
        *state.token_count(),
        *state.unread(),
    )
    .unwrap()
}
pub fn transition(
    graphs: &mut Graphs,
    store: &mut Store,
    state: &LanguageParserState,
    class: u64,
) -> (LanguageParserResult, Value) {
    let scored =
        LanguageParserScoredClass::new(state.basis().clone(), class, dep(), 0, numeric(state))
            .unwrap();
    let (proposal, proposal_evidence): (LanguageParserScoredProposal, _) =
        graphs.invoke(store, "language-parser-score-proposal", scored);
    let (result, result_evidence) = graphs.invoke(
        store,
        "language-parser-transition",
        proposal.request().clone(),
    );
    (
        result,
        json!({"proposal":proposal_evidence,"transition":result_evidence}),
    )
}
pub struct Arrival {
    pub lexical: PreparedLexicalTape,
    pub state: LanguageParserState,
}
pub fn arrival(
    graphs: &mut Graphs,
    store: &mut Store,
    sentence: &Value,
    n: usize,
    full: bool,
    prior: Option<&Arrival>,
    profile: &LanguageLexicalProfile,
) -> Arrival {
    let (source, analysis) =
        source_revision(sentence, n, full, prior.map(|v| v.lexical.tape().source()));
    if let Some(prior) = prior {
        let rebase = PreparedParserRevisionRebase::prepare(
            &prior.state,
            &prior.lexical,
            &source,
            analysis,
            dep(),
        )
        .unwrap();
        let (raw, _): (LanguageParserNumericState, _) = graphs.invoke(
            store,
            "language-parser-revision-rebase",
            rebase.request().clone(),
        );
        let lexical = prepare_lexical_tape(&source, profile, Some(&prior.lexical)).unwrap();
        assert_eq!(lexical.tape(), rebase.lexical().tape());
        let state = admit_state(raw);
        assert_eq!(*state.committed(), 0);
        assert_eq!(*state.unread(), 0);
        assert_eq!(*state.depth(), 1);
        assert_eq!(*state.heads(), [5, 5, 5, 5, 4]);
        Arrival { lexical, state }
    } else {
        let lexical = prepare_lexical_tape(&source, profile, None).unwrap();
        let available = available(&lexical, n);
        let basis = LanguageParserBasis::new(
            analysis,
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap();
        let begin = LanguageParserBegin::new(basis, dep(), *available.token_count()).unwrap();
        let (raw, _) = graphs.invoke(store, "language-parser-initialize", begin);
        Arrival {
            lexical,
            state: admit_state(raw),
        }
    }
}

pub fn source_revision(
    sentence: &Value,
    n: usize,
    full: bool,
    prior: Option<&LanguageTextRevision>,
) -> (LanguageTextRevision, LanguageAnalysisRevisionId) {
    let words = sentence["forms"].as_array().unwrap();
    let content = words[..n].iter().map(text).collect::<Vec<_>>().join(" ") + " ";
    let id = digest(text(sentence.get("sentence_id").unwrap()).as_bytes());
    let arm = if full { "full" } else { "causal" };
    let sequence = prior.map_or(0, |v| *v.sequence() + 1);
    let source = revision_with_text(
        &content,
        &format!("{arm}/{}", &id[..16]),
        &format!("{arm}/{}/{n}", &id[..16]),
        sequence,
        prior,
        Some(content.chars().count().try_into().unwrap()),
        LanguageTextFinality::Partial,
    );
    let analysis = LanguageAnalysisRevisionId::new(format!("{arm}/{}/{n}", &id[..16])).unwrap();
    (source, analysis)
}

/// Real lexical owner/Native admission preflight; never retokenize offline labels.
pub fn lexical_preflight(
    store: &mut Store,
    rows: &[Value],
    profile: &LanguageLexicalProfile,
) -> Vec<Value> {
    let mut sentences = BTreeMap::new();
    for row in rows {
        sentences.entry(text(&row["sentence_id"])).or_insert(row);
    }
    let mut quarantined = Vec::new();
    for (id, row) in sentences {
        let mut prior: Option<PreparedLexicalTape> = None;
        let n = u(&row["full_count"]) as usize;
        for p in 1..=n {
            let (source, _) =
                source_revision(row, p, false, prior.as_ref().map(|v| v.tape().source()));
            let lexical = match prepare_lexical_tape(&source, profile, prior.as_ref()) {
                Ok(lexical) => lexical,
                Err(error) => {
                    let source_blob = store.blob(&source.clone().encode().unwrap());
                    store.write("unexpected-lexical-refusal.json",&json!({"sentence_id":id,"prefix":p,"reason":"unexpected_lexical_preparation_refusal","detail":format!("{error:?}"),"source":source_blob,"fit_eligible":false}));
                    panic!("unexpected lexical refusal is not an exclusion policy: {id} prefix{p}: {error:?}");
                }
            };
            let expected: Vec<_> = row["forms"].as_array().unwrap()[..p]
                .iter()
                .map(text)
                .collect();
            let actual: Vec<_> = lexical
                .tape()
                .tokens()
                .iter()
                .map(|token| token.surface().as_str())
                .collect();
            if actual != expected {
                quarantined.push(json!({"sentence_id":id,"prefix":p,"reason":"actual_source_occurrence_segmentation_differs_from_offline_labels","expected_surfaces":expected,"actual_surfaces":actual,"source":store.blob(&source.encode().unwrap()),"lexical":store.blob(&lexical.tape().clone().encode().unwrap()),"quarantine_scope":"whole sentence/all paired targets; no silent subset fitting"}));
                break;
            }
            if LanguageParserAvailableLexical::new(lexical.tape().clone(), p as u64).is_err() {
                let lexical_blob = store.blob(&lexical.tape().clone().encode().unwrap());
                store.write("unexpected-availability-refusal.json",&json!({"sentence_id":id,"prefix":p,"reason":"unexpected_actual_available_prefix_refusal","lexical":lexical_blob,"fit_eligible":false}));
                panic!(
                    "unexpected availability refusal is not an exclusion policy: {id} prefix{p}"
                );
            }
            prior = Some(lexical);
        }
    }
    quarantined
}
