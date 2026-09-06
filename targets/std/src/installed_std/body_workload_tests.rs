//! Canonical workload composition through installed operations, not UI or HIL proof.
#[path = "body_run_cancellation_tests.rs"]
mod cancellation;
use super::*;
use crate::installed_std::{kernel_preparation::KernelTables, simple_presentation_host};
use conduit_body::{Body, BodyFormPlan, BodyPlan, BodyPlayIdentity, ResidentForm};
use conduit_core::{resource_offer, HostAdvertisement, Plan, SignId};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_kernel::scheduler::SchedulerStatus;
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationOutcome, HostedSignLog,
};
use conduit_plan_lowering::fragment_set::{lower_local_fragment_set, FragmentSetBounds};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE;

fn workload() -> (HostAdvertisement, Vec<Plan>) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    let mut host = crate::StdHost::new().advertisement().clone();
    for offer in [
        conduit_std_offers::button::offer(),
        conduit_std_offers::button::mapper_offer(),
        conduit_std_offers::button::indicator_offer(),
    ] {
        if !host
            .capabilities
            .iter()
            .any(|existing| existing.capability_id == offer.capability_id)
        {
            host.capabilities.push(offer);
        }
    }
    host.resources.push(resource_offer(
        "proof/body-keyboard",
        conduit_core::INPUT_RESOURCE_CLASS,
        1,
    ));
    host.resources.sort();
    host.capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let sources = [
        (
            "button_across_room",
            include_str!("../../../../forms/button-across-room/main.conduit"),
        ),
        (
            "clock-demo",
            include_str!("../../../../forms/clock/main.conduit"),
        ),
        (
            "desk_telegraph",
            include_str!("../../../../forms/desk-telegraph/main.conduit"),
        ),
    ];
    let plans = sources
        .into_iter()
        .map(|(entry, source)| {
            let syntax = parse_syntax_document(source);
            assert_eq!(syntax.round_trip(), source);
            let checked = check_syntax_document(&syntax, &startup).unwrap();
            let expanded = expand_canonical_form(&checked, entry, &profile).unwrap();
            let hosts = [host.clone()];
            let placements =
                conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
            let limits = expanded
                .connections
                .iter()
                .map(|connection| {
                    let bytes = if connection.value_kind
                        == conduit_semantic_catalog::button_source_contract().outputs[0].value_kind
                    {
                        conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES
                    } else if connection.value_kind.as_str() == conduit_core::BOOL_INFO_ID {
                        1
                    } else {
                        64
                    };
                    (
                        (
                            connection.source_gear_id.clone(),
                            connection.source_port_id.clone(),
                            connection.sink_gear_id.clone(),
                            connection.sink_port_id.clone(),
                        ),
                        conduit_planner::ConnectionQueueLimits {
                            item_capacity: 1,
                            byte_capacity: bytes,
                        },
                    )
                })
                .collect();
            conduit_planner::plan_expanded_canonical_with_connection_limits(
                &expanded,
                &hosts,
                &placements,
                &["conduit.base/local@1".into()],
                conduit_planner::PlanningOptions {
                    connection_bases: &Default::default(),
                    line_candidates: &Default::default(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: 1,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                &limits,
            )
            .unwrap()
        })
        .collect();
    (host, plans)
}

#[test]
fn canonical_button_clock_and_telegraph_share_admission_and_one_installed_kernel() {
    let (host, plans) = workload();
    let first = &plans[0];
    let mut body = Body::born(
        first.source_document_id.clone(),
        first.checked_form_id.clone(),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap();
    for (index, plan) in plans.iter().enumerate().skip(1) {
        body = body
            .admit_form(
                ResidentForm::new(
                    plan.source_document_id.clone(),
                    plan.checked_form_id.clone(),
                ),
                SignId::from(format!("sign/admit-{index}")),
            )
            .unwrap();
    }
    let wake = body.wake(1, SignId::from("sign/wake")).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        plans
            .into_iter()
            .map(|plan| BodyFormPlan {
                form: ResidentForm::new(
                    plan.source_document_id.clone(),
                    plan.checked_form_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .unwrap();
    plan.validate_for(&wake).unwrap();
    let original = plan.clone();
    let play = BodyPlayIdentity::bind(&plan, 1);
    assert!(play.validate_for(&plan));
    let fragments: Vec<_> = plan
        .forms
        .iter()
        .map(|part| {
            assert_eq!(part.plan.fragments.len(), 1);
            &part.plan.fragments[0]
        })
        .collect();
    let mut ledger = crate::kernel_preparation::KernelResourceLedger::new(&host).unwrap();
    let requests: Vec<_> = fragments
        .iter()
        .map(|fragment| (*fragment, false))
        .collect();
    let reservations = ledger
        .prepare_and_reserve_partitions(&host, &requests)
        .unwrap();
    let lowered = lower_local_fragment_set(
        &fragments,
        FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 3,
            nodes: 7,
            cords: 4,
            queue_slots: 64,
            value_bytes: fragments
                .iter()
                .flat_map(|fragment| &fragment.connections)
                .try_fold(0_u32, |total, cord| total.checked_add(cord.byte_capacity))
                .unwrap(),
            sign_items: 2048,
            sign_bytes: 262144,
        },
    )
    .unwrap();
    let budgets: Vec<_> = fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| operation_budget(placement).unwrap())
        .collect();
    let value_items = budgets
        .iter()
        .try_fold(0_u16, |total, budget| total.checked_add(budget.value_items))
        .unwrap();
    let value_bytes = budgets
        .iter()
        .try_fold(0_u32, |total, budget| total.checked_add(budget.value_bytes))
        .unwrap();
    let maximum_value_bytes = budgets
        .iter()
        .map(|budget| budget.maximum_value_bytes)
        .max()
        .unwrap();
    let mut values = HostedValueStore::new(value_items, maximum_value_bytes, value_bytes).unwrap();
    let mut drivers =
        core::array::from_fn(|_| OperationDriver::new(InstalledOperation::inactive()).unwrap());
    for (fragment, part) in fragments.iter().zip(&lowered.partitions) {
        for node in &part.nodes {
            drivers[usize::from(node.node.0)] = OperationDriver::new(
                prepare_ordinary_operation(fragment, &node.placement_id, &mut values).unwrap(),
            )
            .unwrap();
        }
    }
    let parts: Vec<_> = lowered.partitions.iter().collect();
    let mut kernel = KernelTables::prepare(&parts)
        .unwrap()
        .install(
            drivers,
            values,
            HostedSignLog::new(
                2048,
                2048 * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
            )
            .unwrap(),
        )
        .unwrap();
    let playing = wake
        .body_plan_ready(&plan, SignId::from("sign/plan-ready"))
        .unwrap()
        .body_play_started(&plan, &play, SignId::from("sign/play-started"))
        .unwrap();
    assert_eq!(playing.body_id, body.body_id);
    let mut keys = [[0x2c_u8, 0, 0], [0x2c, 1, 0]].into_iter();
    let mut output = Vec::with_capacity(1024);
    let mut completed = false;
    for _ in 0..512 {
        while let Some(request) = kernel.next_host_request() {
            let operation = lowered
                .partitions
                .iter()
                .flat_map(|part| &part.host_operations)
                .find(|op| op.node == request.node && op.operation == request.operation)
                .unwrap();
            let mut result = HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            };
            if operation.contract_id.as_str()
                == conduit_std_offers::NEXT_KEY_EVENT_HOST_OPERATION_CONTRACT
            {
                let bytes = keys
                    .next()
                    .expect("only the two admitted button transitions");
                let value = kernel.store_host_value(&bytes).unwrap();
                result.output = Some(BoundedValueRef::new(value, 3).unwrap());
            } else if operation.contract_id
                == conduit_core::wait_host_operation_requirement().contract_id
            {
                // Deterministic timer completion, not wall-clock or physical proof.
                conduit_time::decode_tick(kernel.host_value(request.input.value).unwrap()).unwrap();
            } else {
                assert!(simple_presentation_host::present(
                    operation.target_kind.as_ref(),
                    kernel.host_value(request.input.value).unwrap(),
                    &mut output
                )
                .unwrap());
            }
            kernel
                .complete_host_operation(request.node, request.request, result)
                .unwrap();
        }
        if kernel.step().unwrap() == SchedulerStatus::Complete {
            completed = true;
            break;
        }
    }
    assert!(completed);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("CALLING\n"));
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("bool value="))
            .collect::<Vec<_>>(),
        ["bool value=true", "bool value=false"]
    );
    for sequence in 0..4 {
        assert!(output.contains(&format!("tick sequence={sequence}\n")));
    }
    assert_eq!(plan, original);
    for reservation in reservations {
        ledger.release(reservation).unwrap();
    }
}

