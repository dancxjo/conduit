#![cfg(feature = "kernel-step")]
use conduit_ai::fixed_numeric_codec::FixedF32VectorCodec;

#[test]
fn exact_history_and_atomic_window_record_fit_and_roundtrip() {
    let mut history = FixedF32VectorCodec::<128>::prepare_history().unwrap();
    let values = core::array::from_fn(|i| i as f32);
    let bytes = history.encode(&values).unwrap().to_vec();
    let mut actual = [0.; 128];
    history.decode(&bytes, &mut actual).unwrap();
    assert_eq!(actual, values);
    let mut result = FixedF32VectorCodec::<320>::prepare_window_result().unwrap();
    let values = core::array::from_fn(|i| i as f32);
    let bytes = result.encode(&values).unwrap().to_vec();
    let mut actual = [0.; 320];
    result.decode(&bytes, &mut actual).unwrap();
    assert_eq!(actual, values);
    assert!(history.maximum_bytes() <= 16384);
    assert!(result.maximum_bytes() <= 16384);
    println!(
        "history128={}B combinedwindow320={}B",
        history.maximum_bytes(),
        result.maximum_bytes()
    );
}

use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_preparation::*, fixed_numeric_window_back::*,
};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    ValueStorage,
};
use std::{cell::Cell, rc::Rc};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn ty(name: &str) -> StructuredInfoType {
    fixed_numeric_type(name).unwrap()
}
fn fixture_kind(name: &str, value: &StructuredInfoType, source: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: value.profile().unwrap().value_kind().clone(),
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(name),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: if source { vec![] } else { vec![port.clone()] },
        outputs: if source { vec![port.clone()] } else { vec![] },
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: if source {
                FrontValueLocation::Output(port.port_id)
            } else {
                FrontValueLocation::Input(port.port_id)
            },
            contract: CheckedValueContract::new(port.value_kind, 16_384, vec![]).unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16_384,
        },
    }
}
fn offer(kind: Kind) -> CapabilityOffer {
    let name = kind.kind_id.as_str().to_string();
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(name.clone()),
            execution_profile_id: ExecutionProfileId::from("numeric-test@1"),
            implementation_id: ImplementationId::from(name.clone()),
            artifact_id: ArtifactId::from(name),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
fn plan() -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = vec![];
    for (name, t, source) in [
        ("numeric-test/vector", "NumericF32Vector64", true),
        ("numeric-test/history", "NumericHistory2x64", true),
        ("numeric-test/sink", "NumericWindow2x64", false),
    ] {
        let kind = fixture_kind(name, &ty(t), source);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(kind.clone()).unwrap();
        offers.push(offer(kind));
    }
    offers.push(fixed_window_offer().unwrap());
    let plot=conduit_plot::parse_with_startup("plot window-proof {\n vector: numeric-test/vector\n history: numeric-test/history\n window: numeric/history2x64\n sink: numeric-test/sink\n vector.value >> window.value\n history.value >> window.history\n window.result >> sink.value\n}\n",&startup,&profile).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("numeric-test"),
        boot_id: BootId::from("numeric-test-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("numeric-test@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        16_384,
    )
    .unwrap()
}

