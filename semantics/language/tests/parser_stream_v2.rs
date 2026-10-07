#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/parser_joint_flows.rs"]
mod parser_joint_flows;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_joint_v2_runtime.rs"]
mod runtime;
#[path = "common/scorer_model.rs"]
mod scorer_model;
use conduit_language::parser_model_selection::*;
use parser_joint_flows::Pipelines;
use std::io::Write;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
#[path = "common/parser_planned_runtime.rs"]
mod planned;
fn source() -> String {
    [
        joint::source(),
        include_str!("../parser_revision.conduit").into(),
        include_str!("../parser_session.conduit").into(),
    ]
    .join("\n")
}
struct Session {
    flows: Pipelines,
    previous: Option<LanguageParserAvailableState>,
    outcome: serde_json::Value,
    waits: u64,
    events: Option<std::fs::File>,
    started: std::time::Instant,
    model: PreparedParserModelSelection,
    numeric: planned::Execution,
    numeric_sequence: u64,
}
impl Session {
    fn new(profile: &LanguageLexicalProfile) -> Self {
        let categorical = model_resource::categorical(
            joint::BYTES.into(),
            pinned_v2_model_signature().unwrap(),
            1,
        );
        let model = PreparedParserModelSelection::prepare(categorical.clone(), profile).unwrap();
        let numeric = planned::prepare(categorical);
        let blueprints = [
            "language-parser-availability",
            "language-parser-revision-reset",
            "language-parser-joint-consensus",
            "language-parser-wait-state",
        ]
        .map(|entry| parser_kernel::Blueprint::prepare(source(), entry));
        Self {
            flows: Pipelines::new(&blueprints, 0),
            previous: None,
            outcome: serde_json::Value::Null,
            waits: 0,
            events: std::env::var("CONDUIT_PARSER_STREAM_EVENTS_OUTPUT")
                .ok()
                .map(|path| std::fs::File::create(path).unwrap()),
            started: std::time::Instant::now(),
            model,
            numeric,
            numeric_sequence: 0,
        }
    }
    fn emit(&mut self, event: serde_json::Value) {
        if let Some(file) = &mut self.events {
            serde_json::to_writer(&mut *file, &event).unwrap();
            file.write_all(b"\n").unwrap();
            file.flush().unwrap();
        }
    }
    fn available(&mut self, input: &LanguageParserAvailableLexical) -> serde_json::Value {
        let raw = self
            .flows
            .call(0, &input.clone().into_structured().unwrap());
        let status = LanguageParserAvailability::from_structured(joint::retype(
            &LanguageParserAvailability::semantic_type().unwrap(),
            &raw,
        ))
        .unwrap();
        let event = serde_json::json!({"event":"availability","actual_elapsed_ms":self.started.elapsed().as_millis(),"source_revision":input.tape().source().material().revision().get(),"source_sequence":input.tape().source().sequence(),"lexical_profile_identity":input.tape().profile().identity(),"waiting":status.waiting(),"final_input":status.final_input(),"available":input.token_count(),"revision_bytes":input.tape().source().clone().into_structured().unwrap().canonical_bytes().unwrap(),"invocation":self.flows.flows[0].sequence-1});
        self.emit(event.clone());
        event
    }
}
impl runtime::Observer for Session {
    fn initialize(
        &mut self,
        lexical: &LanguageParserJointLexical,
        initial: LanguageParserState,
    ) -> LanguageParserState {
        let available =
            LanguageParserAvailableLexical::new(lexical.tape().clone(), *lexical.token_count())
                .unwrap();
        let availability = self.available(&available);
        let Some(previous) = &self.previous else {
            self.outcome = serde_json::json!({"availability":availability,"reset":"initial"});
            return initial;
        };
        let lineage = prepare_text_revision_lineage(
            previous.lexical().tape().source(),
            lexical.tape().source(),
        )
        .unwrap();
        let input = LanguageParserRevisionContext::new(
            initial.basis().clone(),
            initial.relation0().clone(),
            lineage,
            available,
            previous.clone(),
        )
        .unwrap();
        let raw = self.flows.call(1, &input.into_structured().unwrap());
        assert!(fixture::accepted(&raw));
        let state = LanguageParserState::from_structured(joint::retype(
            &LanguageParserState::semantic_type().unwrap(),
            fixture::field(&raw, "state"),
        ))
        .unwrap();
        self.outcome = serde_json::json!({"availability":availability,"reset":"source-accepted-uncommitted","reset_invocation":self.flows.flows[1].sequence-1});
        state
    }
    fn waiting(
        &mut self,
        lexical: &LanguageParserJointLexical,
        state: &LanguageParserState,
    ) -> bool {
        let available =
            LanguageParserAvailableLexical::new(lexical.tape().clone(), *lexical.token_count())
                .unwrap();
        let input = LanguageParserAvailableState::new(available, state.clone()).unwrap();
        let raw = self.flows.call(3, &input.into_structured().unwrap());
        let status = LanguageParserWaitState::from_structured(joint::retype(
            &LanguageParserWaitState::semantic_type().unwrap(),
            &raw,
        ))
        .unwrap();
        self.waits += u64::from(*status.waiting());
        *status.waiting()
    }
    fn planned_scores(&mut self, features: &LanguageParserV2ModelFeatures) -> Option<Vec<i64>> {
        let scores = self.numeric.infer(
            self.numeric_sequence,
            &features.clone().into_structured().unwrap(),
        );
        self.numeric_sequence += 1;
        let conduit_core::StructuredInfoValueShape::Collection(values) = scores.shape() else {
            panic!("bare numerical scores")
        };
        Some(
            values
                .iter()
                .map(|value| {
                    conduit_plot::rust_binding::primitive_from_structured::<i64>(value).unwrap()
                })
                .collect(),
        )
    }
    fn snapshot(&mut self, runtime: &LanguageParserJointRuntimeBeam) -> serde_json::Value {
        let beam = runtime.beam();
        let mut agreed = Vec::new();
        for dependent in 0..*beam.lexical().token_count() {
            let input = LanguageParserJointConsensusQuery::new(beam.clone(), dependent).unwrap();
            let raw = self.flows.call(2, &input.into_structured().unwrap());
            let observation =
                LanguageParserJointConsensusObservation::from_structured(raw).unwrap();
            agreed.push(*observation.agreed());
        }
        let candidates = [beam.candidate0(),beam.candidate1(),beam.candidate2(),beam.candidate3()].map(|candidate| serde_json::json!({"active":candidate.parser().active(),"identity":candidate.parser().identity(),"score":candidate.parser().score(),"choices":candidate.choices(),"heads":candidate.parser().state().heads(),"unread":candidate.parser().state().unread(),"depth":candidate.parser().state().depth(),"relations":[format!("{:?}",candidate.parser().state().relation0().base()),format!("{:?}",candidate.parser().state().relation1().base()),format!("{:?}",candidate.parser().state().relation2().base()),format!("{:?}",candidate.parser().state().relation3().base())]}));
        let preferred = beam.candidate0().parser().state();
        self.previous = Some(
            LanguageParserAvailableState::new(
                LanguageParserAvailableLexical::new(
                    beam.lexical().tape().clone(),
                    *beam.lexical().token_count(),
                )
                .unwrap(),
                preferred.clone(),
            )
            .unwrap(),
        );
        let event = serde_json::json!({"event":"snapshot","model_execution":"ordinary-admitted-resource-Plan-Play","model_invocations":self.numeric_sequence,"model_content_identity":joint::hex(self.model.compatibility().model_content),"actual_elapsed_ms":self.started.elapsed().as_millis(),"source_revision":beam.basis().source_revision().get(),"analysis_revision":beam.basis().analysis_revision().get(),"source_sequence":beam.epoch(),"lexical_profile_identity":beam.lexical().tape().profile().identity(),"candidates":candidates,"outcome":self.outcome,"joint_lexical_arc_agreement":agreed,"beam_bytes":runtime.clone().into_structured().unwrap().canonical_bytes().unwrap(),"wait_calls":self.waits,"stable":0,"committed":preferred.committed(),"frontier_policy":"provisional snapshots only; agreement is observed, stabilization is not yet advanced","retained_executions":true,"flow_invocations":self.flows.flows.iter().map(|f|f.sequence).collect::<Vec<_>>()});
        self.emit(event.clone());
        event
    }
}
#[test]
#[ignore = "actual retained partial-source learned stream evidence; cold finite Plan preparation measured"]
fn later_complete_words_revise_provisional_parse_in_retained_flows() {
    let template = joint::Sentence {
        id: "stream-profile".into(),
        forms: vec!["Travis".into(), "Hello".into()],
        pos: vec![11, 6],
        heads: vec![1, 4],
        relations: vec!["vocative".into(), "root".into()],
        text: None,
    };
    let reference = joint::lexical(&template).unwrap();
    let profile = reference.tape().profile();
    let mut prepared: Option<PreparedLexicalTape> = None;
    let mut history = Vec::new();
    let mut rows = Vec::new();
    let mut inputs = Vec::new();
    let mut session = Session::new(profile);
    for (sequence, text, finality, stable) in [
        (0, "Tr", LanguageTextFinality::Partial, None),
        (1, "Travis ", LanguageTextFinality::Partial, Some(7)),
        (2, "Travis Hello ", LanguageTextFinality::Partial, Some(13)),
        (3, "Travis Hello ", LanguageTextFinality::Final, Some(13)),
    ] {
        let prior = prepared.as_ref().map(|old| {
            LanguageTextPriorRevision::new(
                old.tape().source().material().revision().clone(),
                *old.tape().source().sequence(),
            )
            .unwrap()
        });
        let source = LanguageTextRevision::new(
            finality,
            LanguageText::new(
                LanguageTextId::new("stream/v2/travis-hello".into()).unwrap(),
                profile.language().clone(),
                LanguageTextRevisionId::new(format!("stream/{sequence}")).unwrap(),
                text.into(),
            )
            .unwrap(),
            prior,
            profile.provenance().clone(),
            sequence,
            stable,
        )
        .unwrap();
        let next = prepare_lexical_tape(&source, profile, prepared.as_ref()).unwrap();
        history.push(
            source
                .clone()
                .into_structured()
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
        if sequence == 0 {
            let available = LanguageParserAvailableLexical::new(next.tape().clone(), 0).unwrap();
            let waiting = session.available(&available);
            assert_eq!(waiting["waiting"], true);
            eprintln!("STREAM_INITIAL_WAIT {waiting}");
        } else {
            let forms = if sequence == 1 {
                vec!["Travis".into()]
            } else {
                template.forms.clone()
            };
            let row = joint::Sentence {
                id: format!("retained-stream/{sequence}"),
                forms,
                pos: if sequence == 1 {
                    vec![11]
                } else {
                    template.pos.clone()
                },
                heads: if sequence == 1 {
                    vec![4]
                } else {
                    template.heads.clone()
                },
                relations: if sequence == 1 {
                    vec!["root".into()]
                } else {
                    template.relations.clone()
                },
                text: Some(text.into()),
            };
            let count = row.forms.len() as u64;
            inputs.push(Ok(LanguageParserJointLexical::new(
                next.tape().clone(),
                count,
            )
            .unwrap()));
            rows.push(row);
        }
        prepared = Some(next);
    }
    runtime::evaluate_observed(Some("retained-stream"), rows, inputs, &mut session);
    assert_eq!(session.flows.flows[0].sequence, 4);
    assert_eq!(session.flows.flows[1].sequence, 2);
    assert!(session.waits > 0);
    if let Ok(path) = std::env::var("CONDUIT_PARSER_STREAM_HISTORY_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&history).unwrap()).unwrap();
    }
}
