//! Private custody of an exact native forest proof. Rust schedules the authored
//! fixed traversal; Source predicates own graph/frontier admission.
use crate::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum Window8Refusal {
    Native(NativeBindingRefusal),
    Program,
}
#[derive(Clone)]
pub struct PreparedWindow8State {
    proof: LanguageParserWindow8StateProof,
}
impl PreparedWindow8State {
    pub fn state(&self) -> &LanguageParserWindow8RawState {
        self.proof.state()
    }
    pub fn proof(&self) -> &LanguageParserWindow8StateProof {
        &self.proof
    }
}
fn evaluate<I: NativeRustBinding, O: NativeRustBinding>(
    input: I,
    encoded: &str,
) -> Result<O, Window8Refusal> {
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|_| Window8Refusal::Program)?;
    let bytes = program
        .evaluate(&input.encode().map_err(Window8Refusal::Native)?)
        .map_err(|_| Window8Refusal::Program)?;
    O::decode(&bytes).map_err(Window8Refusal::Native)
}
pub fn prepare_window8_state(
    state: &LanguageParserWindow8RawState,
) -> Result<PreparedWindow8State, Window8Refusal> {
    let mut ancestry = alloc::vec::Vec::with_capacity(8);
    for start in 0..8 {
        let query = LanguageParserWindow8WalkQuery::new(*state.heads(), start)
            .map_err(Window8Refusal::Native)?;
        let mut walk: LanguageParserWindow8RawWalk = evaluate(
            query,
            include_str!(concat!(env!("OUT_DIR"), "/window8_walk_initialize.hex")),
        )?;
        for _ in 0..8 {
            walk = evaluate(
                walk,
                include_str!(concat!(env!("OUT_DIR"), "/window8_walk_follow.hex")),
            )?;
        }
        let witness = LanguageParserWindow8Ancestry::new(
            *walk.query().heads(),
            *walk.path(),
            *walk.query().start(),
        )
        .map_err(Window8Refusal::Native)?;
        ancestry.push(witness);
    }
    let roots: LanguageParserWindow8RootCount = evaluate(
        state.clone(),
        include_str!(concat!(env!("OUT_DIR"), "/window8_root_count.hex")),
    )?;
    let proof = LanguageParserWindow8StateProof::new(
        ancestry.try_into().map_err(|_| Window8Refusal::Program)?,
        *roots.count(),
        state.clone(),
    )
    .map_err(Window8Refusal::Native)?;
    Ok(PreparedWindow8State { proof })
}
pub fn initialize_window8(
    begin: &LanguageParserWindow8Begin,
) -> Result<PreparedWindow8State, Window8Refusal> {
    let raw = evaluate(
        begin.clone(),
        include_str!(concat!(env!("OUT_DIR"), "/window8_initialize.hex")),
    )?;
    prepare_window8_state(&raw)
}
pub fn window8_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageParserWindow8CheckedHypothesis",
            LanguageParserWindow8CheckedHypothesis::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Snapshot",
            LanguageParserWindow8Snapshot::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8FactQuery",
            LanguageParserWindow8FactQuery::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8StableLexicalFact",
            LanguageParserWindow8StableLexicalFact::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8StableLexicalFactProposal",
            LanguageParserWindow8StableLexicalFactProposal::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawClassContext",
            LanguageParserWindow8RawClassContext::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawClassRelations",
            LanguageParserWindow8RawClassRelations::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8ChoiceQuery",
            LanguageParserWindow8ChoiceQuery::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Selected",
            LanguageParserWindow8Selected::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawAdvance",
            LanguageParserWindow8RawAdvance::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawHypothesis",
            LanguageParserWindow8RawHypothesis::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawBeam",
            LanguageParserWindow8RawBeam::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawMerge",
            LanguageParserWindow8RawMerge::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8ClassQuery",
            LanguageParserWindow8ClassQuery::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawClassIndex",
            LanguageParserWindow8RawClassIndex::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawClass",
            LanguageParserWindow8RawClass::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawRequest",
            LanguageParserWindow8RawRequest::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawContext",
            LanguageParserWindow8RawContext::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawResult",
            LanguageParserWindow8RawResult::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Completion",
            LanguageParserWindow8Completion::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Available",
            LanguageParserWindow8Available::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Lexical",
            LanguageParserWindow8Lexical::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8CodeQuery",
            LanguageParserWindow8CodeQuery::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Ordinal",
            LanguageParserWindow8Ordinal::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawCodes",
            LanguageParserWindow8RawCodes::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawProjection",
            LanguageParserWindow8RawProjection::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawFeatureQuery",
            LanguageParserWindow8RawFeatureQuery::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawFeatureContext",
            LanguageParserWindow8RawFeatureContext::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawModelFeatures",
            LanguageParserWindow8RawModelFeatures::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Features",
            LanguageParserWindow8Features::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawState",
            LanguageParserWindow8RawState::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Ancestry",
            LanguageParserWindow8Ancestry::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8StateProof",
            LanguageParserWindow8StateProof::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8Begin",
            LanguageParserWindow8Begin::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8WalkQuery",
            LanguageParserWindow8WalkQuery::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RawWalk",
            LanguageParserWindow8RawWalk::semantic_type().unwrap()
        ),
        (
            "LanguageParserWindow8RootCount",
            LanguageParserWindow8RootCount::semantic_type().unwrap()
        ),
    ]
}

