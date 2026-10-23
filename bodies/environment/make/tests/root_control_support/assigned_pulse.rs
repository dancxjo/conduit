//! Bounded Root pulse issuance and retained transfer with inert CPU providers.
use super::*;
use conduit_environment_make::PreparedNativeAppliance;
use conduit_environment_runtime::appliance::assigned::{
    ResidentAppliance, ValidatedAssignedAppliance,
};
use conduit_plan_lowering::selected_assigned_operation::{
    AssignedOperationInput as Input, SelectedAssignedOperationPlan as Selected,
};
use conduit_pulse_capture::{
    installation::ReadyPulseBase, owner::PulseReadProvider, PulseReadContract,
};
pub(super) fn check<P: PulseReadProvider>(
    assignment: &ValidatedAssignedAppliance,
    prepared: &PreparedNativeAppliance,
    resident: &ResidentAppliance<'_>,
    required: c::AssignedPlanRequirements<'_>,
    pulse: &ReadyPulseBase<P>,
) {
    assert_eq!(assignment.cadence(), resident.cadence);
    let contract = PulseReadContract::prepare().unwrap();
    let fragment = &prepared.plan().fragments[0];
    let records: Vec<_> = c::AssignedPlanRecords::new(prepared.assignment())
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for input in [
        Input::Contiguous(prepared.assignment()),
        Input::Records {
            header: &prepared.assignment()[..c::ASSIGNED_PLAN_HEADER_BYTES],
            records: &records,
        },
    ] {
        for refuse_transfer in [true, false] {
            let (owner, effects) = super::super::pulse::fresh(pulse.native_identity().clone());
            let mut slot = Some(owner);
            let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
            let unreserved =
                c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
            let selection = |active, node| Selected {
                input,
                maxima: conduit_environment_make::NativeApplianceLimits::resident_local()
                    .assignment,
                requirements: required,
                computations: resident.computations,
                fragment_id: &fragment.fragment_id,
                active,
                node,
            };
            assert!(issuer
                .issue_assigned_pulse(
                    slot.as_ref().unwrap(),
                    assignment,
                    selection(&unreserved, resident.roles.pulse),
                    &resident.cadence,
                    &contract
                )
                .is_err());
            let active = issuer
                .reserve_assigned_play(assignment, &fragment.plan_id, &fragment.fragment_id)
                .unwrap();
            let mut wrong = resident.cadence.clone();
            wrong.count += 1;
            assert!(issuer
                .issue_assigned_pulse(
                    slot.as_ref().unwrap(),
                    assignment,
                    selection(&active, resident.roles.pulse),
                    &wrong,
                    &contract
                )
                .is_err());
            wrong = resident.cadence.clone();
            wrong.period_ms += 1;
            assert!(issuer
                .issue_assigned_pulse(
                    slot.as_ref().unwrap(),
                    assignment,
                    selection(&active, resident.roles.pulse),
                    &wrong,
                    &contract
                )
                .is_err());
            assert!(issuer
                .issue_assigned_pulse(
                    slot.as_ref().unwrap(),
                    assignment,
                    selection(&active, resident.roles.tick),
                    &resident.cadence,
                    &contract
                )
                .is_err());
            let mut corrupt = prepared.assignment().to_vec();
            let last = corrupt.len() - 1;
            corrupt[last] ^= 1;
            assert!(issuer
                .issue_assigned_pulse(
                    slot.as_ref().unwrap(),
                    assignment,
                    Selected {
                        input: Input::Contiguous(&corrupt),
                        ..selection(&active, resident.roles.pulse)
                    },
                    &resident.cadence,
                    &contract
                )
                .is_err());
            assert!(slot.is_some());
            assert_eq!(effects.get(), 0);
            let (table, handle, claim) = issuer
                .issue_assigned_pulse(
                    slot.as_ref().unwrap(),
                    assignment,
                    selection(&active, resident.roles.pulse),
                    &resident.cadence,
                    &contract,
                )
                .unwrap();
            assert_eq!(
                table.inspections().next().unwrap().scope.maximum_operations,
                u32::try_from(resident.cadence.count).unwrap()
            );
            assert!(issuer
                .issue_assigned_pulse(
                    slot.as_ref().unwrap(),
                    assignment,
                    selection(&active, resident.roles.pulse),
                    &resident.cadence,
                    &contract
                )
                .is_err());
            let node = if refuse_transfer {
                resident.roles.tick
            } else {
                resident.roles.pulse
            };
            let bound = ReadyPulseBase::bind_assigned_retained(
                &mut slot,
                table,
                handle,
                claim,
                &contract,
                selection(&active, node),
            );
            assert_eq!(bound.is_ok(), !refuse_transfer);
            assert_eq!(slot.is_some(), refuse_transfer);
            assert_eq!(effects.get(), 0);
            // A refused handoff retains the pin; it does not renew an already
            // issued Root capability. Each branch above has its own fixture issuer.
        }
    }
}
