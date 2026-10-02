use super::*;

#[derive(Default)]
struct ScheduledTimer {
    now_ms: u64,
    deadlines: Vec<u64>,
    regress_after_wait: bool,
    late_by_ms: u64,
}

impl TimerAdapter for ScheduledTimer {
    fn wait(&mut self, duration: Duration) {
        self.now_ms = self
            .now_ms
            .saturating_add(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX));
    }

    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.now_ms)
    }

    fn wait_until_monotonic_ms(&mut self, deadline_ms: u64) -> bool {
        self.deadlines.push(deadline_ms);
        self.now_ms = if self.regress_after_wait {
            self.regress_after_wait = false;
            self.now_ms.saturating_sub(1)
        } else {
            deadline_ms.saturating_add(self.late_by_ms)
        };
        true
    }
}

const DEBOUNCE_FORM: &str = "plot robot-debounce {\n    switch: test/timing-bool-source\n    stable: time/debounce(duration-ms = 5ms, policy = \"trailing\", maximum-values = 3)\n    sink: test/timing-bool-sink\n    switch >> stable >> sink\n}\n";

const TIMEOUT_FORM: &str = "plot robot-timeout {\n    clock: time/tick(count = 2, period-ms = 10)\n    stale: time/timeout(duration-ms = 7ms, maximum-values = 2)\n    sink: test/timing-bool-sink\n    clock >> stale >> sink\n}\n";

const DELAY_FORM: &str = "plot ordinary-delay {\n    source: test/timing-bool-source\n    paced: time/delay(duration-ms = 5ms, maximum-values = 3)\n    sink: test/timing-bool-sink\n    source >> paced >> sink\n}\n";

const THROTTLE_FORM: &str = "plot patchbay-refresh-throttle {\n    edits: test/timing-bool-source\n    refresh: time/throttle(duration-ms = 5ms, policy = \"leading\", maximum-values = 3)\n    presenter: test/timing-bool-sink\n    edits >> refresh >> presenter\n}\n";

const DEADLINE_CANCELLATION_FORM: &str = "plot deadline-cancel {\n    trigger: test/timing-unit-source\n    deadline: time/deadline(duration-ms = 1ms)\n    operation: audio/tone\n    recovery: conduit-test/tone-terminal-recovery\n    trigger >> deadline.arm\n    deadline.request >> operation~\n    operation.audio! >> recovery.terminal\n}.\n";

fn fragment(host: &StdHost, source: &str) -> conduit_core::PlanFragment {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut startup_profile = conduit_plot::ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut startup_profile)
        .expect("tick startup signature installs");
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut startup_profile)
        .expect("tick presentation signature installs");
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut startup_profile)
        .expect("timing startup signatures install");
    startup
        .insert(conduit_plot::KindSignature {
            kind: "test/timing-bool-source".to_string(),
            startup_parameters: Vec::new(),
        })
        .expect("test source startup signature is unique");
    startup
        .insert(conduit_plot::KindSignature {
            kind: "test/timing-unit-source".to_string(),
            startup_parameters: Vec::new(),
        })
        .expect("test unit source startup signature is unique");
    for kind in ["audio/tone", "conduit-test/tone-terminal-recovery"] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.to_string(),
                startup_parameters: Vec::new(),
            })
            .expect("deadline cancellation proof signature is unique");
    }
    startup
        .insert(conduit_plot::KindSignature {
            kind: "test/timing-bool-sink".to_string(),
            startup_parameters: Vec::new(),
        })
        .expect("test timing sink startup signature is unique");
    let syntax = conduit_plot::parse_syntax_document(source);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .expect("canonical timing Plot checks");
    let profile = installed_std::test_catalog();
    let expanded = conduit_plot::expand_canonical_plot(&checked, &checked.plots[0].name, &profile)
        .expect("canonical timing Plot expands");
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .expect("timing placements resolve");
    conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("timing Plot plans through the ordinary planner")
    .fragments
    .into_iter()
    .next()
    .expect("local timing Plan has one Fragment")
}

fn run(source: &str, id: &str) -> (crate::StdRunReport, ScheduledTimer) {
    let mut host = host(id);
    let fragment = fragment(&host, source);
    let mut output = Vec::with_capacity(4_096);
    let mut timer = ScheduledTimer {
        now_ms: 0,
        deadlines: Vec::with_capacity(16),
        regress_after_wait: false,
        late_by_ms: 0,
    };
    let report = host
        .run_fragment_to(fragment, &mut output, &mut timer)
        .expect("timing Plot executes through the production kernel");
    (report, timer)
}

