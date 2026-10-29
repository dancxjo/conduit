//! Dormant native output binding after authenticated reservation and kernel prep.
//! The actual retained owner is transferred once; no electrical call runs here.
use alloc::{boxed::Box, vec::Vec};
use conduit_core as c;
use conduit_environment_runtime::{
    appliance::{assigned::ResidentSourceIdentity, assignment_transfer as t},
    display_output::*,
};
use conduit_plan_lowering::selected_assigned_operation::{
    AssignedOperationInput, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
};
use conduit_tm1637_wire::{
    installation::ReadyWriteBase, owner::WriteHostCall, OpenDrain, WriteContract,
};
pub struct BindingInput<'a> {
    pub assignment: &'a ValidatedAssignedDisplay,
    pub wire: &'a [u8],
    pub profile: &'a AssignedDisplayProfile<'a>,
    pub resident: &'a [c::AssignedRecord<'a>],
    pub fragment: &'a c::FragmentId,
    pub active: &'a c::ActivePlayIdentity,
    pub maximum_frames: u32,
    pub available_bytes: usize,
}
pub fn bind<P: OpenDrain>(
    issuer: &mut crate::native_authority::RootIssuer,
    owner: &mut Option<ReadyWriteBase<P>>,
    input: BindingInput<'_>,
) -> Result<Box<WriteHostCall<P>>, &'static str> {
    if owner.is_none() {
        return Err("display-owner-unavailable");
    }
    let needed = t::MAXIMUM_RECORDS * core::mem::size_of::<c::AssignedRecord<'_>>()
        + core::mem::size_of::<WriteHostCall<P>>()
        + 32768;
    if input.available_bytes < needed {
        return Err("display-binding-capacity");
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(t::MAXIMUM_RECORDS)
        .map_err(|_| "display-records")?;
    records.resize(
        t::MAXIMUM_RECORDS,
        c::AssignedRecord {
            tag: 0,
            payload: &[],
        },
    );
    let decoded = t::decode(
        input.wire,
        ResidentSourceIdentity::from_expanded(&input.profile.source.expanded_plot_id),
        input.resident,
        &mut records,
    )
    .map_err(|_| "display-transfer")?;
    let selected = || SelectedAssignedForeOperationPlan {
        operation: SelectedAssignedOperationPlan {
            input: AssignedOperationInput::Records {
                header: decoded.header,
                records: decoded.records,
            },
            maxima: c::AssignedPlanMaxima {
                encoded_bytes: 16384,
                runtime_state_bytes: 32768,
                counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
            },
            requirements: c::AssignedPlanRequirements {
                host: input.assignment.identity().host,
                boot: input.assignment.identity().boot,
                host_calls: input.profile.host_calls,
                resources: input.profile.resources,
                remote_bindings: &[],
            },
            computations: input.profile.computations,
            fragment_id: input.fragment,
            active: input.active,
            node: input.profile.write,
        },
        fore: input.profile.fore,
    };
    let contract = WriteContract::prepare().map_err(|_| "display-contract")?;
    let (table, handle, claim) = issuer
        .issue_assigned_display(
            owner.as_ref().ok_or("display-owner-unavailable")?,
            input.assignment,
            selected(),
            input.maximum_frames,
            &contract,
        )
        .map_err(|_| "display-possession")?;
    let bound = ReadyWriteBase::bind_assigned_fore_retained(
        owner,
        table,
        handle,
        claim,
        &contract,
        selected(),
    )
    .map_err(|_| "display-binding")?;
    Ok(Box::new(bound))
}
