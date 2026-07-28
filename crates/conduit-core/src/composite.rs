//! Host-independent composite definitions and transparent exports.

use core::fmt;

use crate::{
    CompatibilityOutcome, ConfigFieldContract, Direction, Id, LossAcceptance, NodeContract,
    PlanCord, PortContract, assess_port_connection, assess_type_contract_exact,
};

/// Stable slash-separated logical path, distinct from a local [`Id`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstancePath<'a>(&'a str);

impl<'a> InstancePath<'a> {
    pub fn new(value: &'a str) -> Result<Self, CompositeError> {
        if value.is_empty() || value.split('/').any(|segment| Id::new(segment).is_err()) {
            return Err(CompositeError::InvalidInstancePath);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

/// One child instance inside a composite definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeChild<'a> {
    pub id: Id<'a>,
    pub definition: Id<'a>,
    pub contract: &'a NodeContract<'a>,
}

/// Direction-preserving boundary mapping to one immediate child port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeExport {
    pub boundary_port: u16,
    pub child: u16,
    pub child_port: u16,
    pub direction: Direction,
}

/// Boundary configuration parameter mapped to one child field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeConfigBinding {
    pub parameter: u16,
    pub child: u16,
    pub child_field: u16,
}

/// One semantic composite node definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeDefinition<'a> {
    pub id: Id<'a>,
    pub contract: &'a NodeContract<'a>,
    pub children: &'a [CompositeChild<'a>],
    pub cords: &'a [PlanCord<'a>],
    pub exports: &'a [CompositeExport],
    pub bindings: &'a [CompositeConfigBinding],
}

/// One definition-to-definition edge used for recursion validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionDependencies<'a> {
    pub definition: Id<'a>,
    pub composite_children: &'a [Id<'a>],
}

/// Validate complete transparent boundary mappings.
pub fn validate_composite(definition: &CompositeDefinition<'_>) -> Result<(), CompositeError> {
    if definition.id != definition.contract.id {
        return Err(CompositeError::IdentityMismatch);
    }
    for (index, child) in definition.children.iter().enumerate() {
        if child.definition != child.contract.id {
            return Err(CompositeError::IdentityMismatch);
        }
        if definition.children[..index]
            .iter()
            .any(|prior| prior.id == child.id)
        {
            return Err(CompositeError::DuplicateChild);
        }
    }
    for cord in definition.cords {
        let from = definition
            .children
            .get(usize::from(cord.from.node))
            .and_then(|child| child.contract.outputs.get(usize::from(cord.from.port)));
        let to = definition
            .children
            .get(usize::from(cord.to.node))
            .and_then(|child| child.contract.inputs.get(usize::from(cord.to.port)));
        let from = from.ok_or(CompositeError::DanglingCord)?;
        let to = to.ok_or(CompositeError::DanglingCord)?;
        let type_decision = assess_type_contract_exact(to.value_type, from.value_type);
        if assess_port_connection(*to, *from, type_decision).outcome
            != CompatibilityOutcome::Compatible
            || (cord.flow.pressure.permits_loss()
                && (from.flow.loss == LossAcceptance::LosslessOnly
                    || to.flow.loss == LossAcceptance::LosslessOnly))
        {
            return Err(CompositeError::IncompatibleCord);
        }
    }

    let boundary_count = definition.contract.inputs.len() + definition.contract.outputs.len();
    if definition.exports.len() != boundary_count {
        return Err(CompositeError::MissingExport);
    }
    for (index, export) in definition.exports.iter().enumerate() {
        if definition.exports[..index].iter().any(|prior| {
            prior.direction == export.direction
                && (prior.boundary_port == export.boundary_port
                    || (prior.child == export.child && prior.child_port == export.child_port))
        }) {
            return Err(CompositeError::DuplicateExport);
        }
        let boundary = boundary_port(definition.contract, *export)?;
        let child = definition
            .children
            .get(usize::from(export.child))
            .ok_or(CompositeError::DanglingExport)?;
        let mapped = match export.direction {
            Direction::Input => child.contract.inputs.get(usize::from(export.child_port)),
            Direction::Output => child.contract.outputs.get(usize::from(export.child_port)),
        }
        .ok_or(CompositeError::DanglingExport)?;
        if !same_port_semantics(boundary, mapped) {
            return Err(CompositeError::IncompatibleExport);
        }
    }

    for (index, binding) in definition.bindings.iter().enumerate() {
        if definition.bindings[..index].iter().any(|prior| {
            prior.parameter == binding.parameter
                && prior.child == binding.child
                && prior.child_field == binding.child_field
        }) {
            return Err(CompositeError::DuplicateBinding);
        }
        let parameter = definition
            .contract
            .config
            .fields
            .get(usize::from(binding.parameter))
            .ok_or(CompositeError::DanglingBinding)?;
        let child_field = definition
            .children
            .get(usize::from(binding.child))
            .and_then(|child| {
                child
                    .contract
                    .config
                    .fields
                    .get(usize::from(binding.child_field))
            })
            .ok_or(CompositeError::DanglingBinding)?;
        if !same_config_semantics(parameter, child_field) {
            return Err(CompositeError::IncompatibleBinding);
        }
    }
    for parameter in 0..definition.contract.config.fields.len() {
        if !definition
            .bindings
            .iter()
            .any(|binding| usize::from(binding.parameter) == parameter)
        {
            return Err(CompositeError::MissingBinding);
        }
    }
    Ok(())
}

