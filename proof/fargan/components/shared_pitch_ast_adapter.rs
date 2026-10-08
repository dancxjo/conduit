//! Composable root-SDK AST-resource adapter; deliberately separate from the
//! older FARGAN SDK. No Native refinement/custody or evaluator-memory claim.
use conduit_speech::bounded_pitch_programs::{Limits, PROGRAMS, PitchPrograms, Refusal, Storage};
extern crate alloc;
use alloc::{rc::Rc, vec::Vec};

pub struct SharedPitchAst {
    owner: Rc<PitchPrograms>,
    storage: Storage,
}
impl SharedPitchAst {
    /// Checks an already prepared immutable owner without reparsing or copying
    /// programs. Limits cover that owner's AST storage only, not this Rc handle,
    /// query/output frames, Source Native admission or evaluator scratch.
    pub fn from_owner(owner: &Rc<PitchPrograms>, limits: Limits) -> Result<Self, Refusal> {
        if !owner.matches_programs(PROGRAMS.map(|(hex, _)| hex)) {
            return Err(Refusal::Program);
        }
        let storage = owner.storage();
        let required = PitchPrograms::requirements(PROGRAMS)?;
        if storage.retained_bound != required.retained_bound
            || storage.preparation_bound != required.preparation_bound
            || storage.actual_retained > storage.retained_bound
            || storage.retained_bound > limits.retained
            || storage.preparation_bound > limits.preparation
        {
            return Err(Refusal::Capacity);
        }
        Ok(Self {
            owner: Rc::clone(owner),
            storage,
        })
    }
    pub fn storage(&self) -> Storage {
        self.storage
    }
    pub fn shares_owner(&self, owner: &Rc<PitchPrograms>) -> bool {
        Rc::ptr_eq(&self.owner, owner)
    }
    /// Raw execution only. Caller must independently admit original input and
    /// output through their full original Source guards at the composed seam.
    pub fn execute_raw(&self, index: usize, original: &[u8]) -> Result<RawPitchExecution, Refusal> {
        let (program, output) = self.owner.execute(index, original)?;
        Ok(RawPitchExecution {
            owner: Rc::clone(&self.owner),
            index,
            original: original.to_vec(),
            program,
            output,
        })
    }
}
pub struct RawPitchExecution {
    owner: Rc<PitchPrograms>,
    index: usize,
    original: Vec<u8>,
    program: &'static str,
    output: Vec<u8>,
}
impl RawPitchExecution {
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    pub fn program(&self) -> &'static str {
        self.program
    }
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn shares_owner(&self, adapter: &SharedPitchAst) -> bool {
        Rc::ptr_eq(&self.owner, &adapter.owner)
    }
}
