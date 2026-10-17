use conduit_core::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, PortId as KernelPort, ValueRef, ValueStorage,
};
use conduit_speech::kernel::{self, NativeSpeechBack};
use std::{cell::Cell, rc::Rc};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub type Scheduler = FixedScheduler<Driver, HostedValueStore, HostedSignLog, 4, 3, PORTS, 3, 4, 3>;

fn fixture_kind(source: bool) -> Kind {
    let id = if source {
        "speech-test/text"
    } else {
        "speech-test/sink"
    };
    let port = PortDescriptor {
        port_id: port_id(if source { "text" } else { "audio" }),
        value_kind: kind_id(if source {
            "value/text"
        } else {
            conduit_audio::AUDIO_PCM_INFO_ID
        }),
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: if source {
            PortTemporal::Value
        } else {
            PortTemporal::Flow { closes: true }
        },
        abnormal_kind: None,
    };
    let location = if source {
        FrontValueLocation::Output(port.port_id.clone())
    } else {
        FrontValueLocation::Input(port.port_id.clone())
    };
    Kind {
        kind_id: kind_id(id),
        kind_contract_revision: KindIdentity::from(id),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: if source { vec![] } else { vec![port.clone()] },
        outputs: if source { vec![port.clone()] } else { vec![] },
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location,
            contract: CheckedValueContract::new(
                port.value_kind,
                if source {
                    512
                } else {
                    kernel::MAXIMUM_PCM_BYTES as u32
                },
                vec![],
            )
            .unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 1,
            max_queue_bytes: 512,
        },
    }
}
fn fixture_offer(source: bool) -> CapabilityOffer {
    let kind = fixture_kind(source);
    let id = kind.kind_id.as_str().to_string();
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(id.clone()),
            execution_profile_id: ExecutionProfileId::from("speech-test@1"),
            implementation_id: ImplementationId::from(id.clone()),
            artifact_id: ArtifactId::from(id),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
pub fn plan() -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    kernel::install(&mut startup, &mut profile).unwrap();
    for source in [true, false] {
        let kind = fixture_kind(source);
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(kind).unwrap();
    }
    let plot = conduit_plot::parse_with_startup("plot speech {\n text: speech-test/text\n voice: speech/utterance(clock = 7, language-request = { language: \"language/english\", variety: some({ identity: \"pronunciation/native-english@2\", language: \"language/english\" }), variety_policy: exact_variety(empty) })\n left: speech-test/sink\n right: speech-test/sink\n text.text >> voice.text\n voice.audio >> left.audio\n voice.audio >> right.audio\n}\n", &startup, &profile).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("speech-test-host"),
        boot_id: BootId::from("speech-test-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("speech-test@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![kernel::offer(), fixture_offer(true), fixture_offer(false)],
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        512,
    )
    .unwrap()
}
pub enum Driver {
    Text {
        reference: ValueRef,
        emitted: bool,
    },
    Voice(Box<NativeSpeechBack>),
    Sink {
        paused: Rc<Cell<bool>>,
        pcm: Vec<u8>,
        next_frame: u64,
        complete: bool,
    },
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Text { reference, emitted } => {
                if *emitted {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KernelPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KernelPort(0), *reference).unwrap();
                *emitted = true;
                StepOutcome::Complete
            }
            Self::Voice(back) => back.step(io, bytes),
            Self::Sink {
                paused,
                pcm,
                next_frame,
                complete,
            } => {
                if paused.get() {
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
                if io.input(KernelPort(0)).is_some() {
                    let raw = bytes.input(KernelPort(0)).unwrap();
                    let (header, payload) =
                        conduit_audio::PcmFrameHeader::decode_frame(raw).unwrap();
                    assert_eq!(header.clock_id(), 7);
                    assert_eq!(header.start_frame(), *next_frame);
                    assert_eq!(header.sample_rate_hz(), 8000);
                    assert_eq!(header.layout(), conduit_audio::PcmChannelLayout::Mono);
                    assert_eq!(
                        header.representation(),
                        conduit_audio::PcmSampleRepresentation::Signed16LittleEndian
                    );
                    assert!(!header.discontinuity());
                    *next_frame += u64::from(header.frame_count());
                    assert!(pcm.len() + payload.len() <= pcm.capacity());
                    pcm.extend_from_slice(payload);
                    io.consume(KernelPort(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(KernelPort(0)) {
                    io.consume_closed(KernelPort(0)).unwrap();
                    *complete = true;
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
    fn step_committed(&mut self) {
        if let Self::Voice(back) = self {
            StepBack::<PORTS>::step_committed(back.as_mut());
        }
    }
    fn prepared_output(&self, port: KernelPort) -> Option<&[u8]> {
        match self {
            Self::Voice(back) => StepBack::<PORTS>::prepared_output(back.as_ref(), port),
            _ => None,
        }
    }
    fn cancel(&mut self) {
        if let Self::Voice(back) = self {
            StepBack::<PORTS>::cancel(back.as_mut());
        }
    }
}
pub fn scheduler(text: &[u8], paused: Rc<Cell<bool>>) -> Scheduler {
    let plan = plan();
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    assert_eq!(lowered.nodes.len(), 4);
    assert_eq!(lowered.cords.len(), 3);
    let mut values = HostedValueStore::new(8, 512, 4096).unwrap();
    let reference = values.store(text).unwrap();
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "speech-test/text" => Driver::Text {
                reference,
                emitted: false,
            },
            kernel::KIND => Driver::Voice(Box::new(
                NativeSpeechBack::prepare::<PORTS>(gear, KernelPort(0), KernelPort(0)).unwrap(),
            )),
            "speech-test/sink" => Driver::Sink {
                paused: if gear.gear_id.as_str().ends_with("right") {
                    paused.clone()
                } else {
                    Rc::new(Cell::new(false))
                },
                pcm: Vec::with_capacity(480000),
                next_frame: 0,
                complete: false,
            },
            _ => panic!("unexpected fixture kind"),
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
    let specs: [_; 4] = lowered.node_specs.try_into().unwrap();
    let cords: [_; 3] = lowered
        .cords
        .into_iter()
        .map(|c| c.spec)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let drivers: [_; 4] = drivers
        .try_into()
        .unwrap_or_else(|_| panic!("four drivers"));
    let signs = HostedSignLog::new(
        20000,
        20000 * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
    )
    .unwrap();
    FixedScheduler::new(specs, cords, routes, drivers, values, signs).unwrap()
}
