//! Shared bounded constructor execution, independent of instruction ownership.
use super::*;
use crate::expression_prepared::canonical_input::CanonicalInput;
#[derive(Clone, Copy, Debug)]
pub enum StructuredTapeInstruction {
    Bytes { start: u32, length: u32 },
    InputNode { start: u32, length: u32 },
    Value(u16),
    BranchFalse { value: u16, target: u16 },
    Jump(u16),
}
pub(in crate::expression_prepared) trait Output {
    fn write(&mut self, bytes: &[u8]) -> Result<(), Refusal>;
}
impl Output for Vec<u8> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        crate::expression_prepared::structured::append(self, bytes)
    }
}
pub(super) trait Values {
    type Error: From<Refusal>;
    fn append(
        &mut self,
        index: u16,
        input: CanonicalInput<'_>,
        output: &mut impl Output,
    ) -> Result<(), Self::Error>;
    fn condition(&mut self, index: u16, input: CanonicalInput<'_>) -> Result<bool, Self::Error>;
}
pub(super) fn execute<V: Values>(
    code: &[Operation],
    constants: &[u8],
    input: CanonicalInput<'_>,
    output: &mut impl Output,
    values: &mut V,
) -> Result<(), V::Error> {
    let mut pc = 0;
    while pc < code.len() {
        match code[pc] {
            Operation::Bytes { start, length } => {
                output.write(slice(constants, start, length, Refusal::InvalidProgram)?)?
            }
            Operation::InputNode { start, length } => {
                let start = usize::try_from(start).map_err(|_| Refusal::InvalidInput)?;
                let length = usize::try_from(length).map_err(|_| Refusal::InvalidInput)?;
                output.write(
                    input
                        .get(start..start.checked_add(length).ok_or(Refusal::InvalidInput)?)
                        .ok_or(Refusal::InvalidInput)?,
                )?
            }
            Operation::Value(value) => values.append(value, input, output)?,
            Operation::BranchFalse { value, target } => {
                if !values.condition(value, input)? {
                    pc = usize::from(target);
                    continue;
                }
            }
            Operation::Jump(target) => {
                pc = usize::from(target);
                continue;
            }
        }
        pc += 1;
    }
    Ok(())
}
fn slice(bytes: &[u8], start: u32, length: u32, refusal: Refusal) -> Result<&[u8], Refusal> {
    let start = usize::try_from(start).map_err(|_| refusal.clone())?;
    let length = usize::try_from(length).map_err(|_| refusal.clone())?;
    let end = start.checked_add(length).ok_or_else(|| refusal.clone())?;
    bytes.get(start..end).ok_or(refusal)
}
