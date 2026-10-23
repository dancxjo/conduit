//! Production Root issuance with inert retained timer and admitted CPU tables.
use super::*;
use conduit_environment_make::PreparedNativeAppliance;
use conduit_environment_runtime::appliance::assigned::{
    ResidentAppliance, ValidatedAssignedAppliance,
};
use conduit_monotonic_wait::{MonotonicWaitProvider, ReadyMonotonicWaitBase};
use conduit_plan_lowering::selected_assigned_operation::{
    AssignedOperationInput as Input, SelectedAssignedOperationPlan as Selected,
};
pub(super) fn check<P: MonotonicWaitProvider>(
    assignment: &ValidatedAssignedAppliance,
    prepared: &PreparedNativeAppliance,
    resident: &ResidentAppliance<'_>,
    required: c::AssignedPlanRequirements<'_>,
    timer: &ReadyMonotonicWaitBase<P>,
) {
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
        let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
        let unreserved =
            c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
        let selection = |active, node| Selected {
            input,
            maxima: conduit_environment_make::NativeApplianceLimits::resident_local().assignment,
            requirements: required,
            computations: resident.computations,
            fragment_id: &fragment.fragment_id,
            active,
            node,
        };
        assert!(issuer
            .issue_assigned_wait(
                timer,
                assignment,
                selection(&unreserved, resident.roles.tick),
                &resident.cadence
            )
            .is_err());
        let active = issuer
            .reserve_assigned_play(assignment, &fragment.plan_id, &fragment.fragment_id)
            .unwrap();
        let mut wrong = resident.cadence.clone();
        wrong.count += 1;
        assert!(issuer
            .issue_assigned_wait(
                timer,
                assignment,
                selection(&active, resident.roles.tick),
                &wrong
            )
            .is_err());
        assert!(issuer
            .issue_assigned_wait(
                timer,
                assignment,
                selection(&active, resident.roles.pulse),
                &resident.cadence
            )
            .is_err());
        let mut corrupted = prepared.assignment().to_vec();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        let invalid = Selected {
            input: Input::Contiguous(&corrupted),
            ..selection(&active, resident.roles.tick)
        };
        assert!(issuer
            .issue_assigned_wait(timer, assignment, invalid, &resident.cadence)
            .is_err());
        let (mut table, handle, claim) = issuer
            .issue_assigned_wait(
                timer,
                assignment,
                selection(&active, resident.roles.tick),
                &resident.cadence,
            )
            .unwrap();
        assert_eq!(
            table.inspections().next().unwrap().scope.maximum_operations,
            u32::try_from(resident.cadence.count).unwrap()
        );
        assert_eq!(claim.plan_id, fragment.plan_id);
        assert_eq!(claim.active_play_id, active.active_play_id);
        table.authorize(&handle, &claim).unwrap();
        assert!(issuer
            .issue_assigned_wait(
                timer,
                assignment,
                selection(&active, resident.roles.tick),
                &resident.cadence
            )
            .is_err());
    }
}
