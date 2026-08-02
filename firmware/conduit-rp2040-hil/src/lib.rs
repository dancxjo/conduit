#![no_std]

use conduit_core::{
    CAPABILITY_REPORT_SCHEMA_VERSION, CapabilityReport, ExecutorKind, Id, PinnedDescriptor,
    PlanResourceBudget, ReportResource, ResourceRef, SemanticHash, validate_capability_report,
};
use conduit_embedded::{
    EmbeddedHostServices, EmbeddedInterest, EmbeddedNode, EmbeddedOutcome, EmbeddedProfile,
    EmbeddedStep, EmbeddedStorage, EmbeddedValue, HostReply, InterestSet,
    STATIC_PLAN_SCHEMA_VERSION, StaticCord, StaticNode, StaticPlan, StepContext,
};

include!(concat!(env!("OUT_DIR"), "/firmware_identity.rs"));

pub type ReferenceStorage = EmbeddedStorage<3, 2, 4, 2, 16, 64, 4, 4>;
pub const PLAN_HASH: SemanticHash = SemanticHash::from_bytes([
    0x70, 0xea, 0xda, 0x87, 0x20, 0x87, 0x75, 0xc2, 0xbc, 0xfe, 0x7c, 0xe2, 0xed, 0xbc, 0x82, 0x01,
    0xd6, 0xcc, 0x18, 0xe4, 0x95, 0x5a, 0xa8, 0x16, 0x4e, 0x9b, 0x48, 0x51, 0x69, 0x91, 0x04, 0x54,
]);
/// Identity of the generic RP2040 board profile implemented by this artifact.
///
/// This profile deliberately does not identify a Pico W or its CYW43 radio.
pub const GENERIC_RP2040_BOARD_PROFILE: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("conduit/board.rp2040-generic"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        48, 7, 31, 188, 116, 146, 112, 76, 78, 248, 103, 199, 60, 133, 163, 94, 176, 13, 182, 55,
        152, 215, 186, 1, 209, 45, 185, 134, 65, 89, 253, 23,
    ]),
};

/// Identity of the Raspberry Pi Pico W board profile (RP2040 microcontroller + CYW43439 Wi-Fi).
pub const PICO_W_BOARD_PROFILE: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("conduit/board.pico-w"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        101, 112, 105, 99, 111, 119, 45, 98, 111, 97, 114, 100, 45, 112, 114, 111, 102, 105, 108,
        101, 45, 118, 49, 45, 115, 101, 109, 97, 110, 116, 105, 99,
    ]),
};

use conduit_core::ReportCapability;

