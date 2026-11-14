//! Retained abnormal-finalization bytes, distinct from public abnormal values.
use super::{
    AssignedAbnormalTransduction, AssignedTerminalTransduction, CanonicalValue, SchedulerError,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PhaseTerminal<const BYTES: usize> {
    len: u8,
    bytes: [u8; BYTES],
}
impl<const BYTES: usize> PhaseTerminal<BYTES> {
    pub(super) fn new(value: CanonicalValue) -> Result<Self, SchedulerError> {
        let source = value.as_slice();
        if source.len() > BYTES {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut terminal = Self {
            len: source.len() as u8,
            bytes: [0; BYTES],
        };
        terminal.bytes[..source.len()].copy_from_slice(source);
        Ok(terminal)
    }
    pub(super) fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}
/// Current contracts carry no smaller abnormal envelope. Any selected finite
/// abnormal finalizer therefore requires the complete public100-byte profile.
/// Other abnormal outcomes keep their original public value and Cord storage.
pub(super) fn validate_capacity<const B: usize, const N: usize, const P: usize>(
    contracts: &[[Option<AssignedTerminalTransduction>; P]; N],
) -> Result<(), SchedulerError> {
    if B < CanonicalValue::MAXIMUM_BYTES
        && contracts.iter().flatten().flatten().any(|c| {
            matches!(
                c.abnormal,
                AssignedAbnormalTransduction::FinalizeThenPropagate(_)
            )
        })
    {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
}
