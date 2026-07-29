#![no_std]

use conduit_core::{
    CAPABILITY_REPORT_SCHEMA_VERSION, CapabilityReport, ExecutorKind, Id, PinnedDescriptor,
    PlanResourceBudget, ReportCapability, ReportResource, ResourceRef, SemanticHash,
    validate_capability_report,
};
use conduit_embedded::{
    EmbeddedHostServices, EmbeddedInterest, EmbeddedNode, EmbeddedOutcome, EmbeddedProfile,
    EmbeddedStep, EmbeddedStorage, EmbeddedValue, HostReply, InterestSet,
    STATIC_PLAN_SCHEMA_VERSION, StaticCord, StaticNode, StaticPlan, StepContext,
};

include!(concat!(env!("OUT_DIR"), "/firmware_identity.rs"));

pub type ReferenceStorage = EmbeddedStorage<3, 2, 4, 2, 16, 64, 4, 4>;
pub const PLAN_HASH: SemanticHash = SemanticHash::from_bytes([
    154, 65, 61, 157, 190, 9, 134, 255, 20, 228, 123, 218, 124, 237, 112, 66, 65, 219, 150, 38, 20,
    41, 211, 195, 153, 152, 206, 249, 31, 169, 105, 79,
]);

pub const NODES: [StaticNode<'static>; 3] = [
    StaticNode {
        semantic_path: Id("fixture/sensor"),
        implementation: Id("fixture/rp2040-sensor"),
        input_ports: 0,
        output_ports: 1,
        maximum_step_work: 2,
        nesting_depth: 1,
    },
    StaticNode {
        semantic_path: Id("fixture/threshold"),
        implementation: Id("fixture/rp2040-threshold"),
        input_ports: 1,
        output_ports: 1,
        maximum_step_work: 2,
        nesting_depth: 1,
    },
    StaticNode {
        semantic_path: Id("fixture/indicator"),
        implementation: Id("fixture/rp2040-indicator"),
        input_ports: 1,
        output_ports: 0,
        maximum_step_work: 2,
        nesting_depth: 1,
    },
];
pub const CORDS: [StaticCord<'static>; 2] = [
    StaticCord {
        semantic_id: Id("fixture/sample"),
        producer_node: 0,
        producer_port: 0,
        consumer_node: 1,
        consumer_port: 0,
        slot_start: 0,
        capacity: 1,
        maximum_value_bytes: 4,
    },
    StaticCord {
        semantic_id: Id("fixture/decision"),
        producer_node: 1,
        producer_port: 0,
        consumer_node: 2,
        consumer_port: 0,
        slot_start: 1,
        capacity: 1,
        maximum_value_bytes: 1,
    },
];

pub fn profile() -> EmbeddedProfile {
    let mut profile = EmbeddedProfile {
        identity: SemanticHash::from_bytes([0; 32]),
        maximum_nodes: 3,
        maximum_cords: 2,
        maximum_ports: 4,
        maximum_queue_slots: 2,
        maximum_value_bytes: 16,
        maximum_evidence_records: 64,
        maximum_timers: 4,
        maximum_interests_per_node: 4,
        maximum_nesting: 2,
        maximum_timer_delay: 1_000,
        static_ram_budget_bytes: 64 * 1024,
        stack_budget_bytes: 4 * 1024,
        flash_budget_bytes: 96 * 1024,
    };
    profile.seal().expect("static embedded profile");
    profile
}

pub fn plan(profile: &EmbeddedProfile) -> StaticPlan<'static> {
    StaticPlan {
        schema_version: STATIC_PLAN_SCHEMA_VERSION,
        full_plan_hash: PLAN_HASH,
        profile_hash: profile.identity,
        nodes: &NODES,
        cords: &CORDS,
    }
}