#[test]
fn representative_robot_debounce_and_timeout_run_through_one_production_kernel() {
    for (source, id, kind) in [
        (
            DEBOUNCE_FORM,
            "robot-debounce",
            conduit_semantic_catalog::TIME_DEBOUNCE_KIND,
        ),
        (
            TIMEOUT_FORM,
            "robot-timeout",
            conduit_semantic_catalog::TIME_TIMEOUT_KIND,
        ),
        (
            DELAY_FORM,
            "ordinary-delay",
            conduit_semantic_catalog::TIME_DELAY_KIND,
        ),
        (
            THROTTLE_FORM,
            "patchbay-refresh-throttle",
            conduit_semantic_catalog::TIME_THROTTLE_KIND,
        ),
    ] {
        let mut planned_host = host(id);
        let planned = fragment(&planned_host, source);
        let timing = planned
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .expect("canonical timing placement exists");
        assert_eq!(timing.host_calls.len(), 1);
        assert_eq!(
            timing.host_calls[0].contract_id,
            conduit_core::MONOTONIC_TIMER_HOST_CALL_CONTRACT.into()
        );
        assert_eq!(timing.resources.len(), 1);

        let mut output = Vec::with_capacity(4_096);
        let mut timer = ScheduledTimer {
            now_ms: 0,
            deadlines: Vec::with_capacity(16),
            regress_after_wait: false,
            late_by_ms: 0,
        };
        let report = planned_host
            .run_fragment_to(planned, &mut output, &mut timer)
            .unwrap_or_else(|error| panic!("representative {kind} Plot completes: {error}"));
        let kernel = report.kernel.expect("production kernel report exists");
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        if kind == conduit_semantic_catalog::TIME_THROTTLE_KIND {
            assert!(kernel.kernel_sign.iter().any(|event| event.kind
                == conduit_kernel::KernelEventKind::HostCallCancellationRequested));
        } else {
            assert!(!timer.deadlines.is_empty(), "{kind} requested no deadlines");
        }
    }
}

#[test]
fn identical_simulated_schedules_have_identical_normalized_output_and_signs() {
    for (source, id) in [
        (DEBOUNCE_FORM, "repeat-debounce"),
        (TIMEOUT_FORM, "repeat-timeout"),
        (DELAY_FORM, "repeat-delay"),
        (THROTTLE_FORM, "repeat-throttle"),
    ] {
        let (left, left_timer) = run(source, id);
        let (right, right_timer) = run(source, id);
        assert_eq!(left_timer.deadlines, right_timer.deadlines);
        assert_eq!(left.observations, right.observations);
        assert_eq!(left.receipts, right.receipts);
        assert_eq!(
            left.kernel.expect("left kernel report").kernel_sign,
            right.kernel.expect("right kernel report").kernel_sign
        );
    }
}

#[test]
fn missing_or_regressed_monotonic_base_fails_deterministically() {
    let baseline = host("missing-deadline-base");
    let planned = fragment(&baseline, TIMEOUT_FORM);
    let mut output = Vec::with_capacity(4_096);
    let mut unavailable = RecordingTimer {
        waits: Vec::with_capacity(4),
    };
    let mut missing_host = baseline;
    let error = missing_host
        .run_fragment_to(planned, &mut output, &mut unavailable)
        .expect_err("an unavailable monotonic Base cannot execute a deadline");
    assert!(
        error.contains("monotonic deadline Base is unavailable"),
        "unexpected missing-Base failure: {error}"
    );

    let baseline = host("regressed-deadline-base");
    let planned = fragment(&baseline, TIMEOUT_FORM);
    let mut regressed = ScheduledTimer {
        now_ms: 1,
        deadlines: Vec::with_capacity(4),
        regress_after_wait: true,
        late_by_ms: 0,
    };
    let mut regressed_host = baseline;
    let error = regressed_host
        .run_fragment_to(planned, &mut output, &mut regressed)
        .expect_err("a regressed timing basis cannot fire a deadline");
    assert!(
        error.contains("regressed or became stale"),
        "unexpected regressed-Base failure: {error}"
    );
}

#[test]
fn simultaneous_input_deadline_order_is_deterministic_and_late_wakes_remain_correlated() {
    let simultaneous = TIMEOUT_FORM.replace("7ms", "10ms");
    let mut simultaneous_host = host("simultaneous-timeout");
    let planned = fragment(&simultaneous_host, &simultaneous);
    let mut output = Vec::with_capacity(4_096);
    let mut timer = ScheduledTimer {
        now_ms: 0,
        deadlines: Vec::with_capacity(16),
        regress_after_wait: false,
        late_by_ms: 0,
    };
    let first = simultaneous_host
        .run_fragment_to(planned, &mut output, &mut timer)
        .expect("simultaneous input/deadline schedule completes");
    let mut repeated_host = host("simultaneous-timeout");
    let repeated_plan = fragment(&repeated_host, &simultaneous);
    let mut repeated_timer = ScheduledTimer {
        now_ms: 0,
        deadlines: Vec::with_capacity(16),
        regress_after_wait: false,
        late_by_ms: 0,
    };
    let repeated = repeated_host
        .run_fragment_to(repeated_plan, &mut output, &mut repeated_timer)
        .expect("repeated simultaneous schedule completes");
    assert_eq!(timer.deadlines, repeated_timer.deadlines);
    assert_eq!(
        first.kernel.expect("first kernel report").kernel_sign,
        repeated.kernel.expect("repeated kernel report").kernel_sign
    );

    let mut late_host = host("late-timeout");
    let planned = fragment(&late_host, TIMEOUT_FORM);
    let mut late_timer = ScheduledTimer {
        now_ms: 0,
        deadlines: Vec::with_capacity(16),
        regress_after_wait: false,
        late_by_ms: 5,
    };
    late_host
        .run_fragment_to(planned, &mut output, &mut late_timer)
        .expect("late monotonic wake still completes the correlated request");
    assert!(!late_timer.deadlines.is_empty());
}

