use super::{fixture, joint, model_resource, parser_joint_flows, parser_kernel, planned, runtime};
use conduit_language::{parser_model_selection::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
use parser_joint_flows::Pipelines;
use std::io::Write;
pub fn source() -> String {
    [
        joint::source(),
        include_str!("../../parser_revision.conduit").into(),
        include_str!("../../parser_session.conduit").into(),
        include_str!("../../parser_session_policy.conduit").into(),
        include_str!("../../parser_session_facts.conduit").into(),
        include_str!("../../parser_session_commit.conduit").into(),
        include_str!("../../parser_session_rebase.conduit").into(),
    ]
    .join("\n")
}
pub struct Session {
    pub flows: Pipelines,
    previous: Option<LanguageParserAvailableState>,
    outcome: serde_json::Value,
    pub waits: u64,
    pub committed_snapshots: Vec<u64>,
    events: Option<std::fs::File>,
    started: std::time::Instant,
    model: PreparedParserModelSelection,
    numeric: planned::Execution,
    numeric_sequence: u64,
    policy: bool,
    choices: [u64; 4],
    seed_choices: [u64; 4],
    seed_selected: u64,
    retained_facts: Vec<LanguageParserJointStableFact>,
    branch_refusals: u64,
}
impl Session {
    pub fn new(profile: &LanguageLexicalProfile) -> Self {
        Self::prepare(profile, false)
    }
    pub fn stable(profile: &LanguageLexicalProfile) -> Self {
        Self::prepare(profile, true)
    }
    #[allow(dead_code)]
    pub fn retained_facts(&self) -> &[LanguageParserJointStableFact] {
        &self.retained_facts
    }
    pub fn committed(&self) -> u64 {
        self.previous
            .as_ref()
            .map_or(0, |state| *state.state().committed())
    }
    fn prepare(profile: &LanguageLexicalProfile, policy: bool) -> Self {
        let started = std::time::Instant::now();
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
            "language-parser-joint-score-band-1000",
            "language-parser-joint-stable-fact",
            "language-parser-joint-commit",
            "language-parser-joint-rebase",
        ];
        let blueprints = blueprints[..if policy { 8 } else { 4 }]
            .iter()
            .map(|entry| parser_kernel::Blueprint::prepare(source(), entry))
            .collect::<Vec<_>>();
        Self {
            flows: Pipelines::new(&blueprints, 0),
            previous: None,
            outcome: serde_json::Value::Null,
            waits: 0,
            committed_snapshots: Vec::new(),
            events: std::env::var("CONDUIT_PARSER_STREAM_EVENTS_OUTPUT")
                .ok()
                .map(|path| std::fs::File::create(path).unwrap()),
            started,
            model,
            numeric,
            numeric_sequence: 0,
            policy,
            choices: [0; 4],
            seed_choices: [0; 4],
            seed_selected: 0,
            retained_facts: Vec::new(),
            branch_refusals: 0,
        }
    }
    fn emit(&mut self, event: serde_json::Value) {
        if let Some(file) = &mut self.events {
            serde_json::to_writer(&mut *file, &event).unwrap();
            file.write_all(b"\n").unwrap();
            file.flush().unwrap();
        }
    }
    fn stabilize(
        &mut self,
        runtime: &LanguageParserJointRuntimeBeam,
    ) -> (
        LanguageParserJointRuntimeBeam,
        Vec<LanguageParserJointStableFact>,
    ) {
        let query = LanguageParserJointScoreBandQuery::new(runtime.clone()).unwrap();
        let raw = self.flows.call(4, &query.into_structured().unwrap());
        let slots = LanguageParserJointRuntimeRawBeam::from_structured(joint::retype(
            &LanguageParserJointRuntimeRawBeam::semantic_type().unwrap(),
            &raw,
        ))
        .unwrap();
        let mut current = joint::admit(
            &slots,
            runtime.beam().lexical(),
            runtime.beam().basis(),
            *runtime.beam().invocation(),
        );
        let mut facts = Vec::new();
        for dependent in 0..*current.beam().lexical().token_count() {
            let query =
                LanguageParserJointConsensusQuery::new(current.beam().clone(), dependent).unwrap();
            let raw = self.flows.call(5, &query.into_structured().unwrap());
            let proposal = LanguageParserJointStableFactProposal::from_structured(raw).unwrap();
            let fact = match LanguageParserJointStableFact::from_structured(joint::retype(
                &LanguageParserJointStableFact::semantic_type().unwrap(),
                &proposal.into_structured().unwrap(),
            )) {
                Ok(fact) => fact,
                Err(conduit_plot::rust_binding::NativeBindingRefusal::ViolatedInvariant {
                    index,
                }) => {
                    self.emit(serde_json::json!({"event":"stable-fact-refused","source_revision":current.beam().basis().source_revision().get(),"dependent":dependent,"Source_law_index":index}));
                    continue;
                }
                Err(error) => panic!("invalid Source stable admission: {error:?}"),
            };
            self.retained_facts.push(fact.clone());
            facts.push(fact.clone());
            let query = match LanguageParserJointCommitQuery::new(fact) {
                Ok(query) => query,
                Err(conduit_plot::rust_binding::NativeBindingRefusal::ViolatedInvariant {
                    index,
                }) => {
                    self.emit(serde_json::json!({"event":"contiguous-commit-refused","source_revision":current.beam().basis().source_revision().get(),"dependent":dependent,"Source_law_index":index}));
                    continue;
                }
                Err(error) => panic!("invalid Source commit admission: {error:?}"),
            };
            let raw = self.flows.call(6, &query.into_structured().unwrap());
            let hypotheses = ["candidate0", "candidate1", "candidate2", "candidate3"].map(|name| {
                LanguageParserJointHypothesis::from_structured(joint::retype(
                    &LanguageParserJointHypothesis::semantic_type().unwrap(),
                    fixture::field(&raw, name),
                ))
                .unwrap()
            });
            let selected = current.selected();
            let candidates = core::array::from_fn::<_, 4, _>(|index| {
                LanguageParserJointRuntimeHypothesis::new(
                    hypotheses[index].clone(),
                    selected[index],
                )
                .unwrap()
            });
            let slots = LanguageParserJointRuntimeRawBeam::new(
                candidates[0].clone(),
                candidates[1].clone(),
                candidates[2].clone(),
                candidates[3].clone(),
            )
            .unwrap();
            current = joint::admit(
                &slots,
                current.beam().lexical(),
                current.beam().basis(),
                *current.beam().invocation(),
            );
            self.emit(serde_json::json!({"event":"dependency-commit","source_revision":current.beam().basis().source_revision().get(),"analysis_revision":current.beam().basis().analysis_revision().get(),"committed":current.beam().candidate0().parser().state().committed(),"native_runtime_bytes":current.clone().into_structured().unwrap().canonical_bytes().unwrap(),"entry":"language-parser-joint-commit"}));
        }
        (current, facts)
    }
    pub fn available(&mut self, input: &LanguageParserAvailableLexical) -> serde_json::Value {
        let raw = self
            .flows
            .call(0, &input.clone().into_structured().unwrap());
        let status = LanguageParserAvailability::from_structured(joint::retype(
            &LanguageParserAvailability::semantic_type().unwrap(),
            &raw,
        ))
        .unwrap();
        let event = serde_json::json!({"event":"availability","text":input.tape().source().material().text(),"actual_elapsed_ms":self.started.elapsed().as_millis(),"source_revision":input.tape().source().material().revision().get(),"source_sequence":input.tape().source().sequence(),"lexical_profile_identity":input.tape().profile().identity(),"waiting":status.waiting(),"final_input":status.final_input(),"available":input.token_count(),"revision_bytes":input.tape().source().clone().into_structured().unwrap().canonical_bytes().unwrap(),"invocation":self.flows.flows[0].sequence-1});
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
        if self.policy {
            let input = LanguageParserJointRebaseContext::new(
                initial.basis().clone(),
                self.choices,
                initial.relation0().clone(),
                lineage,
                available,
                previous.clone(),
            )
            .unwrap();
            let input_bytes = input
                .clone()
                .into_structured()
                .unwrap()
                .canonical_bytes()
                .unwrap();
            if *previous.state().committed() > 0 {
                assert!(
                    LanguageParserJointRebaseContext::new(
                        input.basis().clone(),
                        [99; 4],
                        input.default_relation().clone(),
                        input.lineage().clone(),
                        input.next().clone(),
                        input.previous().clone(),
                    )
                    .is_err(),
                    "Source refuses forged protected choices"
                );
            }
            let raw = self.flows.call(7, &input.into_structured().unwrap());
            let proposal = LanguageParserJointRebaseProposal::from_structured(raw).unwrap();
            self.seed_choices = *proposal.choices();
            self.seed_selected = *proposal.selected();
            let state = LanguageParserState::from_structured(joint::retype(
                &LanguageParserState::semantic_type().unwrap(),
                &proposal.state().clone().into_structured().unwrap(),
            ))
            .unwrap();
            for dependent in 0..*previous.state().committed() as usize {
                assert_eq!(
                    state.heads()[dependent],
                    previous.state().heads()[dependent]
                );
                assert_eq!(self.seed_choices[dependent], self.choices[dependent]);
                let head = previous.state().heads()[dependent] as usize;
                if head < 4 {
                    assert_eq!(self.seed_choices[head], self.choices[head]);
                }
            }
            self.outcome = serde_json::json!({"rebase_context_bytes":input_bytes,"rebase_state_bytes":state.clone().into_structured().unwrap().canonical_bytes().unwrap(),"availability":availability,"rebase":"Source finite4 protected root spine","previous_committed":previous.state().committed(),"current_committed":state.committed(),"rebase_invocation":self.flows.flows[7].sequence-1});
            return state;
        }
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
    fn seed_metadata(&self) -> ([u64; 4], u64) {
        (self.seed_choices, self.seed_selected)
    }
    fn branch_entry(&self) -> Option<(&'static str, String)> {
        self.policy.then(|| {
            (
                "language-parser-joint-protected-branch",
                include_str!("../../parser_session_branch.conduit").into(),
            )
        })
    }
    fn branch_input(
        &mut self,
        query: LanguageParserJointBranchQuery,
    ) -> Result<conduit_core::StructuredInfoValue, conduit_plot::rust_binding::NativeBindingRefusal>
    {
        if !self.policy {
            return query.into_structured();
        }
        let result = LanguageParserJointProtectedBranchQuery::new(query)
            .and_then(|query| query.into_structured());
        if let Err(error) = &result {
            assert!(
                matches!(
                    error,
                    conduit_plot::rust_binding::NativeBindingRefusal::ViolatedInvariant { .. }
                ),
                "unexpected Source branch admission error: {error:?}"
            );
            self.branch_refusals += 1;
            self.emit(serde_json::json!({"event":"Source-policy-refusal","entry":"language-parser-joint-protected-branch","refusal":format!("{error:?}"),"ordinal":self.branch_refusals}));
        }
        result
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
        let (owned, stable_facts) = if self.policy {
            self.stabilize(runtime)
        } else {
            (runtime.clone(), Vec::new())
        };
        let beam = owned.beam();
        self.choices = *beam.candidate0().choices();
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
        self.committed_snapshots.push(*preferred.committed());
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
        let event = serde_json::json!({"event":"snapshot","text":beam.lexical().tape().source().material().text(),"preferred_candidate":beam.candidate0().parser().active().then_some(beam.candidate0().parser().identity()),"preferred_selection":"Source cumulative score and identity rank","model_execution":"ordinary-admitted-resource-Plan-Play","model_invocations":self.numeric_sequence,"model_content_identity":joint::hex(self.model.compatibility().model_content),"model_signature_identity":joint::hex(self.model.compatibility().signature),"session_source_identity":joint::hex(conduit_core::semantic_digest("language/parser-v2-retained-session-source@1",source().as_bytes())),"actual_elapsed_ms":self.started.elapsed().as_millis(),"source_revision":beam.basis().source_revision().get(),"analysis_revision":beam.basis().analysis_revision().get(),"source_sequence":beam.epoch(),"lexical_profile_identity":beam.lexical().tape().profile().identity(),"candidates":candidates,"outcome":self.outcome,"joint_lexical_arc_agreement":agreed,"beam_bytes":owned.clone().into_structured().unwrap().canonical_bytes().unwrap(),"wait_calls":self.waits,"stable":stable_facts.len(),"stable_fact_bytes":stable_facts.iter().map(|fact|fact.clone().into_structured().unwrap().canonical_bytes().unwrap()).collect::<Vec<_>>(),"retained_stable_fact_count":self.retained_facts.len(),"branch_policy_refusals":self.branch_refusals,"committed":preferred.committed(),"frontier_policy":if self.policy {"Source uncalibrated score-band1000; Native stable laws and contiguous commit"}else{"provisional snapshots only; stabilization not advanced"},"retained_executions":true,"flow_invocations":self.flows.flows.iter().map(|f|f.sequence).collect::<Vec<_>>()});
        self.emit(event.clone());
        event
    }
}
