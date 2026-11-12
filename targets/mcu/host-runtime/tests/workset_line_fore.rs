#![cfg(feature = "execution")]
//! Mixed representation preparation, without carrier authority or native effects.
use conduit_body as b;
use conduit_mcu_host_runtime::{
    execution::{Policy, Refusal},
    execution_workset::{
        AssignedPreparationProfile as Choice, PreparedLocalKernels, PreparedLocalWorkset,
    },
};
use std::cell::Cell;
#[allow(dead_code)]
#[path = "../../../../architecture/body/tests/delegated_line_fore/support.rs"]
mod fixture;
use fixture::*;
struct Ticket<'a>(&'a Cell<usize>);
// SAFETY: inert CPU ticket has no effects/backing; bind only counts transfer.
unsafe impl PreparedLocalKernels for Ticket<'_> {
    type Bound = ();
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    unsafe fn bind(self) {
        self.0.set(self.0.get() + 1);
    }
}
fn input(f: &Fixture) -> b::HostCommandInput<4096> {
    let bytes = mixed_workset();
    let mut input = b::HostCommandInput::new();
    input.writable().unwrap()[..bytes.len()].copy_from_slice(&bytes);
    input
        .retain(workset_command(f, &bytes).into_retained_stamp().unwrap())
        .unwrap();
    input
}
fn execution() -> b::HostExecutionDelegation {
    b::HostExecutionDelegation {
        authorizer: b::HostExecutionAuthorizer::SeedOwner,
        maximum_active_worksets: 1,
        continuation: None,
    }
}
#[test]
fn mixed_selected_profiles_reach_factory_once_without_binding_or_granting_a_line() {
    let f = Fixture::new();
    let input = input(&f);
    let preparation = policy();
    let execution = execution();
    let count = Cell::new(0);
    let calls = Cell::new(0);
    let local = [local()];
    let lines = [line(7)];
    let bindings = [binding()];
    let all = [remote(), local[0]];
    let prepared = PreparedLocalWorkset::<Ticket<'_>, 2>::prepare_selected_profiles::<4096, 2, ()>(
        &input,
        f.context(),
        Policy {
            preparation: Some(&preparation),
            execution: Some(&execution),
            active_worksets: 0,
            maximum_retained_bytes: 16384,
        },
        |index, _| {
            Ok(if index == 0 {
                Choice::LineFore(profile(&local, &lines, &bindings))
            } else {
                Choice::Legacy(legacy_profile(&all))
            })
        },
        |fragments, ports| {
            assert_eq!(fragments[0].metadata().fragment, "fragment");
            assert_eq!(fragments[1].metadata().fragment, "fragment/legacy");
            assert_eq!(ports[0].as_slice(), all);
            assert_eq!(ports[1].as_slice(), all);
            calls.set(calls.get() + 1);
            Ok(Ticket(&count))
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(count.get(), 0);
    drop(prepared);
    assert_eq!(count.get(), 0);
}
#[test]
fn stale_set_wrong_profile_boot_line_count_and_legacy_entrance_never_invoke_factory() {
    let f = Fixture::new();
    let input = input(&f);
    let preparation = policy();
    let execution = execution();
    let count = Cell::new(0);
    let calls = Cell::new(0);
    let local = [local()];
    let lines = [line(7)];
    let foreign = [line(8)];
    let bindings = [binding()];
    let all = [remote(), local[0]];
    for change in 0..5 {
        let mut context = f.context();
        if change == 0 {
            context.now_millis = 30;
        }
        let mut other = Fixture::new();
        other.host.boot_id = "boot/new".into();
        other.member.boot_id = other.host.boot_id.clone();
        if change == 1 {
            context = other.context();
        }
        let result = PreparedLocalWorkset::<Ticket<'_>, 2>::prepare_selected_profiles::<4096, 2, ()>(
            &input,
            context,
            Policy {
                preparation: Some(&preparation),
                execution: Some(&execution),
                active_worksets: 0,
                maximum_retained_bytes: 16384,
            },
            |index, _| {
                Ok(if index == 0 {
                    Choice::LineFore(profile(
                        &local,
                        if change == 2 {
                            &foreign
                        } else if change == 3 {
                            &[]
                        } else {
                            &lines
                        },
                        if change == 3 { &[] } else { &bindings },
                    ))
                } else if change == 4 {
                    Choice::LineFore(profile(&local, &lines, &bindings))
                } else {
                    Choice::Legacy(legacy_profile(&all))
                })
            },
            |_, _| {
                calls.set(calls.get() + 1);
                Ok(Ticket(&count))
            },
        );
        assert!(result.is_err());
    }
    let result = PreparedLocalWorkset::<Ticket<'_>, 2>::prepare_selected::<4096, 2, ()>(
        &input,
        f.context(),
        Policy {
            preparation: Some(&preparation),
            execution: Some(&execution),
            active_worksets: 0,
            maximum_retained_bytes: 16384,
        },
        |_, _| Ok(legacy_profile(&all)),
        |_, _| {
            calls.set(calls.get() + 1);
            Ok(Ticket(&count))
        },
    );
    assert!(matches!(result, Err(Refusal::Workset(_))));
    assert_eq!(calls.get(), 0);
    assert_eq!(count.get(), 0);
}

#[test]
fn source_pins_and_configuration_gate_line_profile_before_native_factory() {
    use conduit_core as c;
    use conduit_mcu_host_runtime::execution_group::{prepare_with_source_profiles, EntryPin};
    let f = Fixture::new();
    let projection = projection(c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let child = document(&projection, "body/1", "boot/1", &inventory(), &[]);
    let bytes = b::encode_body_workset_document(&[&child], 4096).unwrap();
    let mut input = b::HostCommandInput::<4096>::new();
    input.writable().unwrap()[..bytes.len()].copy_from_slice(&bytes);
    input
        .retain(workset_command(&f, &bytes).into_retained_stamp().unwrap())
        .unwrap();
    let preparation = policy();
    let execution = execution();
    let binds = Cell::new(0);
    let calls = Cell::new(0);
    let local = [local()];
    let lines = [line(7)];
    let bindings = [binding()];
    let absent_configuration = [c::AssignedRecordRequirement {
        tag: c::ASSIGNED_CONFIGURATION,
        payload_digest: [1; 32],
    }];
    for change in 0..3 {
        let result = prepare_with_source_profiles::<Ticket<'_>, (), 4096, 2, 1>(
            &input,
            f.context(),
            Policy {
                preparation: Some(&preparation),
                execution: Some(&execution),
                active_worksets: 0,
                maximum_retained_bytes: 16384,
            },
            [EntryPin {
                source: if change == 1 {
                    "other-source"
                } else {
                    "source"
                },
                checked: "checked",
                expanded: "expanded",
            }],
            [if change == 2 {
                &absent_configuration[..]
            } else {
                &[]
            }],
            [Choice::LineFore(profile(&local, &lines, &bindings))],
            |fragments, ports| {
                assert_eq!(fragments[0].metadata().fragment, "fragment");
                assert_eq!(ports[0], &[remote(), local[0]]);
                calls.set(calls.get() + 1);
                Ok(Ticket(&binds))
            },
        );
        assert_eq!(result.is_ok(), change == 0);
        assert_eq!(calls.get(), 1);
        assert_eq!(binds.get(), 0);
    }
}