#[test]
fn zero_and_maximum_duration_schedules_are_deterministic() {
    let maximum = conduit_semantic_catalog::TIME_MAXIMUM_DURATION_MS;
    let zero = TIMEOUT_FORM
        .replace("period-ms = 10", "period-ms = 0")
        .replace("7ms", "0ms");
    let maximum = TIMEOUT_FORM
        .replace("period-ms = 10", &format!("period-ms = {maximum}"))
        .replace("7ms", &format!("{maximum}ms"));

    for (source, id) in [
        (zero.as_str(), "zero-duration-timeout"),
        (maximum.as_str(), "maximum-duration-timeout"),
    ] {
        let (first, first_timer) = run(source, id);
        let (repeated, repeated_timer) = run(source, id);
        assert_eq!(first_timer.deadlines, repeated_timer.deadlines);
        assert_eq!(
            first.kernel.expect("first kernel report").kernel_sign,
            repeated.kernel.expect("repeated kernel report").kernel_sign
        );
    }
}

#[test]
fn delay_zero_maximum_and_late_wakes_preserve_exact_ordered_schedule() {
    let maximum = conduit_semantic_catalog::TIME_MAXIMUM_DURATION_MS;
    for (source, id, late_by_ms) in [
        (DELAY_FORM.replace("5ms", "0ms"), "zero-duration-delay", 0),
        (
            DELAY_FORM.replace("5ms", &format!("{maximum}ms")),
            "maximum-duration-delay",
            0,
        ),
        (DELAY_FORM.to_string(), "late-delay", 7),
    ] {
        let mut host = host(id);
        let planned = fragment(&host, &source);
        let mut output = Vec::with_capacity(4_096);
        let mut timer = ScheduledTimer {
            now_ms: 0,
            deadlines: Vec::with_capacity(8),
            regress_after_wait: false,
            late_by_ms,
        };
        let report = host
            .run_fragment_to(planned, &mut output, &mut timer)
            .expect("bounded delay schedule completes");
        let expected_deadlines = if id == "zero-duration-delay" { 0 } else { 3 };
        assert_eq!(timer.deadlines.len(), expected_deadlines, "{id}");
        assert_eq!(
            report
                .kernel
                .expect("delay kernel report")
                .post_play_start_allocations,
            0
        );
    }
}

#[test]
fn authored_deadline_requests_typed_operation_cancellation_without_scheduler_cancellation() {
    let mut host = host("authored-deadline-cancellation");
    let planned = fragment(&host, DEADLINE_CANCELLATION_FORM);
    let deadline = planned
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::TIME_DEADLINE_KIND
        })
        .expect("canonical deadline placement exists");
    let operation = planned
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == "audio/tone")
        .expect("cancellable operation placement exists");
    assert_eq!(deadline.host_calls.len(), 1);
    assert_eq!(
        deadline.host_calls[0].contract_id,
        conduit_core::MONOTONIC_TIMER_HOST_CALL_CONTRACT.into()
    );
    let cancellation = planned
        .connections
        .iter()
        .find(|cord| cord.value_kind.as_str() == conduit_core::CANCELLATION_REQUEST_INFO_ID)
        .expect("deadline request is one exact planned Cord");
    assert_eq!(cancellation.source_placement_id, deadline.placement_id);
    assert_eq!(cancellation.sink_placement_id, operation.placement_id);

    let mut output = Vec::with_capacity(4_096);
    let mut timer = ScheduledTimer {
        now_ms: 0,
        deadlines: Vec::with_capacity(4),
        regress_after_wait: false,
        late_by_ms: 3,
    };
    let report = host
        .run_fragment_to(planned, &mut output, &mut timer)
        .expect("authored deadline cancellation completes through the production kernel");
    assert_eq!(timer.deadlines, vec![1]);
    let kernel = report.kernel.expect("deadline kernel report exists");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::SemanticAbnormal));
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| { event.kind == conduit_kernel::KernelEventKind::SemanticAbnormalRecovered }));
    assert!(!kernel.kernel_sign.iter().any(|event| matches!(
        event.kind,
        conduit_kernel::KernelEventKind::CancellationRequested
            | conduit_kernel::KernelEventKind::RunCancelled
    )));
}