enum Driver {
    Source {
        reference: ValueRef,
        sent: bool,
    },
    Window(Box<FixedWindowBack>),
    Sink {
        pause: Rc<Cell<bool>>,
        seen: Rc<Cell<bool>>,
        expected: Vec<u8>,
    },
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source { reference, sent } => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), *reference).unwrap();
                *sent = true;
                StepOutcome::Complete
            }
            Self::Window(back) => back.step(io, inputs),
            Self::Sink {
                pause,
                seen,
                expected,
            } => {
                if pause.get() {
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
                if io.input(KPort(0)).is_none() {
                    return StepOutcome::Await;
                }
                assert_eq!(inputs.input(KPort(0)).unwrap(), expected);
                io.consume(KPort(0)).unwrap();
                seen.set(true);
                StepOutcome::Complete
            }
        }
    }
    fn step_committed(&mut self) {
        if let Self::Window(back) = self {
            <FixedWindowBack as StepBack<PORTS>>::step_committed(back);
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Window(back) = self {
            <FixedWindowBack as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Window(back) = self {
            <FixedWindowBack as StepBack<PORTS>>::cancel(back);
        }
    }
}

type Scheduler = FixedScheduler<Driver, HostedValueStore, HostedSignLog, 4, 3, PORTS, 3, 4, 3>;
fn scheduler(slots: u16, seen: Rc<Cell<bool>>) -> Scheduler {
    let plan = plan();
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(slots, 16384, u32::from(slots) * 16384).unwrap();
    let mut vcodec = FixedF32VectorCodec::<64>::prepare(&ty("NumericF32Vector64")).unwrap();
    let mut hcodec = FixedF32VectorCodec::<128>::prepare_history().unwrap();
    let v = core::array::from_fn(|i| 200.0 + i as f32);
    let h = core::array::from_fn(|i| i as f32);
    let vr = values.store(vcodec.encode(&v).unwrap()).unwrap();
    let hr = values.store(hcodec.encode(&h).unwrap()).unwrap();
    let mut expected = [0.; 320];
    expected[..64].copy_from_slice(&h[64..]);
    expected[64..128].copy_from_slice(&v);
    expected[128..256].copy_from_slice(&h);
    expected[256..].copy_from_slice(&v);
    let mut codec = FixedF32VectorCodec::<320>::prepare_window_result().unwrap();
    let expected = codec.encode(&expected).unwrap().to_vec();
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(i, gear)| match gear.kind_id.as_str() {
            "numeric-test/vector" => Driver::Source {
                reference: vr,
                sent: false,
            },
            "numeric-test/history" => Driver::Source {
                reference: hr,
                sent: false,
            },
            "numeric/history2x64" => Driver::Window(Box::new(
                FixedWindowBack::prepare_planned::<PORTS>(
                    gear,
                    lowered.node_specs[i].maximum_step_fuel,
                )
                .unwrap(),
            )),
            "numeric-test/sink" => Driver::Sink {
                pause: Rc::new(Cell::new(false)),
                seen: seen.clone(),
                expected: expected.clone(),
            },
            _ => panic!("unexpected fixture"),
        })
        .collect();
    let mut routes = FixedRoutes::<4, 3>::new(1);
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
    FixedScheduler::new(
        lowered.node_specs.try_into().unwrap(),
        lowered
            .cords
            .into_iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes,
        drivers
            .try_into()
            .unwrap_or_else(|_| panic!("four drivers")),
        values,
        HostedSignLog::new(1024, 1024 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}
#[test]
fn ordinary_plan_window_publishes_history_and_window_together() {
    let seen = Rc::new(Cell::new(false));
    let mut scheduler = scheduler(8, seen.clone());
    for _ in 0..64 {
        scheduler.step().unwrap();
        if seen.get() {
            break;
        }
    }
    assert!(seen.get());
}
#[test]
fn ordinary_window_pressure_preserves_inputs_and_cancel_has_no_result() {
    let seen = Rc::new(Cell::new(false));
    let mut scheduler = scheduler(2, seen.clone());
    let mut refused = false;
    for _ in 0..32 {
        if let Err(error) = scheduler.step() {
            assert!(matches!(
                error,
                conduit_kernel::scheduler::SchedulerError::Storage(
                    conduit_kernel::StorageError::ItemCapacityExceeded
                )
            ));
            refused = true;
            break;
        }
    }
    assert!(refused);
    assert!(!seen.get());
    assert!(scheduler
        .drivers()
        .iter()
        .any(|d| matches!(d,Driver::Window(b) if !b.has_committed_result())));
    scheduler.cancel().unwrap();
    assert_eq!(
        scheduler.step().unwrap(),
        conduit_kernel::scheduler::SchedulerStatus::Cancelled
    );
    assert!(!seen.get());
}