#[path = "parser_window8_lexical.rs"]
pub mod lexical;

pub struct PreparedWindow8Step {
    prior: LanguageParserWindow8StateProof,
    proposal: LanguageParserWindow8RawResult,
    next: PreparedWindow8State,
}
impl PreparedWindow8Step {
    pub fn prior(&self) -> &LanguageParserWindow8StateProof {
        &self.prior
    }
    pub fn proposal(&self) -> &LanguageParserWindow8RawResult {
        &self.proposal
    }
    pub fn next(&self) -> &PreparedWindow8State {
        &self.next
    }
    pub fn accepted(&self) -> bool {
        *self.proposal.accepted()
    }
}
/// Source owns basis/action guards and mutation; next forest admission is
/// mandatory. On any refusal the borrowed prior immutable snapshot survives.
pub fn prepare_window8_proposal(
    prior: &PreparedWindow8State,
    basis: &LanguageParserBasis,
    action: LanguageParserAction,
    relation: &LanguageParserRelation,
) -> Result<PreparedWindow8Proposal, Window8Refusal> {
    let top = prior.state().stack()[(*prior.state().depth() - 1) as usize];
    let witness_index = if top < 8 { top as usize } else { 0 };
    let request = LanguageParserWindow8RawRequest::new(
        action,
        basis.clone(),
        relation.clone(),
        prior.proof().clone(),
        prior.proof().ancestry()[witness_index].clone(),
    )
    .map_err(Window8Refusal::Native)?;
    let context: LanguageParserWindow8RawContext = evaluate(
        request,
        include_str!(concat!(env!("OUT_DIR"), "/window8_move_context.hex")),
    )?;
    let proposal = evaluate_context(context)?;
    Ok(PreparedWindow8Proposal {
        prior: prior.proof().clone(),
        proposal,
    })
}
fn evaluate_context(
    context: LanguageParserWindow8RawContext,
) -> Result<LanguageParserWindow8RawResult, Window8Refusal> {
    let mut checked = context;
    for program in [
        include_str!(concat!(env!("OUT_DIR"), "/window8_move_legal_shift.hex")),
        include_str!(concat!(env!("OUT_DIR"), "/window8_move_legal_reduce.hex")),
        include_str!(concat!(env!("OUT_DIR"), "/window8_move_legal_left.hex")),
        include_str!(concat!(
            env!("OUT_DIR"),
            "/window8_move_legal_right_root.hex"
        )),
        include_str!(concat!(
            env!("OUT_DIR"),
            "/window8_move_legal_right_nonroot.hex"
        )),
    ] {
        checked = evaluate(checked, program)?;
    }
    let proposal: LanguageParserWindow8RawResult = evaluate(
        checked,
        include_str!(concat!(env!("OUT_DIR"), "/window8_move_apply.hex")),
    )?;
    Ok(proposal)
}
/// This receipt retains the exact checked prior, but its output remains raw.
/// Search may rank it; publication still requires next full forest admission.
pub struct PreparedWindow8Proposal {
    prior: LanguageParserWindow8StateProof,
    proposal: LanguageParserWindow8RawResult,
}
impl PreparedWindow8Proposal {
    pub fn prior(&self) -> &LanguageParserWindow8StateProof {
        &self.prior
    }
    pub fn proposal(&self) -> &LanguageParserWindow8RawResult {
        &self.proposal
    }
}
pub fn prepare_window8_step(
    prior: &PreparedWindow8State,
    basis: &LanguageParserBasis,
    action: LanguageParserAction,
    relation: &LanguageParserRelation,
) -> Result<PreparedWindow8Step, Window8Refusal> {
    let prepared = prepare_window8_proposal(prior, basis, action, relation)?;
    let proposal = prepared.proposal;
    let next = if *proposal.accepted() {
        prepare_window8_state(proposal.state())?
    } else {
        if proposal.state() != prior.state() {
            return Err(Window8Refusal::Program);
        }
        prior.clone()
    };
    Ok(PreparedWindow8Step {
        prior: prior.proof().clone(),
        proposal,
        next,
    })
}
pub fn window8_complete(state: &PreparedWindow8State) -> Result<bool, Window8Refusal> {
    let result: LanguageParserWindow8Completion = evaluate(
        state.state().clone(),
        include_str!(concat!(env!("OUT_DIR"), "/window8_complete.hex")),
    )?;
    Ok(*result.complete())
}

