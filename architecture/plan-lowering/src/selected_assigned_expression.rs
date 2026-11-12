//! Exact resident computation selection, without issuing authority or storage.
use crate::selected_assigned_operation::{AssignedForeProfile, AssignedOperationInput};
use conduit_core as c;
use conduit_kernel::NodeId;

/// Immutable implementation facts emitted by Make, never inferred from a Plan.
#[derive(Clone, Copy)]
pub struct ResidentExpressionIdentity {
    pub kind: c::AssignedIdentity,
    pub implementation: c::AssignedIdentity,
    pub artifact: c::AssignedIdentity,
    pub input_port: c::AssignedIdentity,
    pub input_kind: c::AssignedIdentity,
    pub output_port: c::AssignedIdentity,
    pub output_kind: c::AssignedIdentity,
    pub computation: c::AssignedComputationRequirement,
    pub private_work_units: u16,
}
/// Root supplies its actual current scope and independently reserved work index.
/// This value, its records and successful checking do not grant Play authority.
pub struct SelectedAssignedExpressionPlan<'a> {
    pub input: AssignedOperationInput<'a>,
    pub maxima: c::AssignedPlanMaxima,
    pub requirements: c::AssignedPlanRequirements<'a>,
    pub computations: c::AssignedComputationProfile<'a>,
    pub fore: Option<&'a [c::AssignedForeRequirement]>,
    /// Borrowed fragment identity text, checked against the exact fragment field.
    /// Its spelling is not authority; the opaque Body scope must also match.
    pub fragment_id: &'a str,
    pub scope: crate::assigned_play_scope::AssignedPlayScope<'a>,
    pub node: NodeId,
    pub work_resource: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidentExpressionBindingRefusal {
    MissingOwner,
    Assignment(c::AssignedPlanRefusal),
    Scope,
    Selection,
}

/// Validate before transferring the initialized owner to an admitted execution.
/// Root still owns signature/Body policy, finite workspace, queues, actual pool
/// reservation and retirement. No input here is an opaque resource capability.
pub fn check_selected_resident_expression(
    identity: ResidentExpressionIdentity,
    selected: SelectedAssignedExpressionPlan<'_>,
) -> Result<NodeId, ResidentExpressionBindingRefusal> {
    let fore = selected.fore.map(AssignedForeProfile::Local);
    check_expression_profile(identity, selected, fore)
}
/// Explicit schema-8 resident descriptor. The legacy inner Fore must be None;
/// the independent Line profile never grants a carrier, resource or native IO.
pub struct SelectedAssignedLineForeExpressionPlan<'a> {
    pub expression: SelectedAssignedExpressionPlan<'a>,
    pub fore: c::AssignedLineForeProfile<'a>,
}
pub fn check_selected_resident_expression_with_line_fore(
    identity: ResidentExpressionIdentity,
    selected: SelectedAssignedLineForeExpressionPlan<'_>,
) -> Result<NodeId, ResidentExpressionBindingRefusal> {
    if selected.expression.fore.is_some() {
        return Err(ResidentExpressionBindingRefusal::Selection);
    }
    check_expression_profile(
        identity,
        selected.expression,
        Some(AssignedForeProfile::Line(selected.fore)),
    )
}
fn check_expression_profile(
    identity: ResidentExpressionIdentity,
    selected: SelectedAssignedExpressionPlan<'_>,
    fore: Option<AssignedForeProfile<'_>>,
) -> Result<NodeId, ResidentExpressionBindingRefusal> {
    use ResidentExpressionBindingRefusal as E;
    let schema = u16_at(selected.input.header(), 8)?;
    if (fore.is_some_and(|profile| schema != profile.schema()))
        || (fore.is_none()
            && !matches!(
                schema,
                c::ASSIGNED_KERNEL_PLAN_SCHEMA | c::ASSIGNED_NATIVE_PLAN_SCHEMA
            ))
    {
        return Err(E::Selection);
    }
    let view = selected
        .input
        .decode_profile(
            selected.maxima,
            selected.requirements,
            selected.computations,
            fore,
        )
        .map_err(E::Assignment)?;
    if view.fragment != c::AssignedIdentity::from_text(selected.fragment_id)
        || !selected.scope.matches(&view, selected.input.header())
    {
        return Err(E::Scope);
    }
    let fuel = identity
        .private_work_units
        .checked_add(3)
        .ok_or(E::Selection)?;
    if identity.private_work_units == 0
        || !selected
            .requirements
            .resources
            .contains(&selected.work_resource)
    {
        return Err(E::Selection);
    }
    let mut seen = [0u16; 5]; // node, input, output, computation, work reservation
    for record in selected.input.records().map_err(E::Assignment)? {
        let record = record.map_err(E::Assignment)?;
        if !matches!(
            record.tag,
            c::ASSIGNED_NODE
                | c::ASSIGNED_PORT
                | c::ASSIGNED_CONFIGURATION
                | c::ASSIGNED_RESOURCE
                | c::ASSIGNED_HOST_CALL
        ) {
            continue;
        }
        let d = record.payload;
        if u16_at(d, 0)? != selected.node.0 {
            continue;
        }
        match record.tag {
            c::ASSIGNED_NODE => {
                seen[0] += 1;
                if u16_at(d, 2)? != fuel
                    || identity_at(d, 4)? != identity.kind
                    || identity_at(d, 20)? != identity.implementation
                    || identity_at(d, 36)? != identity.artifact
                {
                    return Err(E::Selection);
                }
            }
            c::ASSIGNED_PORT => {
                let (index, port, kind) = match d.get(4) {
                    Some(0) => (1, identity.input_port, identity.input_kind),
                    Some(1) => (2, identity.output_port, identity.output_kind),
                    _ => return Err(E::Selection),
                };
                seen[index] += 1;
                if u16_at(d, 2)? != 0 || identity_at(d, 5)? != port || identity_at(d, 21)? != kind {
                    return Err(E::Selection);
                }
            }
            c::ASSIGNED_CONFIGURATION => {
                seen[3] += 1;
                let pin = identity.computation;
                if d.len() != 55
                    || d[18] != 4
                    || identity_at(d, 2)? != pin.key
                    || d.get(19..23) != Some(pin.text_bytes.to_le_bytes().as_slice())
                    || d.get(23..55) != Some(pin.text_digest.as_slice())
                {
                    return Err(E::Selection);
                }
            }
            c::ASSIGNED_RESOURCE => {
                seen[4] += 1;
                if u16_at(d, 2)? != selected.work_resource
                    || d.get(4..8)
                        != Some(
                            u32::from(identity.private_work_units)
                                .to_le_bytes()
                                .as_slice(),
                        )
                {
                    return Err(E::Selection);
                }
            }
            _ => return Err(E::Selection), // A pure expression owns no Host Call.
        }
    }
    if seen != [1; 5] {
        return Err(E::Selection);
    }
    Ok(selected.node)
}
fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, ResidentExpressionBindingRefusal> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(ResidentExpressionBindingRefusal::Selection)?
            .try_into()
            .map_err(|_| ResidentExpressionBindingRefusal::Selection)?,
    ))
}
fn identity_at(
    bytes: &[u8],
    offset: usize,
) -> Result<c::AssignedIdentity, ResidentExpressionBindingRefusal> {
    Ok(c::AssignedIdentity(
        bytes
            .get(offset..offset + 16)
            .ok_or(ResidentExpressionBindingRefusal::Selection)?
            .try_into()
            .map_err(|_| ResidentExpressionBindingRefusal::Selection)?,
    ))
}