fn boundary_port<'a>(
    contract: &'a NodeContract<'a>,
    export: CompositeExport,
) -> Result<&'a PortContract<'a>, CompositeError> {
    match export.direction {
        Direction::Input => contract.inputs.get(usize::from(export.boundary_port)),
        Direction::Output => contract.outputs.get(usize::from(export.boundary_port)),
    }
    .ok_or(CompositeError::DanglingExport)
}

fn same_port_semantics(left: &PortContract<'_>, right: &PortContract<'_>) -> bool {
    left.direction == right.direction
        && left.value_type == right.value_type
        && left.presence == right.presence
        && left.connections == right.connections
        && left.values == right.values
        && left.delivery == right.delivery
        && left.temporal == right.temporal
        && left.terminal == right.terminal
        && left.sensitivity == right.sensitivity
        && left.flow == right.flow
}

fn same_config_semantics(left: &ConfigFieldContract<'_>, right: &ConfigFieldContract<'_>) -> bool {
    left.value_type == right.value_type
        && left.requirement == right.requirement
        && left.sensitivity == right.sensitivity
        && left.mutability == right.mutability
        && left.identity == right.identity
}

/// Reject recursive composite-definition graphs using caller-provided marks.
pub fn validate_definition_dependencies(
    definitions: &[DefinitionDependencies<'_>],
    marks: &mut [u8],
) -> Result<(), CompositeError> {
    if marks.len() < definitions.len() {
        return Err(CompositeError::ScratchTooSmall);
    }
    marks[..definitions.len()].fill(0);
    for index in 0..definitions.len() {
        if definitions[..index]
            .iter()
            .any(|prior| prior.definition == definitions[index].definition)
        {
            return Err(CompositeError::IdentityMismatch);
        }
        visit_definition(index, definitions, marks)?;
    }
    Ok(())
}

fn visit_definition(
    index: usize,
    definitions: &[DefinitionDependencies<'_>],
    marks: &mut [u8],
) -> Result<(), CompositeError> {
    match marks[index] {
        1 => return Err(CompositeError::RecursiveDefinition),
        2 => return Ok(()),
        _ => {}
    }
    marks[index] = 1;
    for child in definitions[index].composite_children {
        let child_index = definitions
            .iter()
            .position(|candidate| candidate.definition == *child)
            .ok_or(CompositeError::UnknownDefinition)?;
        visit_definition(child_index, definitions, marks)?;
    }
    marks[index] = 2;
    Ok(())
}

/// Stable composite validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeError {
    InvalidInstancePath,
    IdentityMismatch,
    DuplicateChild,
    DanglingCord,
    MissingExport,
    DuplicateExport,
    DanglingExport,
    IncompatibleExport,
    IncompatibleCord,
    MissingBinding,
    DuplicateBinding,
    DanglingBinding,
    IncompatibleBinding,
    RecursiveDefinition,
    UnknownDefinition,
    ScratchTooSmall,
}

impl CompositeError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInstancePath | Self::IdentityMismatch => "CND-CMP-001",
            Self::DuplicateChild => "CND-CMP-002",
            Self::DanglingCord | Self::DanglingExport | Self::DanglingBinding => "CND-CMP-003",
            Self::IncompatibleExport | Self::IncompatibleCord | Self::IncompatibleBinding => {
                "CND-CMP-004"
            }
            Self::RecursiveDefinition | Self::UnknownDefinition => "CND-CMP-005",
            Self::MissingExport
            | Self::DuplicateExport
            | Self::MissingBinding
            | Self::DuplicateBinding => "CND-CMP-002",
            Self::ScratchTooSmall => "CND-CMP-008",
        }
    }
}

impl fmt::Display for CompositeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInstancePath => "invalid composite instance path",
            Self::IdentityMismatch => "composite definition identity is inconsistent",
            Self::DuplicateChild => "duplicate composite child",
            Self::DanglingCord => "composite cord endpoint is dangling",
            Self::MissingExport => "not every boundary port has an export",
            Self::DuplicateExport => "duplicate composite export",
            Self::DanglingExport => "composite export target is dangling",
            Self::IncompatibleExport => "export does not retain complete port semantics",
            Self::IncompatibleCord => "internal cord contracts are incompatible",
            Self::MissingBinding => "not every boundary parameter has a binding",
            Self::DuplicateBinding => "duplicate composite parameter binding",
            Self::DanglingBinding => "composite parameter binding is dangling",
            Self::IncompatibleBinding => "binding does not retain complete config semantics",
            Self::RecursiveDefinition => "composite definition graph is recursive",
            Self::UnknownDefinition => "composite dependency is unknown",
            Self::ScratchTooSmall => "composite validation scratch is too small",
        })
    }
}
