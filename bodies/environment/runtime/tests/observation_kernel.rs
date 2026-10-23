//! Ordinary planning, real pulse owner binding, Source decoding and semantic
//! Forms reach one production-kernel observation without allocation in Play.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[allow(
    dead_code,
    reason = "pulse fixture retains both rich and assigned binding paths"
)]
#[path = "support/assignment.rs"]
mod assignment;
#[path = "support/back.rs"]
mod back;
#[allow(
    dead_code,
    reason = "shared fixture also serves the raw assigned-kernel proof"
)]
#[path = "support/plan.rs"]
mod plan;
#[allow(
    dead_code,
    reason = "shared fixture also serves the raw assigned-kernel proof"
)]
#[path = "support/pulse.rs"]
mod pulse;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use back::{Back, Observed};
use conduit_core::{validate_canonical_structured_value as validate, ConfigurationValue, Quantity};
use conduit_environment_runtime::{
    observation::ObservationContract, CaptureBack, PreparedCaptureSelector,
};
use conduit_kernel::{
    scheduler::{FixedScheduler, SchedulerStatus},
    *,
};
use conduit_plot::PortableExpressionProgram;
const N: usize = 14;
const C: usize = 13;
const P: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const R: usize = N * P;
#[test]
fn source_and_forms_emit_quantities_and_preserve_all_sensor_outcomes() {
    for scenario in 0..=6 {
        run(scenario)
    }
}
fn run(scenario: usize) {
    let contract = ObservationContract::prepare().unwrap();
    if scenario == 0 {
        eprintln!(
            "observation_type_prefix_bytes={} result_type_prefix_bytes={} quantity_bytes={}",
            contract.observation_type().canonical_bytes().unwrap().len(),
            contract.result_type().canonical_bytes().unwrap().len(),
            conduit_core::QUANTITY_ENCODED_LEN
        );
    }
    let (ready, effects) = pulse::ready(scenario);
    let (plan, lowered) = plan::prepare_with_observation(&ready, contract.kind().clone());
    assert_eq!(lowered.nodes.len(), N);
    assert_eq!(lowered.cords.len(), C);
    let (mut owner, pulse_node) = pulse::bind(ready, &plan, &lowered).unwrap();
    let observed: [Observed; 4] = core::array::from_fn(|_| Observed::new());
    let mut selectors: [Option<PreparedCaptureSelector>; N] = core::array::from_fn(|_| None);
    let mut prepared: [Option<Back<'_>>; N] = core::array::from_fn(|_| None);
    for placement in &plan.fragments[0].startup_order {
        let i = usize::from(lowered.identity.node_for_placement(placement).unwrap().0);
        let node = &lowered.nodes[i];
        let gear = plan.fragments[0]
            .placements
            .iter()
            .find(|g| g.placement_id == node.placement_id)
            .unwrap();
        prepared[i] = Some(
            if gear.kind_id.as_str() == format!("{}/begin", plan::PREFIX) {
                Back::Begin {
                    sent: false,
                    staged: false,
                }
            } else if let Some(index) = [
                "decoded",
                "timed-out",
                "provider-failed",
                "geometry-refused",
            ]
            .iter()
            .position(|name| gear.kind_id.as_str() == format!("{}/{name}", plan::PREFIX))
            {
                Back::Observe {
                    observed: &observed[index],
                    pending: Box::new([0; 4096]),
                    length: 0,
                    staged: false,
                }
            } else if gear.kind_id.as_str() == conduit_pulse_capture::PULSE_READ_KIND {
                Back::Native(CaptureBack::pulse(1024))
            } else if gear.kind_id.as_str() == conduit_environment_runtime::observation::KIND {
                Back::Native(CaptureBack::observation(&contract).unwrap())
            } else if let Some(ConfigurationValue::Text(text)) = gear
                .configuration
                .iter()
                .find(|v| v.key == "program")
                .map(|v| &v.value)
            {
                Back::Native(
                    CaptureBack::expression(
                        &PortableExpressionProgram::from_canonical_hex(text).unwrap(),
                        conduit_core::PortTemporal::Flow { closes: true },
                    )
                    .unwrap(),
                )
            } else {
                let ConfigurationValue::Text(text) = &gear
                    .configuration
                    .iter()
                    .find(|v| v.key == "selector")
                    .unwrap()
                    .value
                else {
                    panic!("checked selector")
                };
                selectors[i] = Some(
                    PreparedCaptureSelector::prepare(
                        conduit_core::StructuredSelector::from_canonical_hex(text).unwrap(),
                        1024,
                        1024,
                    )
                    .unwrap(),
                );
                Back::Native(CaptureBack::selector(1024))
            },
        );
    }
    let mut routes = FixedRoutes::<R, C>::new(P as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<N>::new(1);
    for call in &lowered.host_calls {
        calls.install(call.node, call.binding).unwrap();
    }
    calls.seal().unwrap();
    let sign_slots = lowered.sign_items.max(
        u16::try_from((lowered.sign_bytes as usize).div_ceil(core::mem::size_of::<KernelEvent>()))
            .unwrap(),
    );
    let mut capacities = [1024; 32];
    capacities[..2].fill(4096);
    let values = HostedValueStore::new_with_slot_capacities(&capacities, 38912).unwrap();
    let mut kernel = FixedScheduler::<_, _, _, N, C, P, C, R, C, N, N>::new_with_host_calls(
        core::array::from_fn(|i| lowered.node_specs[i]),
        core::array::from_fn(|i| lowered.cords[i].spec),
        routes,
        calls,
        prepared.map(Option::unwrap),
        values,
        HostedSignLog::new(sign_slots, lowered.sign_bytes).unwrap(),
    )
    .unwrap();
    let mut pulse_calls = 0;
    let mut drained = false;
    let (_, allocation) = allocation_probe::observe(|| {
        for _ in 0..1000 {
            if let Some(request) = kernel.next_host_request() {
                let input = kernel.host_value(request.input.value).unwrap();
                let bytes = if request.node == pulse_node {
                    pulse_calls += 1;
                    Some(
                        block_on(owner.invoke(request.node, request.call, request.request, input))
                            .unwrap(),
                    )
                } else {
                    selectors[usize::from(request.node.0)]
                        .as_mut()
                        .unwrap()
                        .select(input)
                        .unwrap()
                };
                let output = bytes.map(|b| {
                    BoundedValueRef::new(kernel.store_host_value(b).unwrap(), 1024).unwrap()
                });
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            if kernel.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
    });
    assert!(drained);
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(pulse_calls, 1);
    assert_eq!(owner.inspection().completed_operations, 1);
    assert_eq!(effects.get(), usize::from(scenario != 4));
    let target = match scenario {
        0 | 1 | 5 | 6 => 0,
        2 => 1,
        3 => 2,
        _ => 3,
    };
    for (i, w) in observed.iter().enumerate() {
        assert_eq!(w.count.get(), usize::from(i == target));
    }
    // Face construction follows the completed Play and has its own allocation
    // boundary; the kernel allocation observation above excludes this work.
    if scenario == 0 || scenario == 5 {
        let bytes = observed[0].bytes.borrow();
        let payload = validate(&bytes.0[..bytes.1])
            .unwrap()
            .variant_payload("observation")
            .unwrap()
            .unwrap();
        let quantity = |name| {
            let value = payload.record_field(name).unwrap().unwrap();
            Quantity::decode(&value.value_node()[5..]).unwrap()
        };
        manifest(
            &plan,
            Some(quantity("temperature")),
            Some(quantity("humidity")),
            scenario,
        );
    } else {
        manifest(&plan, None, None, scenario);
    }
    if scenario == 0 || scenario == 5 {
        let bytes = observed[0].bytes.borrow();
        let result = validate(&bytes.0[..bytes.1]).unwrap();
        let observation = result.variant_payload("observation").unwrap().unwrap();
        let expected = if scenario == 5 {
            [("temperature", "-10.8°C"), ("humidity", "53%")]
        } else {
            [("temperature", "19.8°C"), ("humidity", "56%")]
        };
        for (name, literal) in expected {
            let leaf = observation.record_field(name).unwrap().unwrap();
            let node = leaf.value_node();
            assert_eq!(node[0], 0);
            assert_eq!(
                Quantity::decode(&node[5..]).unwrap(),
                Quantity::parse_plot_literal(literal).unwrap()
            );
        }
        eprintln!("environment_first_class_observation_bytes={}", bytes.1);
    } else if scenario == 1 || scenario == 6 {
        let bytes = observed[0].bytes.borrow();
        assert!(validate(&bytes.0[..bytes.1])
            .unwrap()
            .variant_payload("unavailable")
            .unwrap()
            .unwrap()
            .variant_payload(if scenario == 6 {
                "short-frame"
            } else {
                "checksum"
            })
            .unwrap()
            .is_some());
    }
}
fn block_on<F: core::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("finite transcript must complete immediately"),
    }
}

fn manifest(
    plan: &conduit_core::Plan,
    temperature: Option<Quantity>,
    humidity: Option<Quantity>,
    scenario: usize,
) {
    use conduit_environment_face::{contribution, SensorFailure, SensorStatus, SUBJECT};
    use conduit_face::{Face, FaceBasis, FaceContributionBasis};
    use conduit_seven_segment_mask::{
        render, PreparedEnvironmentFace, SegmentPatternForm, SevenSegmentLook,
        SevenSegmentSelection,
    };
    let f = &plan.fragments[0];
    let active = conduit_core::bind_active_play(&plan.plan_id, &f.host_id, &f.boot_id, 0);
    let status = match scenario {
        0 | 5 => SensorStatus::Available,
        1 => SensorStatus::Unavailable(SensorFailure::Checksum),
        2 => SensorStatus::Unavailable(SensorFailure::CaptureTimeout),
        3 => SensorStatus::Unavailable(SensorFailure::ProviderFailed),
        4 => SensorStatus::Unavailable(SensorFailure::GeometryRefused),
        _ => SensorStatus::Unavailable(SensorFailure::ShortFrame),
    };
    let fragment = contribution(
        temperature,
        humidity,
        temperature.is_some(),
        status,
        FaceContributionBasis {
            checked_plot_id: plan.checked_plot_id.clone(),
            plan_id: plan.plan_id.clone(),
            active_play_id: active.active_play_id.clone(),
            required_interaction_context: None,
        },
    )
    .unwrap();
    let face = Face::new(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(plan.source_document_id.clone()),
            checked_plot_id: Some(plan.checked_plot_id.clone()),
            expanded_plot_id: Some(plan.expanded_plot_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: Some(active.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
    )
    .unwrap();
    let view = PreparedEnvironmentFace::prepare(&face, SUBJECT).unwrap();
    let result = render(
        &view,
        SevenSegmentSelection::Temperature,
        &SevenSegmentLook::fahrenheit(),
    );
    let expected = match scenario {
        0 => [0x7d, 0x07, 0x7d, 0x71],
        5 => [0x06, 0x5b, 0x7d, 0x71],
        _ => [0x40; 4],
    };
    assert_eq!(
        result
            .show
            .digits()
            .map(|d| SegmentPatternForm::encode(d)[0]),
        expected
    );
    assert_eq!(
        *result.show.decimal_points(),
        if temperature.is_some() {
            [false, true, false, false]
        } else {
            [false; 4]
        }
    );
}
#[test]
fn coordinate_overflow_and_wrong_schemas_refuse_without_allocating() {
    use conduit_core::{PreparedStructuredComposer as C, StructuredInfoTypeShape as S};
    use conduit_environment_runtime::observation::{ObservationRefusal, PreparedObservationCodec};
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let S::Variant { cases, .. } = contract.input_type().shape() else {
        panic!("variant")
    };
    let frame_ty = cases
        .iter()
        .find(|c| c.tag() == "frame")
        .unwrap()
        .payload_type();
    let S::Record { fields, .. } = frame_ty.shape() else {
        panic!("record")
    };
    for (temperature, humidity, valid) in [
        (198_i128, 560_i128, true),
        (32768, 560, false),
        (198, -1, false),
        (198, 65536, false),
        (-108, 530, true),
    ] {
        let mut h = C::new(fields[0].value_type(), 128).unwrap();
        let mut t = C::new(fields[1].value_type(), 128).unwrap();
        h.leaf(&humidity.to_le_bytes()).unwrap();
        t.leaf(&temperature.to_le_bytes()).unwrap();
        let mut frame = C::new(frame_ty, 1024).unwrap();
        frame
            .record(&[
                validate(h.encoded()).unwrap(),
                validate(t.encoded()).unwrap(),
            ])
            .unwrap();
        let mut input = C::new(contract.input_type(), 1024).unwrap();
        input
            .variant("frame", validate(frame.encoded()).unwrap())
            .unwrap();
        let (result, allocated) =
            allocation_probe::observe(|| codec.convert(input.encoded()).map(|bytes| bytes.len()));
        assert_eq!((allocated.allocations, allocated.reallocations), (0, 0));
        if valid {
            assert_eq!(result, Ok(2274));
        } else {
            assert_eq!(result, Err(ObservationRefusal::CoordinateOutOfRange));
        }
    }
    assert_eq!(
        codec.convert(&[]),
        Err(ObservationRefusal::Canonical(
            conduit_core::StructuredInfoRefusal::WrongType
        ))
    );
}
