//! Real authored bounded history inspection; CPU canonical fixture, no sensor/media claim.
use crate::{
    body_execution::{
        BodyFaceContributionObserver, BodyForeOutputAdapter, BodyRunRequest,
        CommittedFaceContribution, FaceContributionRefusal, ReturnedFaceContribution,
    },
    ExternalForeDelivery, ExternalForeInput, RunControl, StdHost, StdHostConfig, ThreadTimer,
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot, Wake};
use conduit_core::{
    ConnectionTrack, PortDirection, Quantity, StructuredInfoValue, TemporalInstant, TemporalScale,
    Unit,
};
use conduit_data::*;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use std::{cell::Cell, collections::BTreeMap, sync::atomic::Ordering};
fn planned(host: &conduit_core::HostAdvertisement) -> Result<(BodyPlan, Wake), String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profiles)
        .unwrap();
    install_measurement_window_catalog(&mut startup, &mut profiles).unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profiles).unwrap();
    install_measurement_chart_constructor(1, &mut startup, &mut profiles).unwrap();
    install_measurement_chart_flow_catalog(&mut startup, &mut profiles).unwrap();
    let (schema, bound) = measurement_chart_constructor_input(1).unwrap();
    let value = conduit_core::CheckedValueContract::new(
        schema.profile().unwrap().value_kind().clone(),
        bound,
        vec![],
    )
    .unwrap();
    conduit_semantic_catalog::install_flow_singleton_kind(
        &value,
        &schema,
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let b = conduit_std_offers::measurement_window_value_bounds();
    let source =
        include_str!("../../../../../semantics/data/tests/fixtures/measurement-history.conduit");
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .map_err(|e| format!("{e:?}"))?;
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "inspection/history",
        &profiles,
    )
    .map_err(|e| e.to_string())?;
    let hosts = [host.clone()];
    let selected = conduit_planner::default_expanded_placements(&expanded.expanded, &hosts)
        .map_err(|e| e.to_string())?;
    let mut boundary = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, &expanded.input_bindings),
        (PortDirection::Output, &expanded.output_bindings),
    ] {
        for binding in bindings {
            let bytes = if direction == PortDirection::Input {
                if binding.front_port_id.as_str() == "profile" {
                    b[0]
                } else {
                    b[1]
                }
            } else if binding.track == ConnectionTrack::AbnormalTerminal {
                conduit_core::TERMINAL_INFO_ENCODED_LEN as u32
            } else {
                1
            };
            boundary.insert(
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    let connections =
        conduit_planner::default_expanded_connection_limits(&expanded.expanded, &hosts, &selected)
            .map_err(|e| e.to_string())?;
    let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
        &expanded,
        &hosts,
        &selected,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: b[2],
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connections,
        &boundary,
    )
    .map_err(|e| e.to_string())?;
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "proof/history/born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "proof/history/wake".into()).unwrap().1;
    Ok((
        BodyPlan::seal(
            &wake,
            vec![BodyPlotPlan {
                plot: resident,
                plan,
            }],
        )
        .unwrap(),
        wake,
    ))
}
fn inputs() -> Vec<ExternalForeInput> {
    let profile = MeasurementWindowProfile {
        capacity: 4,
        range: MeasurementRange {
            minimum: Quantity::new(0, Unit::Millivolt),
            maximum: Quantity::new(100, Unit::Millivolt),
        },
        clock_basis: "fixture/history-clock".into(),
        full_policy: FullWindowPolicy::Reject,
    };
    let wrap = |front: &str, ty, bytes| ExternalForeInput {
        front_port_id: front.into(),
        track: ConnectionTrack::Payload,
        bytes: StructuredInfoValue::leaf(ty, bytes)
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    };
    let mut out = vec![wrap(
        "profile",
        measurement_window_profile_type(),
        encode_measurement_window_profile(&profile).unwrap(),
    )];
    for index in 1..=3 {
        let sample = MeasurementSample {
            value: Quantity::new(index * 25, Unit::Millivolt),
            observed_at: TemporalInstant {
                ticks: index as u64,
                scale: TemporalScale::Milliseconds,
                clock_basis: profile.clock_basis.clone(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            }
            .try_into()
            .unwrap(),
            uncertainty: None,
        };
        out.push(wrap(
            "measurement",
            measurement_sample_type(),
            encode_measurement_sample(&sample).unwrap(),
        ));
    }
    out
}
struct Outputs;
impl BodyForeOutputAdapter for Outputs {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        Err("history inspection produces no payload Fore output".into())
    }
}
struct Capture {
    frames: Vec<CommittedFaceContribution>,
    pressure: bool,
}
impl BodyFaceContributionObserver for Capture {
    fn contribute(
        &mut self,
        value: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        if self.pressure {
            self.pressure = false;
            return Err(ReturnedFaceContribution {
                contribution: value,
                refusal: FaceContributionRefusal::Pressure,
            });
        }
        if self.frames.len() == self.frames.capacity() {
            return Err(ReturnedFaceContribution {
                contribution: value,
                refusal: FaceContributionRefusal::Pressure,
            });
        }
        self.frames.push(value);
        Ok(())
    }
}
fn image_host(heap: u64, pool: u32) -> Result<StdHost, String> {
    let text = include_str!("../../../profiles/raspberry-pi-5-measurement-cpu.host.conduit").replace(
        r#"capability = {kind: "face/measurement-chart", pinned_implementation: "std/measurement-chart-face@1"}"#,
        r#"capability = {kind: "data/measurement-count-window", pinned_implementation: "std/measurement-count-window@1"}
  capability = {kind: "flow/singleton", pinned_implementation: "std/flow-singleton@1"}
  capability = {kind: "data/measurement-chart/1", pinned_implementation: "std/measurement-chart-constructor@1/1"}
  capability = {kind: "face/measurement-chart-flow", pinned_implementation: "std/measurement-chart-face-flow@1"}"#,
    );
    let mut config =
        conduit_host_make::parse_host_configuration_conduit(&text).map_err(|e| format!("{e:?}"))?;
    config.target.architecture = std::env::consts::ARCH.into();
    config.target.machine = "computer".into();
    config.target.board = None;
    config.limits.heap_arena_bytes = heap;
    let cpu = config
        .bases
        .iter_mut()
        .find(|b| b.kind == "compute/cpu")
        .unwrap();
    cpu.initialization
        .as_mut()
        .unwrap()
        .values
        .insert("memory_bytes".into(), serde_json::json!(pool));
    let memory = config
        .resources
        .iter_mut()
        .find(|r| r.id == "memory/measurement")
        .unwrap();
    memory.slots = pool;
    memory.bytes = pool as u64;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = conduit_host_make::check_host_configuration(config, &catalog, &packages)
        .map_err(|e| format!("{e:?}"))?;
    let image = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &conduit_host_make::BuildInputs {
            source_identity: "proof/history-cpu".into(),
            toolchain_available: true,
        },
    )
    .map_err(|e| format!("{e:?}"))?
    .0;
    StdHost::from_selected_timer_and_cpu_image(
        StdHostConfig {
            host_id: "proof/history-host".into(),
            boot_id: "proof/history-boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &image,
        &catalog,
        &checked,
    )
}
#[test]
fn authored_history_window_projection_chart_commits_under_original_image_custody() {
    let selected = [
        "data/measurement-count-window",
        "data/measurement-plot",
        "data/measurement-chart/1",
        "face/measurement-chart-flow",
        "flow/singleton",
    ];
    let requirements: Vec<_> = conduit_std_offers::measurement_chart_cpu_offers()
        .unwrap()
        .into_iter()
        .filter(|offer| selected.contains(&offer.kind_id.as_str()))
        .map(|offer| {
            (
                offer.kind_id,
                offer
                    .resource_requirements
                    .iter()
                    .map(|r| u64::from(r.units))
                    .sum::<u64>(),
            )
        })
        .collect();
    let required = requirements.iter().map(|(_, n)| n).sum::<u64>();
    println!("Actual selected pipeline requirements={requirements:?}; total={required}; unchanged authored pool={}",2*1024*1024);
    assert!(
        required <= 2 * 1024 * 1024,
        "refuse rather than enlarge the authored CPU pool"
    );
    let mut host = image_host(64 * 1024 * 1024, 2 * 1024 * 1024).unwrap();
    let (plan, wake) = planned(host.advertisement()).unwrap();
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    let mut capture = Capture {
        frames: Vec::with_capacity(1),
        pressure: true,
    };
    let highwater = Cell::new(0);
    let report = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &inputs(),
            true,
            &mut Outputs,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                highwater.set(observed.load(Ordering::Acquire));
                Ok(())
            },
        )
        .unwrap();
    assert!(
        report.report().failure.is_none(),
        "{:?}",
        report.report().failure
    );
    assert_eq!(
        report.report().terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert_eq!(capture.frames.len(), 1);
    let frame = &capture.frames[0];
    assert_eq!(frame.scope().body_play(), &report.report().play);
    assert_eq!(frame.fragment().properties[0].name, "History");
    assert!(highwater.get() > baseline);
    let retained = observed.load(Ordering::Acquire);
    println!("Original IMAGE counter: initial={baseline}, admittedStart={}, retainedWithReportAndFace={retained}; committedFaceTokens={}",highwater.get(),capture.frames.len());
    drop(report);
    assert_eq!(observed.load(Ordering::Acquire), retained);
    drop(capture);
    assert_eq!(observed.load(Ordering::Acquire), baseline);
    drop(host);
    assert_eq!(observed.load(Ordering::Acquire), 0);
}
#[test]
fn authored_cpu_pool_shortage_refuses_before_any_history_play() {
    assert!(image_host(
        64 * 1024 * 1024,
        conduit_std_offers::measurement_window_workspace_bytes() - 1
    )
    .is_err());
}
#[test]
fn authored_history_cancellation_during_observer_pressure_retires_original_custody() {
    struct Cancel<'a> {
        control: &'a RunControl,
        calls: usize,
    }
    impl BodyFaceContributionObserver for Cancel<'_> {
        fn contribute(
            &mut self,
            contribution: CommittedFaceContribution,
        ) -> Result<(), ReturnedFaceContribution> {
            self.calls += 1;
            self.control
                .request_stop(
                    crate::RunControlRequestId::new("test/history-publication-cancel").unwrap(),
                )
                .unwrap();
            Err(ReturnedFaceContribution {
                contribution,
                refusal: FaceContributionRefusal::Pressure,
            })
        }
    }
    let mut host = image_host(64 * 1024 * 1024, 2 * 1024 * 1024).unwrap();
    let (plan, wake) = planned(host.advertisement()).unwrap();
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    let control = RunControl::default();
    let mut observer = Cancel {
        control: &control,
        calls: 0,
    };
    let report = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &inputs(),
            true,
            &mut Outputs,
            &mut observer,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(observer.calls, 1);
    assert!(matches!(
        report.report().terminal,
        conduit_core::TerminalDisposition::Cancelled { .. }
    ));
    drop(report);
    assert_eq!(observed.load(Ordering::Acquire), baseline);
}
