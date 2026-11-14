//! Bind ordinary lowered service terminal laws into the production scheduler.
use super::*;
use crate::selection_fixture;
use conduit_core as c;

fn fixture() -> Fixture<{ crate::read_result::READ_RESULT_BYTES }> {
    let kind = crate::service_contract::definition();
    let mut selected = shape();
    selected.append.maximum_input_bytes = 200;
    selected.append.maximum_output_bytes = crate::append_result::APPEND_RESULT_BYTES;
    selected.read.maximum_output_bytes = crate::read_result::READ_RESULT_BYTES as u32;
    let mut f = Fixture::with_back(RecordServiceBack::new(selected).unwrap());
    let scope = c::BaseCapabilityScope {
        host_id: "host".into(),
        boot_id: "boot".into(),
        base_instance_id: "provider".into(),
        base_provider_generation: 1,
        plan_id: "plan".into(),
        active_play_id: "play".into(),
        authority_grant_id: "grant/append".into(),
        authority_contract_id: crate::native_append::APPEND_AUTHORITY.into(),
        capability_id: crate::native_service::SERVICE_CAPABILITY.into(),
        implementation_id: "service@1".into(),
        operation_contract_id: crate::native_append::APPEND_OPERATION.into(),
        subject_kind: crate::append_contract::APPEND_KIND.into(),
        resource_pool_id: "residence".into(),
        resource_generation_id: c::ResourceGenerationId("generation/1".into()),
        envelope_id: "envelope".into(),
        maximum_parameter_bytes: 200,
        maximum_result_bytes: crate::append_result::APPEND_RESULT_BYTES,
        maximum_work_units: 6,
        maximum_in_flight: 1,
        maximum_operations: 8,
    };
    let (mut fragment, _, _, _) = selection_fixture::selected(
        &scope,
        &kind,
        crate::native_append::APPEND_OPERATION,
        200,
        "residence",
        "service",
    );
    let gear = &mut fragment.placements[0];
    gear.terminal_transductions = kind.terminal_transductions().cloned().collect();
    gear.host_calls = vec![
        c::HostCallRequirement {
            contract_id: crate::native_service::READ_OPERATION.into(),
            target_kind: Some(crate::read_contract::READ_KIND.into()),
            maximum_in_flight: 1,
            maximum_input_bytes: 8,
            maximum_output_bytes: crate::read_result::READ_RESULT_BYTES as u32,
        },
        c::HostCallRequirement {
            contract_id: crate::native_append::APPEND_OPERATION.into(),
            target_kind: Some(crate::append_contract::APPEND_KIND.into()),
            maximum_in_flight: 1,
            maximum_input_bytes: 200,
            maximum_output_bytes: crate::append_result::APPEND_RESULT_BYTES,
        },
    ];
    gear.authority.push(c::AuthorityBinding {
        grant_id: "grant/read".into(),
        contract_id: crate::native_service::READ_AUTHORITY.into(),
        host_call_contract_id: crate::native_service::READ_OPERATION.into(),
        subject_kind: crate::read_contract::READ_KIND.into(),
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        capability_id: scope.capability_id.clone(),
    });
    let fragment = c::seal_plan(
        c::PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
    .fragments
    .remove(0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    let mut profiles = [None; 2];
    for profile in &lowered.nodes[0].terminal_transductions {
        profiles[usize::from(profile.input.0)] = Some(profile.assigned());
    }
    assert!(profiles.iter().all(Option::is_some));
    f.kernel.bind_terminal_transductions([profiles]).unwrap();
    f
}
fn close<const N: usize>(f: &mut Fixture<N>, operation: usize) {
    f.kernel
        .close_remote_input(RemoteEndpointId(operation as u16), CordId(operation as u16))
        .unwrap();
}
#[test]
fn one_input_closes_without_stopping_other_operation_and_last_close_completes() {
    for first in 0..2 {
        let mut f = fixture();
        close(&mut f, first);
        f.step();
        assert!(f.kernel.drivers()[0].back.closed[first]);
        let other = 1 - first;
        f.input(other);
        let request = f.request(other, 0);
        f.complete(request, b"result");
        f.step();
        if other == 1 {
            f.step();
        }
        assert_eq!(f.output(other).unwrap(), b"result");
        f.drain(other);
        close(&mut f, other);
        assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Drained);
        assert_eq!(f.kernel.drivers()[0].back.closed, [true; 2]);
        assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Drained);
    }
}
#[test]
fn final_closure_waits_for_pending_call_and_blocked_read_response_drain() {
    let mut f = fixture();
    f.input(1);
    let old = f.request(1, 0);
    f.complete(old, b"old");
    f.step();
    f.step();
    close(&mut f, 0);
    f.step(); // Append side closed; read still works.
    f.input(1);
    let pending = f.request(1, 1);
    close(&mut f, 1);
    assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Idle);
    f.complete(pending, b"last");
    f.step();
    assert_eq!(f.kernel.drivers()[0].back.closed, [true; 2]);
    assert_eq!(f.view.get().read_len, Some(4));
    f.step(); // The admitted finite flush phase waits for read output.
    assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Idle);
    assert_eq!(f.kernel.drivers()[0].back.closed, [true; 2]);
    assert_eq!(f.output(1).unwrap(), b"old");
    f.drain(1);
    f.step();
    assert_eq!(f.output(1).unwrap(), b"last");
    // Publication commits Complete, with the last output retained in its Cord.
    assert_eq!(f.kernel.drivers()[0].back.closed, [true; 2]);
    assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Idle); // output not yet consumed
    f.drain(1);
    assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Drained);
}

#[test]
fn final_close_flushes_both_pending_append_receipt_and_retained_read_response() {
    let mut f = fixture();
    f.input(1);
    let old = f.request(1, 0);
    f.complete(old, b"old");
    f.step();
    f.step();
    f.input(1);
    let read = f.request(1, 1);
    f.complete(read, b"held");
    f.step();
    close(&mut f, 1);
    f.step();
    assert_eq!(f.kernel.drivers()[0].back.closed, [false, true]);
    assert_eq!(f.view.get().read_len, Some(4));
    f.input(0);
    let append = f.request(0, 2);
    close(&mut f, 0);
    f.complete(append, b"appended");
    f.step(); // First publication in the last input's finite flush phase.
    assert_eq!(f.kernel.drivers()[0].back.closed, [true; 2]);
    assert_eq!(f.output(0).unwrap(), b"appended");
    assert_eq!(f.output(1).unwrap(), b"old");
    assert_eq!(f.view.get().read_len, Some(4));
    f.step();
    assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Idle);
    f.drain(1);
    f.step(); // Second publication completes the same admitted flush phase.
    assert_eq!(f.output(1).unwrap(), b"held");
    assert_eq!(f.view.get().read_len, None);
    assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Idle);
    f.drain(0);
    f.drain(1);
    assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Drained);
}
