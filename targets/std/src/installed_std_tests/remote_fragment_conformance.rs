use super::{host, installed_std, TimerAdapter};
use conduit_body::{Body, BodyHostClockEvidence, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, BodyClockCorrelation,
    BodyTimeRequirement, BodyTimeTolerance, ClockProvenance, GearId, LinkLimits,
    MonotonicClockIdentity, MonotonicDuration, MonotonicInstant, SignId, TemporalScale,
};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
use conduit_plot::parse;
use std::collections::BTreeMap;
use std::time::Duration;

struct FixedClock(MonotonicInstant);

impl TimerAdapter for FixedClock {
    fn wait(&mut self, _: Duration) {}

    fn monotonic_observation(
        &mut self,
        _: &conduit_core::HostId,
        _: &conduit_core::BootId,
    ) -> Option<MonotonicInstant> {
        Some(self.0.clone())
    }
}

#[test]
fn generic_remote_fragment_routes_through_latest_and_atomic_tee() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(run_body_time_qualified_remote_fragment_flow)
        .unwrap()
        .join()
        .unwrap();
}

fn run_body_time_qualified_remote_fragment_flow() {
    let plot = parse(
        "plot remote_typed_flow {\n source: conduit-test/scalar-source\n latest: state/latest\n split: flow/tee\n left: conduit-test/scalar-sink\n right: conduit-test/scalar-sink\n source.value >> latest.in\n latest.out >> split.in\n split.left >> left.in\n split.right >> right.in\n}\n",
        &installed_std::test_catalog(),
    )
    .unwrap();
    let mut source_host = host("remote-flow-source");
    let source = source_host.advertisement().clone();
    let mut middle_host = Box::new(host("remote-flow-middle"));
    let middle = middle_host.advertisement().clone();
    let mut sink_host = Box::new(host("remote-flow-sink"));
    let sink = sink_host.advertisement().clone();
    let hosts = [source.clone(), middle.clone(), sink.clone()];
    let capability = |host: &conduit_core::HostAdvertisement, kind: &str| {
        host.capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == kind)
            .unwrap()
            .capability_id
            .clone()
    };
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("remote_typed_flow/source"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: capability(&source, "conduit-test/scalar-source"),
                },
            ),
            (
                GearId::from("remote_typed_flow/latest"),
                PlacementChoice {
                    host_id: middle.host_id.clone(),
                    capability_id: capability(&middle, conduit_semantic_catalog::LATEST_KIND),
                },
            ),
            (
                GearId::from("remote_typed_flow/split"),
                PlacementChoice {
                    host_id: middle.host_id.clone(),
                    capability_id: capability(&middle, conduit_semantic_catalog::TEE_KIND),
                },
            ),
            (
                GearId::from("remote_typed_flow/left"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: capability(&sink, "conduit-test/scalar-sink"),
                },
            ),
            (
                GearId::from("remote_typed_flow/right"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: capability(&sink, "conduit-test/scalar-sink"),
                },
            ),
        ]),
    };
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
        maximum_buffered_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
        maximum_frame_bytes: 8_192,
    };
    let line =
        |id: &str, from: &conduit_core::HostAdvertisement, to: &conduit_core::HostAdvertisement| {
            process_owned_line_offer_with_limits(
                id,
                &format!("{id}-binding"),
                BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
                &format!("{id}-instance"),
                from,
                to,
                limits,
            )
        };
    let mut incoming = line("remote-flow-in", &source, &middle);
    let mut left = line("remote-flow-left", &middle, &sink);
    let mut right = line("remote-flow-right", &middle, &sink);
    for offered in [&mut incoming, &mut left, &mut right] {
        offered.contract.scope = conduit_core::LineScope::LocalNetwork;
        offered.contract.security = conduit_core::LineSecurity::PlaintextNetwork;
    }
    let line_candidates = BTreeMap::from([
        (
            (
                GearId::from("remote_typed_flow/source"),
                GearId::from("remote_typed_flow/latest"),
            ),
            vec![incoming.line_id.clone()],
        ),
        (
            (
                GearId::from("remote_typed_flow/split"),
                GearId::from("remote_typed_flow/left"),
            ),
            vec![left.line_id.clone()],
        ),
        (
            (
                GearId::from("remote_typed_flow/split"),
                GearId::from("remote_typed_flow/right"),
            ),
            vec![right.line_id.clone()],
        ),
    ]);
    let plan = plan_with_options(
        &plot,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[incoming, left, right],
        },
    )
    .unwrap();
    let wake = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("remote-flow/born"),
    )
    .unwrap()
    .wake(1, SignId::from("remote-flow/wake"))
    .unwrap()
    .1;
    let body_plan = BodyPlan::seal_with_body_time(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan: plan.clone(),
        }],
        BodyTimeRequirement::new(
            wake.body_id.as_str().into(),
            BodyTimeTolerance::new(10, TemporalScale::Milliseconds),
            MonotonicDuration::new(100, TemporalScale::Milliseconds),
        )
        .unwrap(),
    )
    .unwrap();
    let samples = [(&source, 1_000), (&middle, 9_000), (&sink, 40_000)].map(|(host, ticks)| {
        MonotonicInstant::new(
            ticks,
            MonotonicClockIdentity::new(
                host.host_id.clone(),
                host.boot_id.clone(),
                "test/steady".into(),
                TemporalScale::Milliseconds,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
    });
    let correlations = samples.clone().map(|sample| {
        BodyClockCorrelation::new(
            wake.body_id.as_str().into(),
            TemporalScale::Milliseconds,
            1,
            sample,
            10_000,
            0,
            100,
            1,
            10_000,
            ClockProvenance::External {
                provider_id: "fixture/provider".into(),
                admission_reference: "fixture/admission".into(),
                policy_id: "fixture/policy".into(),
            },
        )
        .unwrap()
    });
    let evidence = [0, 1, 2].map(|index| BodyHostClockEvidence {
        sample: &samples[index],
        correlation: &correlations[index],
        transport_uncertainty: MonotonicDuration::new(1, TemporalScale::Milliseconds),
        scheduler_uncertainty: MonotonicDuration::new(1, TemporalScale::Milliseconds),
    });
    let admission = body_plan.admit_body_time(&evidence).unwrap();
    assert_eq!(admission.hosts().len(), 3);
    let fragment = |host: &conduit_core::HostAdvertisement| {
        plan.fragments
            .iter()
            .find(|fragment| fragment.host_id == host.host_id)
            .unwrap()
    };
    let mut source_runtime = source_host
        .prepare_remote_fragment_with_body_time(
            &body_plan,
            &admission,
            fragment(&source),
            &mut FixedClock(samples[0].clone()),
        )
        .unwrap();
    assert_eq!(
        source_runtime.identity().active_play_id,
        source_runtime
            .sessions()
            .iter()
            .next()
            .unwrap()
            .binding()
            .source_active_play_id
    );
    assert!(source_host
        .prepare_remote_fragment(fragment(&source))
        .err()
        .unwrap()
        .contains("combined active-instance limit exceeded"));
    let mut middle_runtime = middle_host
        .prepare_remote_fragment_with_body_time(
            &body_plan,
            &admission,
            fragment(&middle),
            &mut FixedClock(samples[1].clone()),
        )
        .unwrap();
    let mut sink_runtime = sink_host
        .prepare_remote_fragment_with_body_time(
            &body_plan,
            &admission,
            fragment(&sink),
            &mut FixedClock(samples[2].clone()),
        )
        .unwrap();
    assert_eq!(fragment(&middle).placements.len(), 2);
    let source_endpoint = source_runtime.sessions().iter().next().unwrap().endpoint;
    let middle_ingress = middle_runtime
        .sessions()
        .iter()
        .find(|session| {
            session.direction == conduit_plan_lowering::lowering::RemoteCordDirection::Ingress
        })
        .unwrap()
        .endpoint;
    crate::remote_cord_sessions::activate_in_process(
        source_runtime
            .sessions_mut()
            .get_mut(source_endpoint)
            .unwrap(),
        middle_runtime
            .sessions_mut()
            .get_mut(middle_ingress)
            .unwrap(),
    )
    .unwrap();
    let transfer = (0..8)
        .find_map(|_| {
            if let Some(transfer) = source_runtime.next_egress(source_endpoint).unwrap() {
                return Some(transfer);
            }
            if let Some(request) = source_runtime.next_host_request() {
                let work = source_runtime.describe_host_request(request).unwrap();
                assert_eq!(work.request, request);
                assert_eq!(
                    work.contract_id,
                    conduit_core::wait_host_call_requirement().contract_id
                );
                assert_eq!(work.input.len(), request.input.value.byte_len as usize);
                assert_eq!(work.maximum_output_bytes, 0);
                source_runtime
                    .complete_host_call(
                        request,
                        conduit_kernel::HostCallOutcome {
                            disposition: conduit_kernel::HostCallDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            let _ = source_runtime
                .step_with_body_time(&mut FixedClock(samples[0].clone()))
                .unwrap();
            None
        })
        .expect("literal reaches remote Cord");
    source_runtime.accept_egress(&transfer).unwrap();
    middle_runtime
        .admit_ingress(middle_ingress, transfer.sequence, &transfer.bytes)
        .unwrap();
    source_runtime.deliver_egress(&transfer).unwrap();
    assert_eq!(
        source_runtime
            .fail_remote_line(source_endpoint, 0)
            .unwrap_err(),
        "record remote Line failure: InvalidState"
    );
    source_runtime
        .fail_remote_line(source_endpoint, 73)
        .unwrap();
    assert!(source_runtime
        .next_egress(source_endpoint)
        .unwrap_err()
        .contains("Cancelled"));
    middle_runtime.close_ingress(middle_ingress).unwrap();
    let egress = middle_runtime
        .sessions()
        .iter()
        .filter(|session| {
            session.direction == conduit_plan_lowering::lowering::RemoteCordDirection::Egress
        })
        .map(|session| session.endpoint)
        .collect::<Vec<_>>();
    assert_eq!(egress.len(), 2);
    let mut sink_pairs = Vec::new();
    for endpoint in &egress {
        let binding = middle_runtime
            .sessions()
            .get(*endpoint)
            .unwrap()
            .binding()
            .clone();
        let sink_endpoint = sink_runtime
            .sessions()
            .iter()
            .find(|session| session.binding() == &binding)
            .unwrap()
            .endpoint;
        crate::remote_cord_sessions::activate_in_process(
            middle_runtime.sessions_mut().get_mut(*endpoint).unwrap(),
            sink_runtime.sessions_mut().get_mut(sink_endpoint).unwrap(),
        )
        .unwrap();
        sink_pairs.push((*endpoint, sink_endpoint));
    }
    let offers = (0..16)
        .find_map(|_| {
            let offers = egress
                .iter()
                .filter_map(|endpoint| middle_runtime.next_egress(*endpoint).unwrap())
                .collect::<Vec<_>>();
            if offers.len() == 2 {
                return Some(offers);
            }
            let _ = middle_runtime
                .step_with_body_time(&mut FixedClock(samples[1].clone()))
                .unwrap();
            None
        })
        .expect("latest and tee atomically commit both remote branches");
    assert!(offers.iter().all(|offer| offer.bytes == transfer.bytes));
    for offer in &offers {
        let sink_endpoint = sink_pairs
            .iter()
            .find(|(source_endpoint, _)| *source_endpoint == offer.endpoint)
            .unwrap()
            .1;
        middle_runtime.accept_egress(offer).unwrap();
        assert_eq!(
            sink_runtime
                .admit_ingress(sink_endpoint, offer.sequence, &offer.bytes)
                .unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted {
                sequence: offer.sequence
            },
        );
        middle_runtime.deliver_egress(offer).unwrap();
        sink_runtime.close_ingress(sink_endpoint).unwrap();
    }
    source_host.release_remote_fragment(source_runtime).unwrap();
    middle_host.release_remote_fragment(middle_runtime).unwrap();
    sink_host.release_remote_fragment(sink_runtime).unwrap();
    let replacement = source_host
        .prepare_remote_fragment(fragment(&source))
        .expect("released remote capability and resource capacity is reusable");
    source_host.release_remote_fragment(replacement).unwrap();
}

#[test]
fn body_time_qualified_tick_crosses_two_host_fragments() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(run_body_time_qualified_tick)
        .unwrap()
        .join()
        .unwrap();
}

fn run_body_time_qualified_tick() {
    let plot = parse(
        "plot remote_tick {\n clock: time/tick(count = 1, period-ms = 7)\n observe: conduit-test/tick-observer\n clock.tick >> observe.in\n}\n",
        &installed_std::test_catalog(),
    )
    .unwrap();
    let mut source_host = host("remote-tick-source");
    let mut sink_host = Box::new(host("remote-tick-sink"));
    let source = source_host.advertisement().clone();
    let sink = sink_host.advertisement().clone();
    let hosts = [source.clone(), sink.clone()];
    let capability = |host: &conduit_core::HostAdvertisement, kind: &str| {
        host.capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == kind)
            .unwrap()
            .capability_id
            .clone()
    };
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("remote_tick/clock"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: capability(&source, conduit_time::TICK_KIND),
                },
            ),
            (
                GearId::from("remote_tick/observe"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: capability(&sink, "conduit-test/tick-observer"),
                },
            ),
        ]),
    };
    let mut line = process_owned_line_offer_with_limits(
        "remote-tick-line",
        "remote-tick-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "remote-tick-instance",
        &source,
        &sink,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: conduit_time::TICK_ENCODED_LEN,
            maximum_buffered_bytes: conduit_time::TICK_ENCODED_LEN,
            maximum_frame_bytes: 8_192,
        },
    );
    line.contract.scope = conduit_core::LineScope::LocalNetwork;
    line.contract.security = conduit_core::LineSecurity::PlaintextNetwork;
    let line_candidates = BTreeMap::from([(
        (
            GearId::from("remote_tick/clock"),
            GearId::from("remote_tick/observe"),
        ),
        vec![line.line_id.clone()],
    )]);
    let plan = plan_with_options(
        &plot,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_time::TICK_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 2);
    let wake = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("remote-tick/born"),
    )
    .unwrap()
    .wake(1, SignId::from("remote-tick/wake"))
    .unwrap()
    .1;
    let requirement = BodyTimeRequirement::new(
        wake.body_id.as_str().into(),
        BodyTimeTolerance::new(10, TemporalScale::Milliseconds),
        MonotonicDuration::new(100, TemporalScale::Milliseconds),
    )
    .unwrap();
    let body_plan = BodyPlan::seal_with_body_time(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan: plan.clone(),
        }],
        requirement.clone(),
    )
    .unwrap();
    let samples = [(&source, 1_000), (&sink, 9_000)].map(|(host, ticks)| {
        MonotonicInstant::new(
            ticks,
            MonotonicClockIdentity::new(
                host.host_id.clone(),
                host.boot_id.clone(),
                "test/steady".into(),
                TemporalScale::Milliseconds,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
    });
    let correlations = samples.clone().map(|sample| {
        BodyClockCorrelation::new(
            wake.body_id.as_str().into(),
            TemporalScale::Milliseconds,
            1,
            sample,
            10_000,
            0,
            100,
            1,
            10_000,
            ClockProvenance::External {
                provider_id: "fixture/provider".into(),
                admission_reference: "fixture/admission".into(),
                policy_id: "fixture/policy".into(),
            },
        )
        .unwrap()
    });
    let evidence = [0, 1].map(|index| BodyHostClockEvidence {
        sample: &samples[index],
        correlation: &correlations[index],
        transport_uncertainty: MonotonicDuration::new(1, TemporalScale::Milliseconds),
        scheduler_uncertainty: MonotonicDuration::new(1, TemporalScale::Milliseconds),
    });
    let admission = body_plan.admit_body_time(&evidence).unwrap();
    let fragment = |host: &conduit_core::HostAdvertisement| {
        plan.fragments
            .iter()
            .find(|fragment| fragment.host_id == host.host_id)
            .unwrap()
    };
    let mut source_runtime = source_host
        .prepare_remote_fragment_with_body_time(
            &body_plan,
            &admission,
            fragment(&source),
            &mut FixedClock(samples[0].clone()),
        )
        .unwrap();
    let mut sink_runtime = sink_host
        .prepare_remote_fragment_with_body_time(
            &body_plan,
            &admission,
            fragment(&sink),
            &mut FixedClock(samples[1].clone()),
        )
        .unwrap();
    let source_endpoint = source_runtime.sessions().iter().next().unwrap().endpoint;
    let sink_endpoint = sink_runtime.sessions().iter().next().unwrap().endpoint;
    crate::remote_cord_sessions::activate_in_process(
        source_runtime
            .sessions_mut()
            .get_mut(source_endpoint)
            .unwrap(),
        sink_runtime.sessions_mut().get_mut(sink_endpoint).unwrap(),
    )
    .unwrap();
    let transfer = (0..16)
        .find_map(|_| {
            if let Some(transfer) = source_runtime.next_egress(source_endpoint).unwrap() {
                return Some(transfer);
            }
            if let Some(request) = source_runtime.next_host_request() {
                assert_eq!(
                    source_runtime
                        .describe_host_request(request)
                        .unwrap()
                        .contract_id,
                    conduit_core::wait_host_call_requirement().contract_id
                );
                source_runtime
                    .complete_host_call(
                        request,
                        conduit_kernel::HostCallOutcome {
                            disposition: conduit_kernel::HostCallDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            source_runtime
                .step_with_body_time(&mut FixedClock(samples[0].clone()))
                .unwrap();
            None
        })
        .expect("temporal source reaches remote Cord");
    assert_eq!(
        transfer.bytes.len(),
        conduit_time::TICK_ENCODED_LEN as usize
    );
    let source_after_wait = MonotonicInstant::new(1_007, samples[0].clock().clone()).unwrap();
    let sink_at_receive = MonotonicInstant::new(9_008, samples[1].clock().clone()).unwrap();
    assert!(matches!(
        requirement.assess_with_execution_bounds(
            &correlations[0],
            &source_after_wait,
            evidence[0].transport_uncertainty,
            evidence[0].scheduler_uncertainty,
        ),
        conduit_core::BodyTimeQuality::Ready { .. }
    ));
    assert!(matches!(
        requirement.assess_with_execution_bounds(
            &correlations[1],
            &sink_at_receive,
            evidence[1].transport_uncertainty,
            evidence[1].scheduler_uncertainty,
        ),
        conduit_core::BodyTimeQuality::Ready { .. }
    ));
    source_runtime.accept_egress(&transfer).unwrap();
    assert_eq!(
        sink_runtime
            .admit_ingress(sink_endpoint, transfer.sequence, &transfer.bytes)
            .unwrap(),
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted {
            sequence: transfer.sequence
        }
    );
    source_runtime.deliver_egress(&transfer).unwrap();
    sink_runtime.close_ingress(sink_endpoint).unwrap();
    let mut sink_drained = false;
    for _ in 0..8 {
        if let Some(request) = sink_runtime.next_host_request() {
            let work = sink_runtime.describe_host_request(request).unwrap();
            assert_eq!(work.input, transfer.bytes);
            sink_runtime
                .complete_host_call(
                    request,
                    conduit_kernel::HostCallOutcome {
                        disposition: conduit_kernel::HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .unwrap();
        }
        if matches!(
            sink_runtime
                .step_with_body_time(&mut FixedClock(samples[1].clone()))
                .unwrap(),
            conduit_kernel::scheduler::SchedulerStatus::Drained
        ) {
            sink_drained = true;
            break;
        }
    }
    assert!(sink_drained);
    assert!(sink_runtime
        .step()
        .unwrap_err()
        .contains("clock-checked stepping"));
    assert!(sink_runtime
        .runtime_mut()
        .step()
        .unwrap_err()
        .contains("clock-checked stepping"));
    let stale = MonotonicInstant::new(20_000, samples[1].clock().clone()).unwrap();
    assert!(matches!(
        sink_runtime.step_with_body_time(&mut FixedClock(stale)),
        Err(crate::RemoteBodyTimeStepRefusal::Quality(
            conduit_core::BodyTimeQuality::Unsupported { .. }
        ))
    ));
    assert!(matches!(
        sink_runtime.step_with_body_time(&mut FixedClock(samples[1].clone())),
        Err(crate::RemoteBodyTimeStepRefusal::Quality(
            conduit_core::BodyTimeQuality::Unsupported { .. }
        ))
    ));
    let regressed = MonotonicInstant::new(999, samples[0].clock().clone()).unwrap();
    assert_eq!(
        source_runtime.step_with_body_time(&mut FixedClock(regressed)),
        Err(crate::RemoteBodyTimeStepRefusal::Quality(
            conduit_core::BodyTimeQuality::Unsupported {
                reason: conduit_core::BodyTimeRefusal::Regressed,
            }
        ))
    );
    source_host.release_remote_fragment(source_runtime).unwrap();
    sink_host.release_remote_fragment(sink_runtime).unwrap();
}
