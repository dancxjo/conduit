//! Real shared scheduler and finite Fore custody on CPU, without physical IO.
use crate::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    ExternalForeDelivery, RunControl, StdHost, StdHostConfig, TimerAdapter,
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, BodyWorkset, ResidentPlot};
use conduit_core::{
    kind_id, port_id, ConnectionTrack, HostAdvertisement, Plan, PortDirection, Scalar, SignId,
    TerminalDisposition, SCALAR_INFO_ID,
};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{ProfileCatalog, StartupCatalog};
use std::{collections::BTreeMap, time::Duration};

fn plan(name: &str, port: &str, operation: &str, host: &HostAdvertisement) -> Plan {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profiles)
        .unwrap();
    startup
        .insert_value_kind_alias("Scalar", kind_id(SCALAR_INFO_ID))
        .unwrap();
    // The existing I64 startup contract uses raw Scalar microunits.
    let source = format!(
        "plot {name} (\n {port}: Scalar >>\n) {{\n source: scalar/literal(value = 2000000)\n transform: {operation}\n source.value >> transform.in\n transform.out >> {port}\n}}"
    );
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
    let hosts = [host.clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundary = BTreeMap::from([(
        ForeBoundaryKey {
            direction: PortDirection::Output,
            front_port_id: port_id(port),
            track: ConnectionTrack::Payload,
        },
        ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 8,
        },
    )]);
    conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary,
    )
    .unwrap()
}

struct Output {
    expected: [(&'static str, Scalar); 3],
    seen: [bool; 3],
}
impl BodyForeOutputAdapter for Output {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        let index = self
            .expected
            .iter()
            .position(|(port, _)| *port == output.front_port_id.as_str())
            .ok_or("unselected output")?;
        if self.seen[index]
            || output.track != ConnectionTrack::Payload
            || output.value_kind.as_str() != SCALAR_INFO_ID
            || Scalar::decode(&output.bytes).map_err(|error| format!("{error:?}"))?
                != self.expected[index].1
        {
            return Err("duplicate or incorrect scalar output".into());
        }
        self.seen[index] = true;
        Ok(())
    }
}
struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, _: Duration) {}
}

#[test]
fn scalar_host_calls_preserve_prepared_transforms_across_body_partitions() {
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/scalar-body".into(),
        boot_id: "boot/scalar-body".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let cases = [
        (
            "clamped",
            "math/clamp(minimum = 0, maximum = 1000000)",
            Scalar::ONE,
        ),
        (
            "scaled",
            "math/scale(gain = 3000000)",
            Scalar::from_raw_microunits(6 * Scalar::SCALE),
        ),
        ("quieted", "math/deadband(radius = 3000000)", Scalar::ZERO),
    ];
    let partitions = cases
        .iter()
        .map(|(port, operation, _)| {
            let plan = plan(port, port, operation, host.advertisement());
            BodyPlotPlan {
                plot: ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan,
            }
        })
        .collect::<Vec<_>>();
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(partitions.iter().map(|part| part.plot.clone())).unwrap(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/woke")).unwrap().1;
    let plan = BodyPlan::seal(&wake, partitions).unwrap();
    let mut output = Output {
        expected: cases.map(|(port, _, value)| (port, value)),
        seen: [false; 3],
    };
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[],
            false,
            &mut output,
            &mut Vec::with_capacity(256),
            &mut Timer,
        )
        .unwrap();
    assert_eq!(
        report.terminal,
        TerminalDisposition::Completed,
        "{:?}",
        report.failure
    );
    assert_eq!(report.play.body_id, body.body_id);
    assert_eq!(report.play.plan_id, plan.plan_id);
    assert_eq!(report.play.wake_id, wake.wake_id);
    assert_eq!(output.seen, [true; 3]);
    assert_eq!(report.requests.len(), 3);
    assert!(report.requests.iter().enumerate().all(|(index, request)| {
        !report.requests[..index]
            .iter()
            .any(|prior| prior.node == request.node)
    }));
}
