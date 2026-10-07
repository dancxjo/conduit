//! Private custody of an exact native forest proof. Rust schedules the authored
//! fixed traversal; Source predicates own graph/frontier admission.
use crate::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum Window8Refusal {
    Native(NativeBindingRefusal),
    Program,
}
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
