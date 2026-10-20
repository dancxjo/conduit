use super::*;
use crate::thermostat_run::prepare;
use conduit_thermostat_plot::{Command, Mode};

#[path = "../../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

struct Fixture {
    source: CanonicalSource,
    advertisement: HostAdvertisement,
    source_plan: Plan,
    body_plan: BodyPlan,
    report: BodyRunReport,
}
impl Fixture {
    fn new() -> Self {
        let source =
            crate::plot_source::parse(include_str!("../../../../../plots/thermostat/main.conduit"))
                .unwrap();
        let plot = source.expand_entry_for_authoring().unwrap();
        let (host, source_plan) = prepare(&source, &plot).unwrap();
        let advertisement = host.advertisement().clone();
        let mut execution =
            conduit_thermostat_runtime::execution::Execution::from_plan(host, source_plan.clone())
                .unwrap();
        let body_plan = execution.body_plan().clone();
        execution.execute(Command::SetTarget(225)).unwrap();
        execution.execute(Command::SetMode(Mode::Heat)).unwrap();
        execution.execute(Command::Observe(Some(170))).unwrap();
        let report = execution.finish().unwrap();
        Self {
            source,
            advertisement,
            source_plan,
            body_plan,
            report,
        }
    }

    fn render(&self, requested: bool) -> Result<Option<NativeEvidence>, String> {
        super::render(
            requested,
            &self.source,
            &self.advertisement,
            &self.source_plan,
            &self.body_plan,
            &self.report,
        )
    }
}

#[test]
fn ordinary_reportless_execution_avoids_descriptive_reconstruction() {
    let fixture = Fixture::new();
    let events = fixture.report.kernel_events.clone();
    let (skipped, skipped_allocations) = allocation_probe::observe(|| fixture.render(false));
    assert!(skipped.unwrap().is_none());
    assert_eq!(skipped_allocations.allocations, 0);
    assert_eq!(skipped_allocations.reallocations, 0);
    assert_eq!(skipped_allocations.peak_bytes, 0);

    let (rendered, requested_allocations) = allocation_probe::observe(|| fixture.render(true));
    let rendered = rendered.unwrap().unwrap();
    assert!(requested_allocations.allocations > 0);
    assert_eq!(fixture.report.kernel_events, events);
    assert_eq!(
        rendered.report["play"],
        serde_json::to_value(&fixture.report.play).unwrap()
    );
    assert_eq!(
        rendered.report["terminal_sign"],
        serde_json::to_value(&fixture.report.terminal_sign).unwrap()
    );
    let rows = rendered.report["kernel_events"].as_array().unwrap();
    assert_eq!(rows.len(), events.len());
    for (row, event) in rows.iter().zip(&events) {
        assert_eq!(row["sequence"], event.sequence);
        assert_eq!(row["node"], event.node.0);
        assert_eq!(
            row["port"],
            serde_json::to_value(event.port.map(|p| p.0)).unwrap()
        );
        assert_eq!(
            row["request"],
            serde_json::to_value(event.request.map(|r| r.0)).unwrap()
        );
        assert_eq!(row["kind"], format!("{:?}", event.kind));
    }
    let children = fixture.report.scan_child_signs.as_ref().unwrap();
    assert_eq!(rendered.child_signs.len(), children.len());
    for (row, child) in rendered.child_signs.iter().zip(children) {
        assert_eq!(
            row["parent_active_play_id"],
            child.parent_active_play_id.as_str()
        );
        assert_eq!(
            row["child_active_play_id"],
            child.child_active_play_id.as_str()
        );
        assert_eq!(row["events"].as_array().unwrap().len(), child.events.len());
    }
    println!(
        "SIGN_RECONSTRUCTION={}",
        serde_json::json!({
            "scope":"post-Play JSON reconstruction only; no recording/Play or whole-process allocation claim",
            "unrequested_allocations":skipped_allocations.allocations,
            "requested_allocations":requested_allocations.allocations,
            "requested_reallocations":requested_allocations.reallocations,
            "requested_peak_new_bytes":requested_allocations.peak_bytes,
            "numeric_event_bytes":std::mem::size_of::<conduit_kernel::KernelEvent>(),
            "retained_parent_event_storage_bytes":fixture.report.kernel_events.capacity() * std::mem::size_of::<conduit_kernel::KernelEvent>(),
            "retained_child_event_storage_bytes":children.iter().map(|c| c.events.capacity() * std::mem::size_of::<conduit_kernel::KernelEvent>()).sum::<usize>(),
            "event_storage_scope":"numeric Vec capacity only, excluding metadata and RSS",
            "requested_report_json_bytes":serde_json::to_vec(&rendered.report).unwrap().len(),
        })
    );
}

#[test]
fn mandatory_child_sign_refusal_is_not_hidden_by_an_unrequested_report() {
    let mut fixture = Fixture::new();
    fixture.report.scan_child_signs = Err(conduit_composite::BoundedScanError::MissingOutput);
    for requested in [false, true] {
        assert_eq!(
            fixture.render(requested).err().unwrap(),
            "child Signs: MissingOutput"
        );
    }
}