/// Decode only the admitted finite version3 model class ABI. This does not
/// admit a transition or establish a linguistic analysis.
pub fn window8_class(
    code: u64,
    default_relation: &LanguageParserRelation,
) -> Result<LanguageParserWindow8RawClass, Window8Refusal> {
    let query = LanguageParserWindow8ClassQuery::new(code, default_relation.clone())
        .map_err(Window8Refusal::Native)?;
    let index: LanguageParserWindow8RawClassIndex = evaluate(
        query,
        include_str!(concat!(env!("OUT_DIR"), "/window8_class_index.hex")),
    )?;
    let relations: LanguageParserWindow8RawClassRelations = evaluate(
        index,
        include_str!(concat!(env!("OUT_DIR"), "/window8_class_relations.hex")),
    )?;
    evaluate(
        relations,
        include_str!(concat!(env!("OUT_DIR"), "/window8_class_relation.hex")),
    )
}

/// Raw search only. Ranking retains complete proposal bodies; selected states
/// still require exact lexical/model custody and full Source forest admission.
pub fn window8_rank(
    mut beam: LanguageParserWindow8RawBeam,
) -> Result<LanguageParserWindow8RawBeam, Window8Refusal> {
    for program in [
        include_str!(concat!(env!("OUT_DIR"), "/window8_rank_0_1.hex")),
        include_str!(concat!(env!("OUT_DIR"), "/window8_rank_2_3.hex")),
        include_str!(concat!(env!("OUT_DIR"), "/window8_rank_0_2.hex")),
        include_str!(concat!(env!("OUT_DIR"), "/window8_rank_1_3.hex")),
        include_str!(concat!(env!("OUT_DIR"), "/window8_rank_1_2.hex")),
    ] {
        beam = evaluate(beam, program)?;
    }
    Ok(beam)
}
pub fn window8_merge(
    beam: LanguageParserWindow8RawBeam,
    proposal: LanguageParserWindow8RawHypothesis,
) -> Result<LanguageParserWindow8RawBeam, Window8Refusal> {
    let input = LanguageParserWindow8RawMerge::new(window8_rank(beam)?, proposal)
        .map_err(Window8Refusal::Native)?;
    window8_rank(evaluate(
        input,
        include_str!(concat!(env!("OUT_DIR"), "/window8_rank_insert.hex")),
    )?)
}

/// Source guards already selected choices and computes the next lexical frontier.
pub fn window8_choice_frontier(
    query: LanguageParserWindow8ChoiceQuery,
) -> Result<LanguageParserWindow8Selected, Window8Refusal> {
    evaluate(
        query,
        include_str!(concat!(env!("OUT_DIR"), "/window8_choice_frontier.hex")),
    )
}
/// Finite raw score accumulation. The outcome must come from the retained
/// Source proposal producer; this function alone grants no graph authority.
pub fn window8_score_advance(
    query: LanguageParserWindow8RawAdvance,
) -> Result<LanguageParserWindow8RawHypothesis, Window8Refusal> {
    evaluate(
        query,
        include_str!(concat!(env!("OUT_DIR"), "/window8_score_advance.hex")),
    )
}

/// Exact immutable Source-produced context, retained with its full prior proof.
/// Its outputs remain raw; every selected forest requires independent admission.
pub struct PreparedWindow8Context {
    prior: PreparedWindow8State,
    seed: LanguageParserWindow8RawContext,
}
pub fn prepare_window8_context(
    prior: &PreparedWindow8State,
    basis: &LanguageParserBasis,
) -> Result<PreparedWindow8Context, Window8Refusal> {
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
    let seed = evaluate(
        request,
        include_str!(concat!(env!("OUT_DIR"), "/window8_move_context.hex")),
    )?;
    Ok(PreparedWindow8Context {
        prior: prior.clone(),
        seed,
    })
}
impl PreparedWindow8Context {
    pub fn prior(&self) -> &PreparedWindow8State {
        &self.prior
    }
    pub fn propose(
        &self,
        class: &LanguageParserWindow8RawClass,
    ) -> Result<PreparedWindow8Proposal, Window8Refusal> {
        let query = LanguageParserWindow8RawClassContext::new(class.clone(), self.seed.clone())
            .map_err(Window8Refusal::Native)?;
        let context = evaluate(
            query,
            include_str!(concat!(env!("OUT_DIR"), "/window8_class_context.hex")),
        )?;
        Ok(PreparedWindow8Proposal {
            prior: self.prior.proof().clone(),
            proposal: evaluate_context(context)?,
        })
    }
}
