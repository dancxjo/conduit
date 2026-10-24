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
#[path = "native_dispatch.rs"]
mod dispatch;
struct Timer {
    effects: Rc<Cell<usize>>,
    pending: Rc<Cell<bool>>,
    active: Rc<Cell<bool>>,
    failure: Option<w::WaitProviderFailure>,
    pending_after_first: bool,
}
struct PendingWait(Rc<Cell<bool>>);
impl Drop for PendingWait {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
impl w::MonotonicWaitProvider for Timer {
    async fn wait(&mut self, _: u64) -> Result<(), w::WaitProviderFailure> {
        self.effects.set(self.effects.get() + 1);
        self.active.set(true);
        let _pending = PendingWait(self.active.clone());
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if self.pending.get() && (!self.pending_after_first || self.effects.get() > 1) {
            core::future::pending::<()>().await;
        }
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
        for behavior in 0..22 {
            let wrong_role = behavior % 2 == 0;
            let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
            let active = issuer
                .reserve_assigned_play(assignment, &fragment.plan_id, &fragment.fragment_id)
                .unwrap();
            let (pulse, pulse_effects) =
                super::super::pulse::fresh(pulse.native_identity().clone());
            let timer_effects = Rc::new(Cell::new(0));
            let pending = Rc::new(Cell::new(false));
            let active_wait = Rc::new(Cell::new(false));
            // SAFETY: inert, exclusive, bounded CPU slot; no hardware claim.
            let timer = unsafe {
                w::ReadyMonotonicWaitBase::new(
                    timer.native_identity().clone(),
                    w::WaitAttachment {
                        generation: 1,
                        resource_bytes: 32,
                        maximum_duration_ms: 60000,
                    },
                    Timer {
                        effects: timer_effects.clone(),
                        pending: pending.clone(),
                        active: active_wait.clone(),
                        pending_after_first: behavior == 5 || behavior == 9,
                        failure: match behavior {
                            2 | 10 => Some(w::WaitProviderFailure::Pressure),
                            3 | 11 => Some(w::WaitProviderFailure::ProviderUnavailable),
                            _ => None,
                        },
                    },
                )
            }
            .unwrap();
            let original_pulse_identity = pulse.native_identity().clone();
            let original_timer_identity = timer.native_identity().clone();
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
            pending.set(matches!(behavior, 1 | 5 | 8 | 9 | 13));
            dispatch::check(
                prepared,
                resident,
                required,
                bound,
                &pulse_effects,
                &timer_effects,
                &active_wait,
                behavior,
                |retired| {
                    assert!(retired.matches_active(&active));
                    let mut newer_pulse = original_pulse_identity.clone();
                    let mut newer_timer = original_timer_identity.clone();
                    newer_pulse.provider_generation += 1;
                    newer_timer.provider_generation += 1;
                    // A separately sealed fixture Plan selects the newer Base
                    // generations. Metadata alone is still not native possession.
                    let mut updated = fragment.clone();
                    for gear in &mut updated.placements {
                        if gear.kind_id.as_str() == conduit_time::TICK_KIND
                            || gear.kind_id.as_str() == conduit_pulse_capture::PULSE_READ_KIND
                        {
                            gear.base.as_mut().unwrap().provider_generation += 1;
                        }
                    }
                    let updated = c::seal_plan(
                        c::PlotIdentity {
                            source_document_id: updated.source_document_id.clone(),
                            checked_plot_id: updated.checked_plot_id.clone(),
                            expanded_plot_id: updated.expanded_plot_id.clone(),
                        },
                        vec![updated],
                    )
                    .fragments
                    .remove(0);
                    let interleaved = if behavior == 14 {
                        Some(issuer.reserve_play(&updated).unwrap())
                    } else {
                        None
                    };
                    let mut foreign = active.clone();
                    foreign.active_play_id = "foreign/play".into();
                    assert!(issuer.finish_native_play(&foreign, &retired).is_err());
                    let next_sequence = issuer.next_play_sequence();
                    let (finished, memory) =
                        allocation_probe::observe(|| issuer.finish_native_play(&active, &retired));
                    assert_eq!(finished, Ok(()));
                    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
                    assert_eq!(issuer.next_play_sequence(), next_sequence);
                    assert!(issuer.finish_native_play(&active, &retired).is_err());
                    if let Some(other) = interleaved {
                        // A later independent reservation must survive finishing
                        // this actual earlier native Play.
                        assert!(issuer
                            .issue_pulse(&newer_pulse, &updated, &other, &resident.cadence)
                            .is_ok());
                        assert!(issuer
                            .issue_wait(&newer_timer, &updated, &other, &resident.cadence)
                            .is_ok());
                        return;
                    }

                    let next = issuer
                        .reserve_assigned_play(assignment, &fragment.plan_id, &fragment.fragment_id)
                        .unwrap();
                    assert_eq!(next.play_sequence, active.play_sequence + 1);
                    assert!(!retired.matches_active(&next));
                    let pulse_identity = original_pulse_identity.clone();
                    let timer_identity = original_timer_identity.clone();
                    assert!(issuer
                        .issue_pulse(&pulse_identity, fragment, &next, &resident.cadence)
                        .is_err());
                    assert!(issuer
                        .issue_wait(&timer_identity, fragment, &next, &resident.cadence)
                        .is_err());
                    let next = issuer.reserve_play(&updated).unwrap();
                    assert_eq!(next.play_sequence, active.play_sequence + 2);

                    assert!(issuer
                        .issue_pulse(&newer_pulse, &updated, &next, &resident.cadence)
                        .is_ok());
                    assert!(issuer
                        .issue_wait(&newer_timer, &updated, &next, &resident.cadence)
                        .is_ok());
                    // These are CPU fixture capability issuances only. No returned
                    // provider is rebound, no ready offer appears and no second
                    // Play runs.
                },
            );
        }
    }
}
