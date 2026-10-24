//! Production Root binding with pins that panic on electrical effects.
use super::*;

pub(super) fn check(
    assignment: &ValidatedAssignedDisplay,
    fragment: &c::PlanFragment,
    bytes: &[u8],
    records: &[c::AssignedRecord<'_>],
    profile: &AssignedDisplayProfile<'_>,
    prepared: native_display_kernel::Prepared,
) {
    use conduit_environment_runtime::appliance::{
        assigned::ResidentSourceIdentity, assignment_transfer as t,
    };
    use native_display_binding::{bind, BindingInput};
    let resident: Vec<_> = records
        .iter()
        .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
        .copied()
        .collect();
    let mut wire = vec![0; t::MAXIMUM_WIRE_BYTES];
    let size = t::encode(
        &bytes[..125],
        records,
        ResidentSourceIdentity::from_expanded(&profile.source.expanded_plot_id),
        &resident,
        &mut wire,
    )
    .unwrap();
    wire.truncate(size);
    fixtures::scope_mode(3);
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    fixtures::scope_mode(0);
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    issuer
        .reserve_display_play_matching(
            assignment,
            &fragment.plan_id,
            &fragment.fragment_id,
            active.active_play_id.as_str(),
        )
        .unwrap();
    let mut owner = Some(display_fixture::ready(&fragment.host_id, &fragment.boot_id));
    let input = |available_bytes| BindingInput {
        assignment,
        wire: &wire,
        profile,
        resident: &resident,
        fragment: &fragment.fragment_id,
        active: &active,
        maximum_frames: 2,
        available_bytes,
    };
    assert!(bind(&mut issuer, &mut owner, input(0)).is_err());
    assert!(owner.is_some());
    let bound = bind(&mut issuer, &mut owner, input(usize::MAX)).unwrap();
    assert!(owner.is_none());
    assert_eq!(bound.inspection().completed_operations, 0);
    assert_eq!(
        bound.inspection().scope.active_play_id,
        active.active_play_id
    );
    assert!(bind(&mut issuer, &mut owner, input(usize::MAX)).is_err());

    let mut storage = Some(prepared);
    let mut bound = Some(bound);
    let mut missing = None;
    assert!(display_execution::Execution::boxed_retained(&mut missing, &mut bound).is_err());
    assert!(bound.is_some());
    let mut execution =
        Some(display_execution::Execution::boxed_retained(&mut storage, &mut bound).unwrap());
    assert!(storage.is_none() && bound.is_none());
    assert_eq!(
        execution
            .as_ref()
            .unwrap()
            .inspection()
            .completed_operations,
        0
    );
    assert_eq!(
        execution.as_ref().unwrap().progress(),
        display_execution::Progress::default()
    );
    let outcome = run(execution.as_mut().unwrap().run(core::future::ready(())));
    assert_eq!(outcome.exit, display_execution::Exit::Cancelled);
    assert_eq!(outcome.cleanup, Ok(()));
    assert_eq!(outcome.progress.steps, 0);
    let replay = run(execution.as_mut().unwrap().run(core::future::ready(())));
    assert!(matches!(replay.exit, display_execution::Exit::Failed(_)));
    let retired = display_execution::Execution::retire_retained(&mut execution).unwrap();
    assert!(execution.is_none());
    issuer.finish_display_play(&active, &retired).unwrap();
    assert!(issuer.finish_display_play(&active, &retired).is_err());
    execution_checks::check(assignment, fragment, &wire, profile, &resident);
}

#[path = "display_execution.rs"]
mod execution_checks;
