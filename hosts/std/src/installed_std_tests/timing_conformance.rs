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

const DEBOUNCE_FORM: &str = "form robot-debounce {\n    switch: test/timing-bool-source\n    stable: time/debounce(duration-ms = 5ms, policy = \"trailing\", maximum-values = 3)\n    sink: test/timing-bool-sink\n    switch > stable > sink\n}\n";

const TIMEOUT_FORM: &str = "form robot-timeout {\n    clock: time/tick(count = 2, period-ms = 10)\n    stale: time/timeout(duration-ms = 7ms, maximum-values = 2)\n    sink: test/timing-bool-sink\n    clock > stale > sink\n}\n";

fn fragment(host: &StdHost, source: &str) -> conduit_core::PlanFragment {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut startup_profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_tick_pipeline_catalogs(&mut startup, &mut startup_profile)
        .expect("tick startup signature installs");
    conduit_std_catalog::install_timing_catalogs(&mut startup, &mut startup_profile)
        .expect("timing startup signatures install");
    startup
        .insert(conduit_form::KindSignature {
            kind: "test/timing-bool-source".to_string(),
            startup_parameters: Vec::new(),
        })
        .expect("test source startup signature is unique");
    startup
        .insert(conduit_form::KindSignature {
            kind: "test/timing-bool-sink".to_string(),
            startup_parameters: Vec::new(),
        })
        .expect("test timing sink startup signature is unique");
    let syntax = conduit_form::parse_syntax_document(source);
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .expect("canonical timing Form checks");
    let profile = installed_std::test_catalog();
    let expanded = conduit_form::expand_canonical_form(&checked, &checked.forms[0].name, &profile)
        .expect("canonical timing Form expands");
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .expect("timing placements resolve");
    conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
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
    .expect("timing Form plans through the ordinary planner")
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
        .expect("timing Form executes through the production kernel");
    (report, timer)
}

#[test]
fn representative_robot_debounce_and_timeout_run_through_one_production_kernel() {
    for (source, id, kind) in [
        (
            DEBOUNCE_FORM,
            "robot-debounce",
            conduit_std_catalog::TIME_DEBOUNCE_KIND,
        ),
        (
            TIMEOUT_FORM,
            "robot-timeout",
            conduit_std_catalog::TIME_TIMEOUT_KIND,
        ),
    ] {
        let mut planned_host = host(id);
        let planned = fragment(&planned_host, source);
        let timing = planned
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .expect("canonical timing placement exists");
        assert_eq!(timing.host_operations.len(), 1);
        assert_eq!(
            timing.host_operations[0].contract_id,
            conduit_core::MONOTONIC_TIMER_HOST_OPERATION_CONTRACT.into()
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
            .unwrap_or_else(|error| panic!("representative {kind} Form completes: {error}"));
        let kernel = report.kernel.expect("production kernel report exists");
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(!timer.deadlines.is_empty());
    }
}

#[test]
fn identical_simulated_schedules_have_identical_normalized_output_and_signs() {
    for (source, id) in [
        (DEBOUNCE_FORM, "repeat-debounce"),
        (TIMEOUT_FORM, "repeat-timeout"),
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
    let maximum = conduit_std_catalog::TIME_MAXIMUM_DURATION_MS;
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
