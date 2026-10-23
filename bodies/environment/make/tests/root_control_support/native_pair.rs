//! Joint production admission over inert CPU pulse/timer owners.
use super::*;
use conduit_environment_runtime::appliance::assigned::{
    ResidentAppliance, ValidatedAssignedAppliance,
};
use conduit_monotonic_wait as w;
use conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput as Input;
use conduit_pulse_capture::{
    installation::ReadyPulseBase, owner::PulseReadProvider, PulseReadContract,
};
struct Timer(Rc<Cell<usize>>);
impl w::MonotonicWaitProvider for Timer {
    async fn wait(&mut self, _: u64) -> Result<(), w::WaitProviderFailure> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
}
pub(super) fn check<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    assignment: &ValidatedAssignedAppliance,
    prepared: &conduit_environment_make::PreparedNativeAppliance,
    resident: &ResidentAppliance<'_>,
    required: c::AssignedPlanRequirements<'_>,
    pulse: &ReadyPulseBase<P>,
    timer: &w::ReadyMonotonicWaitBase<W>,
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
        for wrong_role in [true, false] {
            let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
            let active = issuer
                .reserve_assigned_play(assignment, &fragment.plan_id, &fragment.fragment_id)
                .unwrap();
            let (pulse, pulse_effects) =
                super::super::pulse::fresh(pulse.native_identity().clone());
            let timer_effects = Rc::new(Cell::new(0));
            // SAFETY: inert, exclusive, bounded CPU slot; no hardware claim.
            let timer = unsafe {
                w::ReadyMonotonicWaitBase::new(
                    timer.native_identity().clone(),
                    w::WaitAttachment {
                        generation: 1,
                        resource_bytes: 32,
                        maximum_duration_ms: 60000,
                    },
                    Timer(timer_effects.clone()),
                )
            }
            .unwrap();
            let mut pulse = Some(pulse);
            let mut timer = Some(timer);
            let contract = PulseReadContract::prepare().unwrap();
            let mut bad_roles = resident.roles;
            if wrong_role {
                bad_roles.tick = resident.roles.pulse;
            } else {
                bad_roles.pulse = resident.roles.tick;
            }
            let bad = ResidentAppliance {
                cadence: resident.cadence.clone(),
                source: resident.source,
                layout: conduit_environment_runtime::assigned::ResidentLayout {
                    records: resident.layout.records,
                    counts: resident.layout.counts,
                },
                computations: resident.computations,
                roles: bad_roles,
            };
            let make = |resident| native_authority::AssignedNativeInput {
                assignment,
                resident,
                input,
                maxima: conduit_environment_make::NativeApplianceLimits::resident_local()
                    .assignment,
                requirements: required,
                fragment: &fragment.fragment_id,
                active: &active,
                contract: &contract,
            };
            assert!(issuer
                .bind_assigned_native(&mut pulse, &mut timer, make(&bad))
                .is_err());
            assert!(pulse.is_some() && timer.is_some());
            assert_eq!((pulse_effects.get(), timer_effects.get()), (0, 0));
            let bound: native_authority::NativeBindings<_, _> = issuer
                .bind_assigned_native(&mut pulse, &mut timer, make(resident))
                .unwrap();
            assert!(pulse.is_none() && timer.is_none());
            for inspection in [bound.pulse.inspection(), bound.wait.inspection()] {
                assert_eq!(inspection.scope.active_play_id, active.active_play_id);
                assert_eq!(
                    u64::from(inspection.scope.maximum_operations),
                    resident.cadence.count
                );
                assert_eq!(
                    (
                        inspection.completed_operations,
                        inspection.in_flight_operations
                    ),
                    (0, 0)
                );
            }
            assert_eq!((pulse_effects.get(), timer_effects.get()), (0, 0));
        }
    }
}