#[test]
fn production_body_entry_executes_and_preserves_failed_and_refused_outcomes() {
    use crate::body_execution::BodyRunRequest;
    use crate::hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll};
    struct Keys(std::collections::VecDeque<[u8; 3]>);
    impl HostedKeyboardAdapter for Keys {
        fn poll_next(&mut self) -> HostedKeyboardPoll {
            self.0
                .pop_front()
                .map_or(HostedKeyboardPoll::Cancelled, |bytes| {
                    HostedKeyboardPoll::Event(conduit_human::KeyEvent::decode(&bytes).unwrap())
                })
        }
    }
    struct Clock;
    impl crate::TimerAdapter for Clock {
        fn wait(&mut self, _: std::time::Duration) {}
    }
    let (advertisement, plans) = workload();
    let mut host = crate::StdHost::from_advertisement(advertisement).unwrap();
    let first = &plans[0];
    let mut body = Body::born(
        first.source_document_id.clone(),
        first.checked_form_id.clone(),
        1,
        SignId::from("sign/production-body"),
    )
    .unwrap();
    for (index, part) in plans.iter().enumerate().skip(1) {
        body = body
            .admit_form(
                ResidentForm::new(
                    part.source_document_id.clone(),
                    part.checked_form_id.clone(),
                ),
                SignId::from(format!("sign/production-admit-{index}")),
            )
            .unwrap();
    }
    let wake = body
        .wake(1, SignId::from("sign/production-wake"))
        .unwrap()
        .1;
    let plan = BodyPlan::seal(
        &wake,
        plans
            .into_iter()
            .map(|plan| BodyFormPlan {
                form: ResidentForm::new(
                    plan.source_document_id.clone(),
                    plan.checked_form_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .unwrap();
    let original = plan.clone();
    let control = crate::RunControl::default();
    let mut output = Vec::with_capacity(2048);
    assert!(host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None
            },
            &mut output,
            &mut Clock
        )
        .is_err());
    assert!(output.is_empty());
    let mut bad_keys = Keys([[0x2c, 1, 0]].into());
    let failed = host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: Some(&mut bad_keys),
            },
            &mut output,
            &mut Clock,
        )
        .unwrap();
    assert!(matches!(
        failed.terminal,
        conduit_core::TerminalDisposition::Failed { .. }
    ));
    assert!(failed.failure.is_some());
    assert!(failed.cleanup_failure.is_none());
    assert!(failed
        .kernel_events
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::RunCancelled));
    output.clear();
    let mut keys = Keys([[0x2c, 0, 0], [0x2c, 1, 0]].into());
    let report = host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: Some(&mut keys),
            },
            &mut output,
            &mut Clock,
        )
        .unwrap();
    assert_eq!(
        report.terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert!(report.failure.is_none());
    assert!(report.play.validate_for(&plan));
    assert_ne!(report.play.active_play_id, failed.play.active_play_id);
    assert_eq!(
        report.terminal_sign.active_play_id,
        Some(report.play.active_play_id.clone())
    );
    assert_eq!(report.partitions.len(), 3);
    for request in &report.requests {
        assert_eq!(
            report
                .partitions
                .iter()
                .filter(|partition| partition.placement_for_node(request.node).is_some())
                .count(),
            1
        );
    }
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("CALLING\n"));
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("bool value="))
            .collect::<Vec<_>>(),
        ["bool value=true", "bool value=false"]
    );
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("tick sequence="))
            .count(),
        4
    );
    let stop = crate::RunControl::default();
    stop.request_stop(crate::RunControlRequestId::new("stop-before-body-effects").unwrap())
        .unwrap();
    let mut untouched = Keys([[0x2c, 0, 0], [0x2c, 1, 0]].into());
    let mut cancelled_output = Vec::new();
    let cancelled = host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &stop,
                keyboard: Some(&mut untouched),
            },
            &mut cancelled_output,
            &mut Clock,
        )
        .unwrap();
    assert!(matches!(
        cancelled.terminal,
        conduit_core::TerminalDisposition::Cancelled { .. }
    ));
    assert!(cancelled.failure.is_none());
    assert!(cancelled_output.is_empty());
    assert_eq!(untouched.0.len(), 2);
    assert_eq!(plan, original);
}