pub const PICO_W_CAPABILITIES: [ReportCapability<'static>; 5] = [
    ReportCapability {
        interface: PinnedDescriptor {
            id: Id("conduit/host.wifi-network"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([201; 32]),
        },
        mode: Id("ap"),
        subject: Id("cyw43"),
        details: SemanticHash::from_bytes([202; 32]),
        capacity: FIXED_EXECUTOR_BUDGET,
    },
    ReportCapability {
        interface: PinnedDescriptor {
            id: Id("conduit/host.wifi-network"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([201; 32]),
        },
        mode: Id("sta"),
        subject: Id("cyw43"),
        details: SemanticHash::from_bytes([203; 32]),
        capacity: FIXED_EXECUTOR_BUDGET,
    },
    ReportCapability {
        interface: PinnedDescriptor {
            id: Id("conduit/host.tcp-socket"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([204; 32]),
        },
        mode: Id("client"),
        subject: Id("embassy-net"),
        details: SemanticHash::from_bytes([205; 32]),
        capacity: FIXED_EXECUTOR_BUDGET,
    },
    ReportCapability {
        interface: PinnedDescriptor {
            id: Id("conduit/host.udp-socket"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([206; 32]),
        },
        mode: Id("bound"),
        subject: Id("embassy-net"),
        details: SemanticHash::from_bytes([207; 32]),
        capacity: FIXED_EXECUTOR_BUDGET,
    },
    ReportCapability {
        interface: PinnedDescriptor {
            id: Id("conduit/transport.zenoh-pico"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([208; 32]),
        },
        mode: Id("session"),
        subject: Id("zenoh-pico"),
        details: SemanticHash::from_bytes([209; 32]),
        capacity: FIXED_EXECUTOR_BUDGET,
    },
];

pub const FIXED_EXECUTOR_BUDGET: PlanResourceBudget = PlanResourceBudget {
    memory_bytes: 64 * 1024,
    storage_bytes: 0,
    cpu_units: 1,
    timers: 4,
    transports: 1,
    checkpoints: 0,
    evidence_bytes: 16 * 1024,
};

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
    let resources = [ReportResource {
        resource: ResourceRef {
            kind: Id("conduit/static-memory-pool"),
            id: Id("sram0"),
        },
        descriptor: PinnedDescriptor {
            id: Id("conduit/rp2040-fixed-pools"),
            schema_version: 0,
            semantic_hash: selected.identity,
        },
        capacity: FIXED_EXECUTOR_BUDGET,
        exclusive: true,
    }];
    let executors = [ExecutorKind::Firmware];
    let targets = [Id("thumbv6m-none-eabi")];
    let abis = [Id("conduit-static-step")];
    let constraints = [
        GENERIC_RP2040_BOARD_PROFILE.semantic_hash,
        selected.identity,
        FIRMWARE_IDENTITY,
    ];
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id("conduit/rp2040-generic-firmware-report"),
        host: Id("conduit/rp2040-generic"),
        boot_id: Id("conduit/rp2040-generic-boot"),
        reporter: PinnedDescriptor {
            id: Id("conduit/rp2040-generic-firmware"),
            schema_version: 0,
            semantic_hash: FIRMWARE_IDENTITY,
        },
        trust: PinnedDescriptor {
            id: Id("conduit/linked-firmware-trust"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([16; 32]),
        },
        membership: None,
        time_basis: Id("clock/boot-ticks"),
        observed_at_tick,
        valid_until_tick: observed_at_tick.saturating_add(1_000),
        available: FIXED_EXECUTOR_BUDGET,
        capabilities: &[],
        resources: &resources,
        topology: &[],
        supported_executors: &executors,
        supported_targets: &targets,
        supported_abis: &abis,
        minimum_plan_version: 0,
        maximum_plan_version: conduit_core::EXECUTION_PLAN_SCHEMA_VERSION,
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
        conduit_core::EXECUTION_PLAN_SCHEMA_VERSION,
        &mut scratch,
    )
    .expect("fresh fixed capability report");
    action(report)
}

pub fn with_pico_w_capability_report<R>(
    observed_at_tick: u64,
    action: impl FnOnce(CapabilityReport<'_>) -> R,
) -> R {
    let selected = profile();
    let resources = [ReportResource {
        resource: ResourceRef {
            kind: Id("conduit/static-memory-pool"),
            id: Id("sram0"),
        },
        descriptor: PinnedDescriptor {
            id: Id("conduit/pico-w-fixed-pools"),
            schema_version: 0,
            semantic_hash: selected.identity,
        },
        capacity: FIXED_EXECUTOR_BUDGET,
        exclusive: true,
    }];
    let executors = [ExecutorKind::Firmware];
    let targets = [Id("thumbv6m-none-eabi")];
    let abis = [Id("conduit-static-step")];
    let constraints = [
        PICO_W_BOARD_PROFILE.semantic_hash,
        selected.identity,
        FIRMWARE_IDENTITY,
    ];
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id("conduit/pico-w-firmware-report"),
        host: Id("conduit/pico-w"),
        boot_id: Id("conduit/pico-w-boot"),
        reporter: PinnedDescriptor {
            id: Id("conduit/pico-w-firmware"),
            schema_version: 0,
            semantic_hash: FIRMWARE_IDENTITY,
        },
        trust: PinnedDescriptor {
            id: Id("conduit/linked-firmware-trust"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([16; 32]),
        },
        membership: None,
        time_basis: Id("clock/boot-ticks"),
        observed_at_tick,
        valid_until_tick: observed_at_tick.saturating_add(1_000),
        available: FIXED_EXECUTOR_BUDGET,
        capabilities: &PICO_W_CAPABILITIES,
        resources: &resources,
        topology: &[],
        supported_executors: &executors,
        supported_targets: &targets,
        supported_abis: &abis,
        minimum_plan_version: 0,
        maximum_plan_version: conduit_core::EXECUTION_PLAN_SCHEMA_VERSION,
        current_constraints: &constraints,
    };
    let mut scratch = [SemanticHash::from_bytes([0; 32]); 32];
    report.identity = report
        .computed_semantic_hash(&mut scratch)
        .expect("fixed capability report");
    validate_capability_report(
        &report,
        Id("clock/boot-ticks"),
        observed_at_tick,
        conduit_core::EXECUTION_PLAN_SCHEMA_VERSION,
        &mut scratch,
    )
    .expect("fresh fixed capability report");
    action(report)
}

pub mod pico_w {
    use super::*;

    /// Microcontroller and radio pin configuration for Pico W hardware.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PicoWHardwareConfig {
        pub cyw43_pio: u8,
        pub cyw43_sm: u8,
        pub cyw43_dma: u8,
        pub uart_tx_pin: u8,
        pub uart_rx_pin: u8,
        pub i2c_sda_pin: u8,
        pub i2c_scl_pin: u8,
        pub power_toggle_pin: u8,
        pub txs_oe_pin: u8,
        pub status_led_pin: u8,
        pub charging_indicator_pin: u8,
    }

    impl PicoWHardwareConfig {
        pub const DEFAULT: Self = Self {
            cyw43_pio: 0,
            cyw43_sm: 0,
            cyw43_dma: 0,
            uart_tx_pin: 0,
            uart_rx_pin: 1,
            i2c_sda_pin: 2,
            i2c_scl_pin: 3,
            power_toggle_pin: 18,
            txs_oe_pin: 19,
            status_led_pin: 17,
            charging_indicator_pin: 20,
        };
    }

    /// Pico W host services abstraction layer managing hardware capabilities and network state.
    pub struct PicoWHostServices {
        pub config: PicoWHardwareConfig,
        pub wifi_ap_active: bool,
        pub wifi_sta_active: bool,
        pub indicator: bool,
    }

    impl PicoWHostServices {
        #[must_use]
        pub const fn new(config: PicoWHardwareConfig) -> Self {
            Self {
                config,
                wifi_ap_active: true,
                wifi_sta_active: false,
                indicator: false,
            }
        }
    }

    impl Default for PicoWHostServices {
        fn default() -> Self {
            Self::new(PicoWHardwareConfig::DEFAULT)
        }
    }

    impl EmbeddedHostServices<16> for PicoWHostServices {
        fn invoke(&mut self, binding: u16, request: EmbeddedValue<16>) -> HostReply<16> {
            match binding {
                0 => HostReply::Completed(
                    EmbeddedValue::from_slice(&42_u32.to_be_bytes()).expect("fixed sample"),
                ),
                1 if request.length == 1 => {
                    self.indicator = request.bytes[0] != 0;
                    HostReply::Completed(EmbeddedValue::EMPTY)
                }
                2 => HostReply::Completed(
                    EmbeddedValue::from_slice(&[u8::from(self.wifi_ap_active)]).expect("ap status"),
                ),
                _ => HostReply::Failed(Id("fixture/pico-w-operation")),
            }
        }
    }
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
