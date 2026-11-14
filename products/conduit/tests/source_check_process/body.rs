//! Actual compiler output through one ordinary body-wide Plan/Play.
use super::host_call::{bounds, support};
use super::*;
use conduit_body::{Body, BodyPlan, BodyPlotPlan, BodyWorkset, ResidentPlot};
use conduit_core::*;
use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
use conduit_std_host::{source_check::*, *};
struct Capture(Vec<Vec<u8>>);
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value.bytes.to_vec());
        Ok(())
    }
}
#[test]
fn actual_compiler_uses_one_body_play_and_ordinary_host_call_signs() {
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let mut outputs = Vec::new();
    let mut requests = Vec::new();
    let mut expected = Vec::new();
    let mut worker = parent::worker(std::time::Duration::from_secs(20), 16 * 1024);
    for (source, generation) in [
        (include_str!("../../../../plots/hello/main.conduit"), 7),
        ("plot hello { bad: unavailable/kind }", 8),
    ] {
        let (input, mut reference) = input_with_generation(source, generation);
        expected.push(
            schema
                .encode_reply(
                    SourceCheckReply::Checked(&reference.project(&input.request).unwrap()),
                    bounds().report_bytes as usize,
                )
                .unwrap(),
        );
        requests.push(
            schema
                .encode_request(&input.request, bounds().request_bytes as usize)
                .unwrap(),
        );
        outputs.push(worker.prepare(&input, &RunControl::default()).unwrap());
    }
    let prepared = PreparedSourceChecker::from_process_outputs(
        &schema,
        &outputs,
        bounds(),
        &RunControl::default(),
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    prepared
        .install_authoring(&mut startup, &mut profiles)
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../make/body/tests/source_check_call/check.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "check-source", &profiles).unwrap();
    let mut host = StdHost::new();
    let definition = support::definition(host.advertisement(), &prepared);
    let grant = definition.grant.clone();
    // SAFETY: Fixture independently owns exact immutable Source, canonical
    // compiler outputs and current Host/Boot grant; child processes are reaped.
    unsafe { host.install_immediate_native_source(definition, Box::new(prepared.into_provider())) }
        .unwrap_or_else(|(_, _, error)| panic!("{error}"));
    let plan = support::plan(&host, &authored, &[grant], bounds()).unwrap();
    let plot = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let source_plan_id = plan.plan_id.clone();
    let source_placement = plan.fragments[0].placements[0].placement_id.clone();
    conduit_time::install_tick_catalog(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profiles).unwrap();
    let (timer_plot,timer_plan) = (0..128).find_map(|salt| {
        let source = format!("# route test {salt}\nplot ticks (\n ticks: Tick...| >>\n) {{\n clock: time/tick(count = 1, period-ms = 0)\n clock.tick >> ticks\n}}\n");
        let checked = check_syntax_document(&parse_syntax_document(&source),&startup).unwrap();
        let expanded = expand_canonical_plot_for_authoring(&checked,"ticks",&profiles).unwrap();
        let hosts = [host.advertisement().clone()];
        let placements = conduit_planner::default_expanded_placements(&expanded.expanded,&hosts).unwrap();
        let boundaries = std::collections::BTreeMap::from([(
            conduit_planner::ForeBoundaryKey { direction: PortDirection::Output,
                front_port_id: "ticks".into(), track: ConnectionTrack::Payload },
            conduit_planner::ConnectionQueueLimits { item_capacity: 1, byte_capacity: 64 },
        )]);
        let plan = conduit_planner::plan_expanded_authoring_with_options(&expanded,&hosts,&placements,
            &["conduit.base/local@1".into()], conduit_planner::PlanningOptions {
                connection_bases: &std::collections::BTreeMap::new(), line_candidates: &std::collections::BTreeMap::new(),
                connection_item_capacity: 1, connection_byte_capacity: 64,
                authority_grants: &[], protected_resource_grants: &[], line_offers: &[],
            },&boundaries).unwrap();
        let resident = ResidentPlot::new(plan.source_document_id.clone(),plan.checked_plot_id.clone());
        (resident < plot).then_some((resident,plan))
    }).expect("bounded fixture selects a timer before the Source in canonical Body order");
    let body = Body::born_with_plots(
        BodyWorkset::from_plots([plot.clone(), timer_plot.clone()]).unwrap(),
        1,
        "fixture/body-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "fixture/body-woke".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        vec![
            BodyPlotPlan { plot, plan },
            BodyPlotPlan {
                plot: timer_plot,
                plan: timer_plan,
            },
        ],
    )
    .unwrap();
    let saved_request = requests[0].clone();
    let mut prior = None;
    for (request, expected) in requests.into_iter().zip(expected) {
        let mut capture = Capture(vec![]);
        let report = host
            .run_body_plan_with_fore_to(
                BodyRunRequest {
                    wake: &wake,
                    plan: &plan,
                    control: &RunControl::default(),
                    keyboard: None,
                },
                &[ExternalForeInput {
                    front_port_id: "request".into(),
                    track: ConnectionTrack::Payload,
                    bytes: request,
                }],
                false,
                &mut capture,
                &mut Vec::new(),
                &mut ThreadTimer,
            )
            .unwrap();
        assert_eq!(
            report.terminal,
            TerminalDisposition::Completed,
            "{:?}",
            report.failure
        );
        assert_eq!(capture.0.len(), 2);
        assert!(capture.0.contains(&expected));
        let source_map = report
            .partitions
            .iter()
            .find(|map| map.plan_id == source_plan_id)
            .unwrap();
        assert_eq!(
            source_map.node_for_placement(&source_placement),
            Some(conduit_kernel::NodeId(1))
        );
        assert!(report.play.validate_for(&plan));
        assert_eq!(report.wake_at_start.plans.len(), 1);
        assert_eq!(
            report.wake_at_start.plans[0].active_play_id.as_ref(),
            Some(&report.play.active_play_id)
        );
        if let Some(previous) = prior {
            assert_ne!(report.play.active_play_id, previous);
        }
        prior = Some(report.play.active_play_id.clone());
        for kind in [
            conduit_kernel::KernelEventKind::HostCallRequested,
            conduit_kernel::KernelEventKind::HostCallCompleted,
        ] {
            assert!(report.kernel_events.iter().any(|event| event.kind == kind));
        }
    }
    let stop = RunControl::default();
    stop.request_stop(RunControlRequestId::new("stop-before-native-body").unwrap())
        .unwrap();
    let mut capture = Capture(vec![]);
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &stop,
                keyboard: None,
            },
            &[ExternalForeInput {
                front_port_id: "request".into(),
                track: ConnectionTrack::Payload,
                bytes: saved_request.clone(),
            }],
            false,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert!(capture.0.is_empty());
    // SAFETY: Every actual child Play is terminal; the fixture provider has no
    // background work and finite Vec/Box retirement.
    unsafe { host.retire_immediate_native_source() }.unwrap();
    let started = std::cell::Cell::new(false);
    let refusal = host.run_body_plan_with_fore_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &[ExternalForeInput {
            front_port_id: "request".into(),
            track: ConnectionTrack::Payload,
            bytes: saved_request,
        }],
        false,
        &mut capture,
        &mut Vec::new(),
        &mut ThreadTimer,
        |_, _| {
            started.set(true);
            Ok(())
        },
    );
    assert!(refusal.is_err());
    assert!(!started.get());
    assert!(capture.0.is_empty());
}
