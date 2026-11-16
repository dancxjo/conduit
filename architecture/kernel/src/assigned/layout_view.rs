//! Borrowed trusted inventories; no ownership, allocation or authority.
use super::*;
/// Repeatable expected rows retain exact multiplicity under the same validator.
pub trait ResidentLayoutSource: Copy {
    fn counts(self) -> [u8; c::ASSIGNED_PLAN_COUNT_KINDS];
    fn records(self) -> impl Iterator<Item = c::AssignedRecordRequirement> + Clone;
}
impl ResidentLayoutSource for &ResidentLayout<'_> {
    fn counts(self) -> [u8; c::ASSIGNED_PLAN_COUNT_KINDS] {
        self.counts
    }
    fn records(self) -> impl Iterator<Item = c::AssignedRecordRequirement> + Clone {
        self.records.iter().copied()
    }
}
#[derive(Clone, Copy)]
pub struct ResidentLayoutView<'a> {
    installed: &'a ResidentLayout<'a>,
    overrides: &'a [c::AssignedLayoutOverride],
}
impl<'a> ResidentLayoutView<'a> {
    /// Overrides must come from independently checked construction, never from
    /// incoming assignment rows. Validation here checks only representation.
    pub fn checked(
        installed: &'a ResidentLayout<'a>,
        overrides: &'a [c::AssignedLayoutOverride],
    ) -> Result<Self, AssignedKernelRefusal> {
        if overrides.len() > 32
            || overrides.iter().enumerate().any(|(i, o)| {
                usize::from(o.ordinal) >= installed.records.len()
                    || installed.records[usize::from(o.ordinal)].tag != o.requirement.tag
                    || overrides[..i].iter().any(|p| p.ordinal >= o.ordinal)
            })
        {
            return Err(AssignedKernelRefusal::Layout);
        }
        Ok(Self {
            installed,
            overrides,
        })
    }
}
impl ResidentLayoutSource for ResidentLayoutView<'_> {
    fn counts(self) -> [u8; c::ASSIGNED_PLAN_COUNT_KINDS] {
        self.installed.counts
    }
    fn records(self) -> impl Iterator<Item = c::AssignedRecordRequirement> + Clone {
        self.installed
            .records
            .iter()
            .copied()
            .enumerate()
            .map(move |(i, row)| {
                self.overrides
                    .iter()
                    .find(|o| usize::from(o.ordinal) == i)
                    .map_or(row, |o| o.requirement)
            })
    }
}
impl<'a> From<&'a ResidentLayout<'a>> for ResidentLayoutView<'a> {
    fn from(installed: &'a ResidentLayout<'a>) -> Self {
        Self {
            installed,
            overrides: &[],
        }
    }
}
impl ResidentLayoutView<'_> {
    pub fn installed(&self) -> &ResidentLayout<'_> {
        self.installed
    }
    pub fn has_overrides(&self) -> bool {
        !self.overrides.is_empty()
    }
}