pub fn with_capability_report<R>(
    observed_at_tick: u64,
    action: impl FnOnce(CapabilityReport<'_>) -> R,
) -> R {
    let selected = profile();
    let capabilities = [ReportCapability {
        interface: PinnedDescriptor {
            id: Id("conduit/host.wifi-network"),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([13; 32]),
        },
        mode: Id("ap"),
        subject: Id("cyw43"),
        details: SemanticHash::from_bytes([15; 32]),
        capacity: PlanResourceBudget {
            memory_bytes: 64 * 1024,
            storage_bytes: 0,
            cpu_units: 1,
            timers: 4,
            transports: 1,
            checkpoints: 0,
            evidence_bytes: 16 * 1024,
        },
    }];
    let resources = [ReportResource {
        resource: ResourceRef {
            kind: Id("conduit/static-memory-pool"),
            id: Id("sram0"),
        },
        descriptor: PinnedDescriptor {
            id: Id("conduit/rp2040-fixed-pools"),
            schema_version: 1,
            semantic_hash: selected.identity,
        },
        capacity: capabilities[0].capacity,
        exclusive: true,
    }];
    let executors = [ExecutorKind::Firmware];
    let targets = [Id("thumbv6m-none-eabi")];
    let abis = [Id("conduit-static-step-v1")];
    let constraints = [selected.identity, FIRMWARE_IDENTITY];
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id("conduit/rp2040-firmware-report"),
        host: Id("conduit/rp2040"),
        reporter: PinnedDescriptor {
            id: Id("conduit/rp2040-firmware"),
            schema_version: 1,
            semantic_hash: FIRMWARE_IDENTITY,
        },
        trust: PinnedDescriptor {
            id: Id("conduit/linked-firmware-trust"),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([16; 32]),
        },
        membership: None,
        time_basis: Id("clock/boot-ticks"),
        observed_at_tick,
        valid_until_tick: observed_at_tick.saturating_add(1_000),
        available: capabilities[0].capacity,
        capabilities: &capabilities,
        resources: &resources,
        topology: &[],
        supported_executors: &executors,
        supported_targets: &targets,
        supported_abis: &abis,
        minimum_plan_version: 3,
        maximum_plan_version: 9,
        current_constraints: &constraints,
    };
    let mut scratch = [SemanticHash::from_bytes([0; 32]); 8];
    report.identity = report
        .computed_semantic_hash(&mut scratch)
        .expect("fixed capability report");
    validate_capability_report(
        &report,
        Id("clock/boot-ticks"),
        observed_at_tick,
        9,
        &mut scratch,
    )
    .expect("fresh fixed capability report");
    action(report)
}

pub struct ReferenceHost {
    pub indicator: bool,
}

impl EmbeddedHostServices<16> for ReferenceHost {
    fn invoke(&mut self, binding: u16, request: EmbeddedValue<16>) -> HostReply<16> {
        match binding {
            0 => HostReply::Completed(
                EmbeddedValue::from_slice(&42_u32.to_be_bytes()).expect("fixed sample"),
            ),
            1 if request.length == 1 => {
                self.indicator = request.bytes[0] != 0;
                HostReply::Completed(EmbeddedValue::EMPTY)
            }
            _ => HostReply::Failed(Id("fixture/host-operation")),
        }
    }
}

pub enum ReferenceDriver {
    Sensor { emitted: bool },
    Threshold,
    Indicator,
}

pub fn drivers() -> [ReferenceDriver; 3] {
    [
        ReferenceDriver::Sensor { emitted: false },
        ReferenceDriver::Threshold,
        ReferenceDriver::Indicator,
    ]
}

impl EmbeddedNode<ReferenceHost, 16, 4, 4> for ReferenceDriver {
    fn step(&mut self, context: &mut StepContext<'_, ReferenceHost, 16, 4>) -> EmbeddedStep<4> {
        match self {
            Self::Sensor { emitted } => {
                if *emitted {
                    return EmbeddedStep::completed();
                }
                let HostReply::Completed(sample) = context
                    .invoke_host(0, EmbeddedValue::EMPTY)
                    .expect("bounded host operation")
                else {
                    return failed("fixture/sample");
                };
                context.send(0, sample).expect("planned output");
                *emitted = true;
                EmbeddedStep::progress()
            }
            Self::Threshold => {
                if let Some(sample) = context.input(0) {
                    let sample =
                        u32::from_be_bytes(sample.bytes[..4].try_into().expect("fixed sample"));
                    context.consume(0).expect("planned input");
                    context
                        .send(
                            0,
                            EmbeddedValue::from_slice(&[u8::from(sample >= 40)])
                                .expect("fixed decision"),
                        )
                        .expect("planned output");
                    EmbeddedStep::progress()
                } else if context.input_closed(0) {
                    EmbeddedStep::completed()
                } else {
                    EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Input(0)))
                }
            }
            Self::Indicator => {
                if context.input(0).is_some() {
                    let decision = context.consume(0).expect("planned input");
                    let _reply = context
                        .invoke_host(1, decision)
                        .expect("bounded host operation");
                    EmbeddedStep::progress()
                } else if context.input_closed(0) {
                    EmbeddedStep::completed()
                } else {
                    EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Input(0)))
                }
            }
        }
    }
}

fn failed(code: &'static str) -> EmbeddedStep<4> {
    EmbeddedStep {
        outcome: EmbeddedOutcome::Failed(Id(code)),
        interests: InterestSet::EMPTY,
    }
}
