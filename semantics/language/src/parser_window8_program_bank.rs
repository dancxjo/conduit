//! Explicitly owned, allocating Reference evaluator program bank. No globals,
//! prepared evaluator subset, grammar policy, or raw-state admission shortcut.
use super::*;
use alloc::{collections::BTreeMap, vec::Vec};
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};

pub struct Window8ProgramBank {
    programs: BTreeMap<&'static str, PortableExpressionProgram>,
}
#[derive(Clone)]
pub struct Window8BankState {
    proof: LanguageParserWindow8StateProof,
}
impl Window8BankState {
    pub fn state(&self) -> &LanguageParserWindow8RawState {
        self.proof.state()
    }
    pub fn proof(&self) -> &LanguageParserWindow8StateProof {
        &self.proof
    }
}
pub struct Window8BankContext<'a> {
    bank: &'a Window8ProgramBank,
    prior: Window8BankState,
    seed: LanguageParserWindow8RawContext,
}
pub struct Window8BankProposal {
    prior: Window8BankState,
    proposal: LanguageParserWindow8RawResult,
}
impl Window8BankProposal {
    pub fn prior(&self) -> &Window8BankState {
        &self.prior
    }
    pub fn proposal(&self) -> &LanguageParserWindow8RawResult {
        &self.proposal
    }
}
pub struct Window8BankFeatures {
    state: Window8BankState,
    lexical: LanguageParserWindow8Lexical,
    query: LanguageParserWindow8RawFeatureQuery,
    features: LanguageParserWindow8Features,
}
impl Window8BankFeatures {
    pub fn state(&self) -> &Window8BankState {
        &self.state
    }
    pub fn lexical(&self) -> &LanguageParserWindow8Lexical {
        &self.lexical
    }
    pub fn query(&self) -> &LanguageParserWindow8RawFeatureQuery {
        &self.query
    }
    pub fn features(&self) -> &LanguageParserWindow8Features {
        &self.features
    }
}
impl Window8ProgramBank {
    pub fn prepare() -> Result<Self, Window8Refusal> {
        macro_rules! entries {
            ($($name:literal),* $(,)?) => { [$(($name,include_str!(concat!(env!("OUT_DIR"),"/",$name,".hex")))),*] };
        }
        let mut programs = BTreeMap::new();
        for (name, encoded) in entries!(
            "window8_initialize",
            "window8_walk_initialize",
            "window8_walk_follow",
            "window8_root_count",
            "window8_move_context",
            "window8_class_context",
            "window8_move_legal_shift",
            "window8_move_legal_reduce",
            "window8_move_legal_left",
            "window8_move_legal_right_root",
            "window8_move_legal_right_nonroot",
            "window8_move_apply",
            "window8_complete",
            "window8_class_index",
            "window8_class_relations",
            "window8_class_relation",
            "window8_rank_0_1",
            "window8_rank_2_3",
            "window8_rank_0_2",
            "window8_rank_1_3",
            "window8_rank_1_2",
            "window8_rank_insert",
            "window8_choice_frontier",
            "window8_score_advance",
            "window8_feature_context",
            "window8_feature_values"
        ) {
            programs.insert(
                name,
                PortableExpressionProgram::from_canonical_hex(encoded)
                    .map_err(|_| Window8Refusal::Program)?,
            );
        }
        Ok(Self { programs })
    }
    fn run<I: NativeRustBinding, O: NativeRustBinding>(
        &self,
        name: &str,
        input: I,
    ) -> Result<O, Window8Refusal> {
        let program = self.programs.get(name).ok_or(Window8Refusal::Program)?;
        let bytes = program
            .evaluate(&input.encode().map_err(Window8Refusal::Native)?)
            .map_err(|_| Window8Refusal::Program)?;
        O::decode(&bytes).map_err(Window8Refusal::Native)
    }
    pub fn admit_state(
        &self,
        state: &LanguageParserWindow8RawState,
    ) -> Result<Window8BankState, Window8Refusal> {
        let mut ancestry = Vec::with_capacity(8);
        for start in 0..8 {
            let query = LanguageParserWindow8WalkQuery::new(*state.heads(), start)
                .map_err(Window8Refusal::Native)?;
            let mut walk: LanguageParserWindow8RawWalk =
                self.run("window8_walk_initialize", query)?;
            for _ in 0..8 {
                walk = self.run("window8_walk_follow", walk)?;
            }
            ancestry.push(
                LanguageParserWindow8Ancestry::new(*walk.query().heads(), *walk.path(), start)
                    .map_err(Window8Refusal::Native)?,
            );
        }
        let roots: LanguageParserWindow8RootCount =
            self.run("window8_root_count", state.clone())?;
        let proof = LanguageParserWindow8StateProof::new(
            ancestry.try_into().map_err(|_| Window8Refusal::Program)?,
            *roots.count(),
            state.clone(),
        )
        .map_err(Window8Refusal::Native)?;
        Ok(Window8BankState { proof })
    }
    pub fn initialize(
        &self,
        begin: &LanguageParserWindow8Begin,
    ) -> Result<Window8BankState, Window8Refusal> {
        let raw = self.run("window8_initialize", begin.clone())?;
        self.admit_state(&raw)
    }
    pub fn class(
        &self,
        code: u64,
        relation: &LanguageParserRelation,
    ) -> Result<LanguageParserWindow8RawClass, Window8Refusal> {
        let query = LanguageParserWindow8ClassQuery::new(code, relation.clone())
            .map_err(Window8Refusal::Native)?;
        let index: LanguageParserWindow8RawClassIndex = self.run("window8_class_index", query)?;
        let relations: LanguageParserWindow8RawClassRelations =
            self.run("window8_class_relations", index)?;
        self.run("window8_class_relation", relations)
    }
    /// Raw ranking only: complete-body equality does not authorize a graph.
    pub fn rank(
        &self,
        mut beam: LanguageParserWindow8RawBeam,
    ) -> Result<LanguageParserWindow8RawBeam, Window8Refusal> {
        for name in [
            "window8_rank_0_1",
            "window8_rank_2_3",
            "window8_rank_0_2",
            "window8_rank_1_3",
            "window8_rank_1_2",
        ] {
            beam = self.run(name, beam)?;
        }
        Ok(beam)
    }
    pub fn merge(
        &self,
        beam: LanguageParserWindow8RawBeam,
        proposal: LanguageParserWindow8RawHypothesis,
    ) -> Result<LanguageParserWindow8RawBeam, Window8Refusal> {
        let query = LanguageParserWindow8RawMerge::new(self.rank(beam)?, proposal)
            .map_err(Window8Refusal::Native)?;
        self.rank(self.run("window8_rank_insert", query)?)
    }
    pub fn context<'a>(
        &'a self,
        prior: &Window8BankState,
        basis: &LanguageParserBasis,
    ) -> Result<Window8BankContext<'a>, Window8Refusal> {
        let top = prior.state().stack()[(*prior.state().depth() - 1) as usize];
        let witness = prior.proof().ancestry()[if top < 8 { top as usize } else { 0 }].clone();
        let request = LanguageParserWindow8RawRequest::new(
            LanguageParserAction::RightArc,
            basis.clone(),
            prior.state().relation0().clone(),
            prior.proof().clone(),
            witness,
        )
        .map_err(Window8Refusal::Native)?;
        let seed = self.run("window8_move_context", request)?;
        Ok(Window8BankContext {
            bank: self,
            prior: prior.clone(),
            seed,
        })
    }
    pub fn complete(&self, state: &Window8BankState) -> Result<bool, Window8Refusal> {
        let result: LanguageParserWindow8Completion =
            self.run("window8_complete", state.state().clone())?;
        Ok(*result.complete())
    }
    pub fn choice_frontier(
        &self,
        query: LanguageParserWindow8ChoiceQuery,
    ) -> Result<LanguageParserWindow8Selected, Window8Refusal> {
        self.run("window8_choice_frontier", query)
    }
    pub fn score_advance(
        &self,
        query: LanguageParserWindow8RawAdvance,
    ) -> Result<LanguageParserWindow8RawHypothesis, Window8Refusal> {
        self.run("window8_score_advance", query)
    }
    /// Full native lexical custody remains separate from compact feature data.
    pub fn features(
        &self,
        state: &Window8BankState,
        lexical: &lexical::PreparedWindow8Lexical,
        basis: &LanguageParserBasis,
        choices: [u64; 8],
    ) -> Result<Window8BankFeatures, Window8Refusal> {
        let query = LanguageParserWindow8RawFeatureQuery::new(
            choices,
            basis.clone(),
            lexical.projection().clone(),
            state.state().clone(),
        )
        .map_err(Window8Refusal::Native)?;
        let context: LanguageParserWindow8RawFeatureContext =
            self.run("window8_feature_context", query.clone())?;
        let raw = self.run("window8_feature_values", context)?;
        let features = LanguageParserWindow8Features::new(raw).map_err(Window8Refusal::Native)?;
        Ok(Window8BankFeatures {
            state: state.clone(),
            lexical: lexical.lexical().clone(),
            query,
            features,
        })
    }
}
impl Window8BankContext<'_> {
    pub fn prior(&self) -> &Window8BankState {
        &self.prior
    }
    pub fn propose(
        &self,
        class: &LanguageParserWindow8RawClass,
    ) -> Result<Window8BankProposal, Window8Refusal> {
        let query = LanguageParserWindow8RawClassContext::new(class.clone(), self.seed.clone())
            .map_err(Window8Refusal::Native)?;
        let mut context: LanguageParserWindow8RawContext =
            self.bank.run("window8_class_context", query)?;
        for name in [
            "window8_move_legal_shift",
            "window8_move_legal_reduce",
            "window8_move_legal_left",
            "window8_move_legal_right_root",
            "window8_move_legal_right_nonroot",
        ] {
            context = self.bank.run(name, context)?;
        }
        let proposal = self.bank.run("window8_move_apply", context)?;
        Ok(Window8BankProposal {
            prior: self.prior.clone(),
            proposal,
        })
    }
}
