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
pub fn prepare_window8_step(
    prior: &PreparedWindow8State,
    basis: &LanguageParserBasis,
    action: LanguageParserAction,
    relation: &LanguageParserRelation,
) -> Result<PreparedWindow8Step, Window8Refusal> {
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
