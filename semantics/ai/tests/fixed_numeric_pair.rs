#![cfg(feature = "kernel-step")]
use conduit_ai::fixed_numeric_codec::FixedF32VectorCodec;
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_pair_back::*, fixed_numeric_pair_catalog::*,
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
    fixed_numeric_pair_contracts()
        .unwrap()
        .into_iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, ty, _)| ty)
        .unwrap_or_else(|| fixed_numeric_type(name).unwrap())
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
    install_fixed_numeric_pair_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = vec![];
    for (name, t, source) in [
        ("numeric-test/vector", "NumericF32Vector40", true),
        ("numeric-test/history", "NumericF32Vector164", true),
        ("numeric-test/sink", "NumericPair40x164", false),
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
    offers.push(fixed_value_pair_offer("numeric/pair40x164").unwrap());
    let plot=conduit_plot::parse_with_startup("plot window-proof {\n vector: numeric-test/vector\n history: numeric-test/history\n window: numeric/pair40x164\n sink: numeric-test/sink\n vector.value >> window.left\n history.value >> window.right\n window.result >> sink.value\n}\n",&startup,&profile).unwrap();
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
    Window(Box<FixedValuePairBack>),
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
            <FixedValuePairBack as StepBack<PORTS>>::step_committed(back);
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Window(back) = self {
            <FixedValuePairBack as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Window(back) = self {
            <FixedValuePairBack as StepBack<PORTS>>::cancel(back);
        }
    }
}

type Scheduler = FixedScheduler<Driver, HostedValueStore, HostedSignLog, 4, 3, PORTS, 3, 4, 3>;
fn scheduler(slots: u16, seen: Rc<Cell<bool>>, corrupt: bool) -> Scheduler {
    let plan = plan();
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(slots, 16384, u32::from(slots) * 16384).unwrap();
    let mut vcodec = FixedF32VectorCodec::<40>::prepare(&ty("NumericF32Vector40")).unwrap();
    let mut hcodec = FixedF32VectorCodec::<164>::prepare(&ty("NumericF32Vector164")).unwrap();
    let v = core::array::from_fn(|i| 200.0 + i as f32);
    let h = core::array::from_fn(|i| i as f32);
    let mut vector = vcodec.encode(&v).unwrap().to_vec();
    if corrupt {
        let offset = vector
            .windows(4)
            .position(|bytes| bytes == 200f32.to_le_bytes())
            .unwrap();
        vector[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    }
    let vr = values.store(&vector).unwrap();
    let hr = values.store(hcodec.encode(&h).unwrap()).unwrap();
    let mut encoder = PreparedTypedTuplePairEncoder::new(
        ty("NumericF32Vector40"),
        vcodec.maximum_bytes() as u32,
        ty("NumericF32Vector164"),
        hcodec.maximum_bytes() as u32,
    )
    .unwrap();
    let expected = encoder
        .encode(vcodec.encoded(), hcodec.encoded())
        .unwrap()
        .to_vec();
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
            "numeric/pair40x164" => Driver::Window(Box::new(
                FixedValuePairBack::prepare_planned::<PORTS>(
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
fn ordinary_plan_value_pair_publishes_exact_typed_values_together() {
    let seen = Rc::new(Cell::new(false));
    let mut scheduler = scheduler(8, seen.clone(), false);
    for _ in 0..64 {
        scheduler.step().unwrap();
        if seen.get() {
            break;
        }
    }
    assert!(seen.get());
}
#[test]
fn ordinary_value_pair_pressure_preserves_inputs_and_cancel_has_no_result() {
    let seen = Rc::new(Cell::new(false));
    let mut scheduler = scheduler(2, seen.clone(), false);
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

#[test]
fn ordinary_pair_refuses_nonfinite_member_before_consumption_or_publication() {
    let seen = Rc::new(Cell::new(false));
    let mut scheduler = scheduler(8, seen.clone(), true);
    let mut refused = false;
    for _ in 0..32 {
        if let Err(error) = scheduler.step() {
            assert!(matches!(
                error,
                conduit_kernel::scheduler::SchedulerError::BackFailed(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidInput,
                    ..
                })
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
        .any(|driver| matches!(driver,Driver::Window(back) if !back.has_committed_result())));
}
