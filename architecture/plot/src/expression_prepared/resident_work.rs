//! The same cold cooperative-work accounting for compiler-owned and resident
//! metadata. These finite counts are not cycles, grants or execution authority.
use super::StructuredTapeInstruction as Operation;

#[derive(Clone, Copy)]
pub(crate) struct PathWork {
    pub steps: usize,
    pub each: bool,
}
impl PathWork {
    fn visits(self, nodes: u32) -> u32 {
        let depth = (self.steps + 1) as u32;
        if self.each {
            depth.saturating_mul(nodes).saturating_mul(2)
        } else {
            depth
        }
    }
}
pub(crate) fn native_validation_work(
    maximum_bytes: usize,
    schema_maximum_bytes: usize,
    contracts: impl IntoIterator<Item = (PathWork, u16)>,
    scopes: impl IntoIterator<Item = PathWork>,
) -> u32 {
    let nodes = (maximum_bytes.min(schema_maximum_bytes) / 5 + 1) as u32;
    let mut work = 1u32;
    for (path, checks) in contracts {
        let leaves = if path.each { nodes } else { 1 };
        work = work
            .saturating_add(1)
            .saturating_add(path.visits(nodes))
            .saturating_add(leaves.saturating_mul(u32::from(checks)));
    }
    for path in scopes {
        work = work.saturating_add(1).saturating_add(path.visits(nodes));
    }
    work
}
#[derive(Clone, Copy)]
pub(crate) enum ProjectedInputWork {
    Fixed,
    Path(usize),
    VariantIs(usize),
    Length(usize),
}
pub(crate) fn projected_work(
    instructions: usize,
    inputs: impl IntoIterator<Item = ProjectedInputWork>,
    output_contract: u16,
) -> u32 {
    let mut work = (instructions as u32).saturating_add(4);
    for source in inputs {
        let units = match source {
            ProjectedInputWork::Fixed => 2,
            ProjectedInputWork::Path(depth) | ProjectedInputWork::Length(depth) => depth + 2,
            ProjectedInputWork::VariantIs(depth) => depth + 3,
        };
        work = work.saturating_add(units as u32);
    }
    work.saturating_add(u32::from(output_contract))
}
pub(crate) fn direct_path_work(depth: usize) -> u32 {
    (depth + 1) as u32
}
pub(crate) fn structured_work(
    input_validation: u32,
    output_validation: u32,
    instructions: &[Operation],
    child_work: impl Fn(u16) -> u32,
) -> u32 {
    let mut work = input_validation
        .saturating_add(output_validation)
        .saturating_add(2);
    for op in instructions {
        let units = match *op {
            Operation::Value(i) => child_work(i).saturating_add(4),
            Operation::BranchFalse { value, .. } => child_work(value).saturating_add(2),
            Operation::Bytes { .. } | Operation::InputNode { .. } => 2,
            Operation::Jump(_) => 1,
        };
        work = work.saturating_add(units);
    }
    work
}
