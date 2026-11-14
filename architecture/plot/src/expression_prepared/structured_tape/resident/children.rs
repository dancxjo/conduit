//! Constructor-checked child storage; runtime lookup never validates definitions.
use super::*;
use crate::expression_prepared::scalar_tape::projected::ParentCheckedProjectedExpression;
use conduit_core::BorrowedFixedIntegerContract;

/// Immutable Make descriptions and constant-time output-guard slot selection.
/// With no guards the slot slice is empty. Otherwise it has one entry per
/// child; u16::MAX denotes a child without an output guard.
pub struct StaticStructuredChildTable {
    pub children: &'static [StaticStructuredChildProgram],
    pub output_guard_slots: &'static [u16],
}
#[derive(Clone, Copy)]
pub(super) enum ParentCheckedStructuredValue {
    Scalar(ParentCheckedProjectedExpression),
    InputPath(&'static [crate::StaticExpressionPathStep]),
}
// Private implementations close this interface over checked storage only.
pub(super) trait Children {
    fn get(&self, index: usize) -> Option<ParentCheckedStructuredValue>;
}
impl<const C: usize> Children for [ParentCheckedStructuredValue; C] {
    fn get(&self, index: usize) -> Option<ParentCheckedStructuredValue> {
        self.as_slice().get(index).copied()
    }
}
pub(super) struct BorrowedChildren<const G: usize> {
    table: &'static StaticStructuredChildTable,
    guards: [Option<BorrowedFixedIntegerContract<'static>>; G],
}
impl<const G: usize> BorrowedChildren<G> {
    pub(super) fn prepare<const I: usize>(
        input: &ResidentNativeValidator<I>,
        table: &'static StaticStructuredChildTable,
    ) -> Result<Self, StaticStructuredRefusal> {
        let count = table.children.len();
        if count > MAX_VALUES
            || G > count
            || (G == 0 && !table.output_guard_slots.is_empty())
            || (G != 0 && table.output_guard_slots.len() != count)
        {
            return Err(Refusal::InvalidProgram.into());
        }
        let mut guards = [None; G];
        for (index, descriptor) in table.children.iter().enumerate() {
            let slot = table
                .output_guard_slots
                .get(index)
                .copied()
                .unwrap_or(u16::MAX);
            match descriptor {
                StaticStructuredChildProgram::Scalar(program) => {
                    let checked = StaticProjectedExpression::new(input.clone(), program)
                        .map_err(|error| StaticStructuredRefusal::Child {
                            index: index as u16,
                            refusal: StaticProjectedRefusal::Expression(error),
                        })?
                        .into_parent_checked();
                    match checked.output_contract() {
                        Some(guard) => {
                            let target = guards
                                .get_mut(usize::from(slot))
                                .ok_or(Refusal::InvalidProgram)?;
                            if target.replace(guard).is_some() {
                                return Err(Refusal::InvalidProgram.into());
                            }
                        }
                        None if slot != u16::MAX => return Err(Refusal::InvalidProgram.into()),
                        None => {}
                    }
                }
                StaticStructuredChildProgram::InputPath(path) => {
                    if slot != u16::MAX
                        || !crate::expression_prepared::resident_projection::valid(path)
                    {
                        return Err(Refusal::InvalidProgram.into());
                    }
                }
            }
        }
        if guards.iter().any(Option::is_none) {
            return Err(Refusal::InvalidProgram.into());
        }
        Ok(Self { table, guards })
    }
}
impl<const G: usize> Children for BorrowedChildren<G> {
    fn get(&self, index: usize) -> Option<ParentCheckedStructuredValue> {
        match self.table.children.get(index)? {
            StaticStructuredChildProgram::InputPath(path) => {
                Some(ParentCheckedStructuredValue::InputPath(path))
            }
            StaticStructuredChildProgram::Scalar(program) => {
                let guard = if program.output_contract.is_some() {
                    let slot = *self.table.output_guard_slots.get(index)?;
                    Some((*self.guards.get(usize::from(slot))?)?)
                } else {
                    None
                };
                // prepare validated this exact immutable program and slot binding.
                Some(ParentCheckedStructuredValue::Scalar(
                    ParentCheckedProjectedExpression::from_parent_checked_program(program, guard),
                ))
            }
        }
    }
}
