//! Actual selected Tick Body execution through the canonical IMAGE fixture.
use super::*;
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot, Wake};
use std::{cell::Cell, rc::Rc, sync::atomic::Ordering};

mod admission;
mod custody;
mod execution;
mod measurement_refusal;

fn fixture(count: u64, heap: u64) -> (StdHost, Wake, BodyPlan, Clock) {
    // Reuse this constructor suite's canonical checked configuration and Make
    // pipeline. Only the requested memory limit varies; no hand-built IMAGE.
    let source = SOURCE.replace(
        "heap_arena_bytes: 67108864",
        &format!("heap_arena_bytes: {heap}"),
    );
    let selected = image(&source);
    let current = host(&selected, "boot/funded").unwrap();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profile).unwrap();
    let source = format!("plot ticks (\n ticks: Tick...| >>\n) {{\nclock: time/tick(count = {count}, period-ms = 0)\nclock.tick >> ticks\n}}\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authoring =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "ticks", &profile).unwrap();
    let hosts = [current.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundary_limits = authoring
        .front
        .outputs()
        .iter()
        .map(|port| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Output,
                    front_port_id: port.port_id.clone(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 4,
                    byte_capacity: 8,
                },
            )
        })
        .collect();
    let bases = std::collections::BTreeMap::new();
    let lines = std::collections::BTreeMap::new();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &bases,
            line_candidates: &lines,
            connection_item_capacity: 4,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .unwrap();
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/funded-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/funded-woke".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident,
            plan,
        }],
    )
    .unwrap();
    let clock = Clock::new(&current);
    (current, wake, plan, clock)
}
struct Clock {
    identity: MonotonicClockIdentity,
    lost: Rc<Cell<bool>>,
    sampled: usize,
}
impl Clock {
    fn new(host: &StdHost) -> Self {
        Self {
            identity: MonotonicClockIdentity::new(
                host.advertisement.host_id.clone(),
                host.advertisement.boot_id.clone(),
                "fixture/funded-clock".into(),
                TemporalScale::Microseconds,
                1,
                1,
            )
            .unwrap(),
            lost: Rc::new(Cell::new(false)),
            sampled: 0,
        }
    }
}
impl crate::TimerAdapter for Clock {
    fn wait(&mut self, _: std::time::Duration) {}
    fn monotonic_identity_backing_bound(&self, _: &HostId, _: &BootId) -> Option<usize> {
        self.identity.retained_backing_bytes()
    }
    fn monotonic_observation(&mut self, _: &HostId, _: &BootId) -> Option<MonotonicInstant> {
        self.sampled += 1;
        Some(MonotonicInstant::new(0, self.identity.clone()).unwrap())
    }
    fn monotonic_ticks_on(&mut self, basis: &MonotonicClockIdentity) -> Option<u64> {
        (!self.lost.get() && basis == &self.identity).then_some(0)
    }
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        (!self.lost.get()).then_some(0)
    }
    fn wait_until_monotonic_ms(&mut self, _: u64) -> bool {
        !self.lost.get()
    }
}
struct Output {
    retained: Vec<crate::ExternalForeDelivery>,
}
impl Output {
    fn new(capacity: usize) -> Self {
        Self {
            retained: Vec::with_capacity(capacity),
        }
    }
}
impl crate::body_execution::BodyForeOutputAdapter for Output {
    fn deliver(&mut self, value: &crate::ExternalForeDelivery) -> Result<(), String> {
        // A closing Flow has a separate typed terminal Fore. Acknowledge it
        // without pretending its Empty value is a Tick payload.
        if value.track == ConnectionTrack::Payload {
            assert!(self.retained.len() < self.retained.capacity());
            self.retained.push(value.clone());
        }
        Ok(())
    }
}
fn request<'a>(
    wake: &'a Wake,
    plan: &'a BodyPlan,
    control: &'a crate::RunControl,
) -> crate::body_execution::BodyRunRequest<'a> {
    crate::body_execution::BodyRunRequest {
        wake,
        plan,
        control,
        keyboard: None,
    }
}
fn footprint(host: &StdHost, wake: &Wake, plan: &BodyPlan, clock: &Clock) -> usize {
    crate::installed_std::body_kernel::funded::TickPreparationLayout::prepare(
        &request(wake, plan, &crate::RunControl::default()),
        &host.advertisement,
        &host.kernel_resources,
        clock,
        true,
    )
    .unwrap()
    .peak_bytes
}
