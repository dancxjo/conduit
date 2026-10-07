//! Private custody used only by the actual retained model Session.
//! All semantic choice/arc policy stays in checked Source and Native laws.
use super::{fixture, joint, parser_joint_flows, parser_kernel};
use conduit_core::{semantic_digest, StructuredInfoValue};
use conduit_language::{parser_model_selection::PreparedParserModelSelection, *};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use std::sync::Arc;
pub fn append_source(base: String) -> String {
    [
        base,
        include_str!("../../parser_session_protection.conduit").into(),
        include_str!("../../parser_session_protected_set.conduit").into(),
        include_str!("../../parser_session_branch.conduit").into(),
        include_str!("../../parser_session_protected_branch.conduit").into(),
        include_str!("../../parser_session_protected_mask.conduit").into(),
        include_str!("../../parser_session_protected_rebase.conduit").into(),
        include_str!("../../parser_session_protected_forest.conduit").into(),
        include_str!("../../parser_session_dependency.conduit").into(),
        include_str!("../../parser_session_independent_admission.conduit").into(),
    ]
    .join("\n")
}
fn bind<T: NativeRustBinding>(
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<T, NativeBindingRefusal> {
    T::from_structured(fixture::record(&T::semantic_type()?, fields))
}
// This object has no public raw-set/fact constructor. Session owns actual
// model execution, then supplies its own Source-admitted stable facts.
pub struct Protection {
    _model: Arc<conduit_ai::integer_categorical_step::PreparedCategoricalStep>,
    expected_profile: LanguageLexicalProfile,
    source_identity: [u8; 32],
    flows: parser_joint_flows::Pipelines,
    origins: Vec<LanguageParserIndependentProtectedAdmission>,
    current: Option<LanguageParserProtectedSetProposal>,
    rebases: Vec<(
        LanguageParserProtectedSetRebaseContext,
        LanguageParserProtectedSetProposal,
    )>,
    pub refused: u64,
}
impl Protection {
    pub fn prepare(source: String, model: &PreparedParserModelSelection) -> Self {
        let source_identity = semantic_digest(
            "language/parser-v2-independent-protection-source@1",
            source.as_bytes(),
        );
        let entries = [
            "language-parser-protected-edge-projection",
            "language-parser-protected-set-initialize",
            "language-parser-protected-set-insert",
            "language-parser-protected-set-rebase",
            "language-parser-protection-forest-projection",
            "language-parser-independent-mask",
        ];
        let blueprints = entries
            .iter()
            .map(|entry| parser_kernel::Blueprint::prepare(source.clone(), entry))
            .collect::<Vec<_>>();
        Self {
            _model: model.prepared_categorical().clone(),
            expected_profile: model.expected_lexical_profile().clone(),
            source_identity,
            flows: parser_joint_flows::Pipelines::new(&blueprints, 1000),
            origins: Vec::new(),
            current: None,
            rebases: Vec::new(),
            refused: 0,
        }
    }
    pub fn origins(&self) -> &[LanguageParserIndependentProtectedAdmission] {
        &self.origins
    }
    pub fn current(&self) -> Option<&LanguageParserProtectedSetProposal> {
        self.current.as_ref()
    }
    pub fn source_identity(&self) -> [u8; 32] {
        self.source_identity
    }
    pub fn rebase_receipts(
        &self,
    ) -> &[(
        LanguageParserProtectedSetRebaseContext,
        LanguageParserProtectedSetProposal,
    )] {
        &self.rebases
    }
    pub fn initialize(&mut self, state: &LanguageParserState) {
        if self.current.is_some() {
            return;
        }
        // Opaque inactive payload, explicitly not a token/fact. Source sets every
        // active flag false because dependent4 is outside physical slots0..3.
        let ty = LanguageParserProtectedEdgeProposal::semantic_type().unwrap();
        let occurrence = LinguisticTokenIdentity::new(
            4,
            state.basis().text().clone(),
            state.basis().source_revision().clone(),
        )
        .unwrap()
        .into_structured()
        .unwrap();
        let edge: LanguageParserProtectedEdgeProposal = bind(vec![
            (
                "origin_basis",
                state.basis().clone().into_structured().unwrap(),
            ),
            (
                "current_basis",
                state.basis().clone().into_structured().unwrap(),
            ),
            (
                "dependent",
                fixture::number(fixture::field_type(&ty, "dependent"), 4),
            ),
            ("head", fixture::number(fixture::field_type(&ty, "head"), 4)),
            (
                "dependent_choice",
                fixture::number(fixture::field_type(&ty, "dependent_choice"), 0),
            ),
            (
                "head_choice",
                fixture::number(fixture::field_type(&ty, "head_choice"), 0),
            ),
            (
                "relation",
                state.relation0().clone().into_structured().unwrap(),
            ),
            ("dependent_occurrence", occurrence.clone()),
            ("head_occurrence", occurrence),
        ])
        .unwrap();
        let set = LanguageParserProtectedSetProposal::from_structured(
            self.flows.call(1, &edge.into_structured().unwrap()),
        )
        .unwrap();
        assert_eq!(*set.active(), [false; 4]);
        self.current = Some(set);
    }
    pub fn acquire(
        &mut self,
        fact: &LanguageParserJointStableFact,
    ) -> Result<(), NativeBindingRefusal> {
        assert_eq!(
            fact.query().beam().lexical().tape().profile(),
            &self.expected_profile
        );
        let beam = fact.query().beam();
        let dep = *fact.query().dependent() as usize;
        let state = beam.candidate0().parser().state();
        let head = state.heads()[dep];
        let dependent = beam.lexical().tape().tokens()[dep].clone();
        let head_token = if head < 4 {
            beam.lexical().tape().tokens()[head as usize].clone()
        } else {
            dependent.clone()
        };
        let context =
            LanguageParserProtectedProjectionContext::new(dependent, fact.clone(), head_token)?;
        let edge = LanguageParserProtectedEdgeProposal::from_structured(
            self.flows.call(0, &context.clone().into_structured()?),
        )?;
        // Initialization was Source-derived before first model input. No externally
        // supplied active flags can enter this private ledger.
        let previous = self.current.as_ref().unwrap().clone();
        let insert = LanguageParserProtectedInsertContext::new(context, previous)?;
        let output = LanguageParserProtectedSetProposal::from_structured(
            self.flows.call(2, &insert.clone().into_structured()?),
        )?;
        let token = |index: usize| {
            LanguageAnalysisTokenRef::new(
                beam.basis().analysis_revision().clone(),
                beam.lexical().tape().tokens()[index].identity().clone(),
            )
            .unwrap()
        };
        let governor = if head == 4 {
            LanguageDependencyHead::Root
        } else {
            let t = token(head as usize);
            LanguageDependencyHead::token(t.revision().clone(), t.token().clone()).unwrap()
        };
        let relation = [
            state.relation0(),
            state.relation1(),
            state.relation2(),
            state.relation3(),
        ][dep];
        let subtype = if relation.subtype().get().is_empty() {
            None
        } else {
            Some(LanguageDependencySubtype::new(
                relation.subtype().get().into(),
            )?)
        };
        let arc = LanguageDependencyArc::new(
            token(dep),
            governor,
            LanguageDependencyRelation::new(*relation.base(), subtype)?,
        )?;
        let ty = LanguageParserStableDependencyAdmission::semantic_type()?;
        let admission: LanguageParserStableDependencyAdmission = bind(vec![
            ("fact", fact.clone().into_structured()?),
            ("arc", arc.into_structured()?),
            (
                "head",
                fixture::number(fixture::field_type(&ty, "head"), head),
            ),
            ("subtype", relation.subtype().clone().into_structured()?),
        ])?;
        let retained: LanguageParserIndependentProtectedAdmission = bind(vec![
            ("admission", admission.into_structured()?),
            ("insert", insert.into_structured()?),
            ("output", output.clone().into_structured()?),
        ])?;
        // Native admission above authorizes the exact idempotent insert. Retain
        // the first full receipt for each Source slot, rather than duplicating
        // origin custody on every snapshot of the same admitted fact.
        if !self
            .origins
            .iter()
            .any(|origin| origin.admission().fact().query().dependent() == fact.query().dependent())
        {
            assert!(self.origins.len() < 4);
            self.origins.push(retained);
        }
        self.current = Some(output);
        let _ = edge;
        Ok(())
    }
    pub fn rebase(
        &mut self,
        rebase: &LanguageParserJointRebaseContext,
    ) -> Result<(), NativeBindingRefusal> {
        let Some(previous) = self.current.as_ref() else {
            return Ok(());
        };
        let tokens = rebase.next().tape().tokens();
        let at = |ordinal: usize| {
            tokens[if ordinal < tokens.len() { ordinal } else { 0 }]
                .clone()
                .into_structured()
                .unwrap()
        };
        let context: LanguageParserProtectedSetRebaseContext = bind(vec![
            ("previous", previous.clone().into_structured()?),
            ("rebase", rebase.clone().into_structured()?),
            ("token0", at(0)),
            ("token1", at(1)),
            ("token2", at(2)),
            ("token3", at(3)),
        ])?;
        let output = LanguageParserProtectedSetProposal::from_structured(
            self.flows.call(3, &context.clone().into_structured()?),
        )?;
        self.rebases.push((context, output.clone()));
        self.current = Some(output);
        Ok(())
    }
    pub fn branch(
        &mut self,
        query: LanguageParserJointBranchQuery,
    ) -> Result<StructuredInfoValue, NativeBindingRefusal> {
        let branch = LanguageParserJointProtectedBranchQuery::new(query)?;
        bind::<LanguageParserIndependentBranchContext>(vec![
            ("branch", branch.into_structured()?),
            (
                "retained",
                self.current.as_ref().unwrap().clone().into_structured()?,
            ),
        ])?
        .into_structured()
    }
    pub fn mask(
        &mut self,
        mask: LanguageParserLegalMask,
        default_relation: &LanguageParserRelation,
    ) -> Result<LanguageParserLegalMask, NativeBindingRefusal> {
        let query: LanguageParserIndependentMaskQuery = bind(vec![
            ("mask", mask.into_structured()?),
            (
                "retained",
                self.current.as_ref().unwrap().clone().into_structured()?,
            ),
            (
                "default_relation",
                default_relation.clone().into_structured()?,
            ),
        ])?;
        LanguageParserLegalMask::from_structured(self.flows.call(5, &query.into_structured()?))
    }
    pub fn admit_hypothesis(
        &mut self,
        hypothesis: &LanguageParserJointRuntimeHypothesis,
    ) -> Result<(), NativeBindingRefusal> {
        let query: LanguageParserProtectionForestQuery = bind(vec![
            ("hypothesis", hypothesis.clone().into_structured()?),
            (
                "retained",
                self.current.as_ref().unwrap().clone().into_structured()?,
            ),
        ])?;
        let proposal = self.flows.call(4, &query.clone().into_structured()?);
        let forest = LanguageParserProtectionForest::from_structured(joint::retype(
            &LanguageParserProtectionForest::semantic_type()?,
            &proposal,
        ))?;
        bind::<LanguageParserProtectedHypothesisCompatibility>(vec![
            ("query", query.into_structured()?),
            ("forest", forest.into_structured()?),
        ])?;
        Ok(())
    }
}
