#![cfg(feature = "kernel-step")]
use conduit_ai::fixed_numeric_codec::FixedF32VectorCodec;
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_pair_catalog::*, fixed_numeric_pair_flow::*,
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
        temporal: PortTemporal::Flow { closes: true },
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
            contract: CheckedValueContract::new(
                port.value_kind,
                conduit_plot::maximum_prepared_transport_value_bytes(value).unwrap(),
                vec![],
            )
            .unwrap(),
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
    install_fixed_flow_pair_catalogs(&mut startup, &mut profile).unwrap();
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
    offers.push(fixed_flow_pair_offer("numeric/flow-pair40x164").unwrap());
    let plot=conduit_plot::parse_with_startup("plot window-proof {\n vector: numeric-test/vector\n history: numeric-test/history\n window: numeric/flow-pair40x164\n sink: numeric-test/sink\n vector.value >> window.left\n history.value >> window.right\n window.result >> sink.value\n}\n",&startup,&profile).unwrap();
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
        references: Vec<ValueRef>,
        cursor: usize,
        staged: bool,
    },
    Window(Box<FixedFlowPairBack>),
    Sink {
        pause: Rc<Cell<bool>>,
        seen: Rc<Cell<usize>>,
        expected: Vec<u8>,
    },
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source {
                references,
                cursor,
                staged,
            } => {
                *staged = false;
                if *cursor == references.len() {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), references[*cursor]).unwrap();
                *staged = true;
                StepOutcome::Progress
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
                if io.input_closed(KPort(0)) {
                    return StepOutcome::Complete;
                }
                if io.input(KPort(0)).is_none() {
                    return StepOutcome::Await;
                }
                assert_eq!(inputs.input(KPort(0)).unwrap(), expected);
                io.consume(KPort(0)).unwrap();
                seen.set(seen.get() + 1);
                StepOutcome::Progress
            }
        }
    }
    fn step_committed(&mut self) {
        if let Self::Source { cursor, staged, .. } = self {
            if *staged {
                *cursor += 1;
                *staged = false;
            }
        }
        if let Self::Window(back) = self {
            <FixedFlowPairBack as StepBack<PORTS>>::step_committed(back);
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Window(back) = self {
            <FixedFlowPairBack as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Window(back) = self {
            <FixedFlowPairBack as StepBack<PORTS>>::cancel(back);
        }
    }
}

type Scheduler = FixedScheduler<Driver, HostedValueStore, HostedSignLog, 4, 3, PORTS, 3, 4, 3>;
fn scheduler(slots: u16, seen: Rc<Cell<usize>>, corrupt: bool, right_frames: usize) -> Scheduler {
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
    let vr = (0..3)
        .map(|_| values.store(&vector).unwrap())
        .collect::<Vec<_>>();
    let hr = (0..right_frames)
        .map(|_| values.store(hcodec.encode(&h).unwrap()).unwrap())
        .collect::<Vec<_>>();
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
                references: vr.clone(),
                cursor: 0,
                staged: false,
            },
            "numeric-test/history" => Driver::Source {
                references: hr.clone(),
                cursor: 0,
                staged: false,
            },
            "numeric/flow-pair40x164" => Driver::Window(Box::new(
                FixedFlowPairBack::prepare_planned::<PORTS>(
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

fn committed(s: &Scheduler) -> u64 {
    s.drivers()
        .iter()
        .find_map(|d| match d {
            Driver::Window(b) => Some(b.committed_frames()),
            _ => None,
        })
        .unwrap()
}
#[test]
fn closing_flow_pair_reuses_one_owner_for_three_frames() {
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(12, seen.clone(), false, 3);
    for _ in 0..128 {
        if scheduler.step().unwrap() == conduit_kernel::scheduler::SchedulerStatus::Drained {
            break;
        }
    }
    assert_eq!(seen.get(), 3);
    assert_eq!(committed(&scheduler), 3);
    assert!(fixed_flow_pair_offer("numeric/pair40x164").is_err());
}
#[test]
fn storage_pressure_and_cancel_do_not_commit_pair_progress() {
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(6, seen.clone(), false, 3);
    let mut refused = false;
    for _ in 0..32 {
        if let Err(e) = scheduler.step() {
            assert!(matches!(
                e,
                conduit_kernel::scheduler::SchedulerError::Storage(
                    conduit_kernel::StorageError::ItemCapacityExceeded
                )
            ));
            refused = true;
            break;
        }
    }
    assert!(refused);
    assert_eq!(seen.get(), 0);
    assert_eq!(committed(&scheduler), 0);
    scheduler.cancel().unwrap();
    assert_eq!(
        scheduler.step().unwrap(),
        conduit_kernel::scheduler::SchedulerStatus::Cancelled
    );
    assert_eq!(committed(&scheduler), 0);
}
#[test]
fn nonfinite_member_refuses_before_consumption_or_publication() {
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(12, seen.clone(), true, 3);
    let mut refused = false;
    for _ in 0..32 {
        if let Err(e) = scheduler.step() {
            assert!(matches!(
                e,
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
    assert_eq!(seen.get(), 0);
    assert_eq!(committed(&scheduler), 0);
}
#[test]
fn mismatched_flow_lengths_refuse_after_exact_common_prefix() {
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(12, seen.clone(), false, 2);
    let mut refused = false;
    for _ in 0..128 {
        if let Err(e) = scheduler.step() {
            assert!(matches!(
                e,
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
    assert_eq!(committed(&scheduler), 2);
    assert_eq!(seen.get(), 2);
}

#[test]
fn hosted_pair_factory_requires_exact_selected_placement_and_offer() {
    use conduit_composite::KernelOperationFactory;
    let plan = plan();
    let factory =
        conduit_std_host::fixed_numeric_pair_flow::FixedFlowPairOperationFactory::for_plan(&plan)
            .unwrap();
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == "numeric/flow-pair40x164")
        .unwrap();
    let mut store = HostedValueStore::new(4, 16384, 65536).unwrap();
    factory.prepare(gear, &mut store).unwrap();
    let mut foreign = gear.clone();
    foreign.implementation_id = ImplementationId::from("foreign/pair");
    assert!(factory.prepare(&foreign, &mut store).is_err());
    foreign = gear.clone();
    foreign.placement_id = PlacementId::from("foreign/pair");
    assert!(factory.budget(&foreign).is_err());
}
