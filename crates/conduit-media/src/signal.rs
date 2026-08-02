//! Bounded standing-patch signal contracts and deterministic reference providers.
//!
//! Events, gates, controls, and audio frames remain different value contracts.
//! Nothing here starts a run, chooses a host clock, or grants device authority.

use conduit_core::{
    ArtifactDigest, ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability,
    ConfigRequirement, ConnectionCardinality, Delivery, Direction, ExecutorKind, Id,
    LossAcceptance, NodeContract, PinnedDescriptor, PortContract, PortFlowConstraints, Presence,
    SemanticHash, Sensitivity, TemporalContract, TerminalContract, TypeContractRef,
    ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, HostedPrimitiveImplementation, HostedServiceStep,
    HostedServiceStepContext, InstalledArtifactRegistration, InstalledImplementationRegistration,
    Registry, RegistryError, ResolutionError, RunIo, RuntimeError, Value,
};
use sha2::{Digest as _, Sha256};

pub const MAXIMUM_CONTROL_LEVEL: u32 = 1024;
pub const MAXIMUM_SEQUENCE_STEPS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissedPulsePolicy {
    Coalesce,
    DropWithCount,
    Fail,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockAdvance {
    pub pulses: Vec<u64>,
    pub dropped: u64,
    pub discontinuity: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockAdvanceReason {
    InvalidConfiguration,
    TimeReversal,
    ArithmeticOverflow,
    MissedPulse,
}

/// Deterministic host-neutral reference state for the exact clock policy.
/// Providers own their wake mechanism; this state owns phase, live enable/rate
/// transitions, missed-pulse accounting, and discontinuity boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeterministicClockState {
    period_ticks: u64,
    reset_phase: u64,
    enabled: bool,
    observed_tick: u64,
    next_pulse_tick: u64,
    pending_discontinuity: bool,
}

impl DeterministicClockState {
    pub fn new(
        period_ticks: u64,
        startup_phase: u64,
        reset_phase: u64,
    ) -> Result<Self, ClockAdvanceReason> {
        if period_ticks == 0 || startup_phase >= period_ticks || reset_phase >= period_ticks {
            return Err(ClockAdvanceReason::InvalidConfiguration);
        }
        Ok(Self {
            period_ticks,
            reset_phase,
            enabled: true,
            observed_tick: 0,
            next_pulse_tick: startup_phase,
            pending_discontinuity: false,
        })
    }

    pub fn reset(&mut self, tick: u64) -> Result<(), ClockAdvanceReason> {
        self.observe(tick)?;
        self.next_pulse_tick = tick
            .checked_add(self.reset_phase)
            .ok_or(ClockAdvanceReason::ArithmeticOverflow)?;
        self.pending_discontinuity = true;
        Ok(())
    }

    pub fn set_enabled(&mut self, tick: u64, enabled: bool) -> Result<(), ClockAdvanceReason> {
        self.observe(tick)?;
        if enabled && !self.enabled && self.next_pulse_tick <= tick {
            self.next_pulse_tick = tick
                .checked_add(self.period_ticks)
                .ok_or(ClockAdvanceReason::ArithmeticOverflow)?;
        }
        self.enabled = enabled;
        Ok(())
    }

    pub fn change_period(
        &mut self,
        tick: u64,
        period_ticks: u64,
    ) -> Result<(), ClockAdvanceReason> {
        if period_ticks == 0 {
            return Err(ClockAdvanceReason::InvalidConfiguration);
        }
        self.observe(tick)?;
        self.period_ticks = period_ticks;
        self.next_pulse_tick = tick
            .checked_add(period_ticks)
            .ok_or(ClockAdvanceReason::ArithmeticOverflow)?;
        self.pending_discontinuity = true;
        Ok(())
    }

    pub fn advance(
        &mut self,
        tick: u64,
        maximum_pending: u16,
        policy: MissedPulsePolicy,
    ) -> Result<ClockAdvance, ClockAdvanceReason> {
        self.observe(tick)?;
        let discontinuity = std::mem::take(&mut self.pending_discontinuity);
        if !self.enabled || self.next_pulse_tick > tick {
            return Ok(ClockAdvance {
                pulses: Vec::new(),
                dropped: 0,
                discontinuity,
            });
        }
        let due = (tick - self.next_pulse_tick) / self.period_ticks + 1;
        if maximum_pending == 0
            || (due > u64::from(maximum_pending) && policy == MissedPulsePolicy::Fail)
        {
            return Err(ClockAdvanceReason::MissedPulse);
        }
        let emitted = match policy {
            MissedPulsePolicy::Coalesce => 1,
            MissedPulsePolicy::DropWithCount | MissedPulsePolicy::Fail => {
                due.min(u64::from(maximum_pending))
            }
        };
        let mut pulses = Vec::with_capacity(emitted as usize);
        for offset in 0..emitted {
            pulses.push(
                self.next_pulse_tick
                    .checked_add(
                        offset
                            .checked_mul(self.period_ticks)
                            .ok_or(ClockAdvanceReason::ArithmeticOverflow)?,
                    )
                    .ok_or(ClockAdvanceReason::ArithmeticOverflow)?,
            );
        }
        self.next_pulse_tick = self
            .next_pulse_tick
            .checked_add(
                due.checked_mul(self.period_ticks)
                    .ok_or(ClockAdvanceReason::ArithmeticOverflow)?,
            )
            .ok_or(ClockAdvanceReason::ArithmeticOverflow)?;
        Ok(ClockAdvance {
            pulses,
            dropped: due - emitted,
            discontinuity,
        })
    }

    fn observe(&mut self, tick: u64) -> Result<(), ClockAdvanceReason> {
        if tick < self.observed_tick {
            return Err(ClockAdvanceReason::TimeReversal);
        }
        self.observed_tick = tick;
        Ok(())
    }
}

pub fn bounded_counter_next(current: u32, modulus: u32) -> Result<u32, ClockAdvanceReason> {
    if modulus == 0 || current >= modulus {
        return Err(ClockAdvanceReason::InvalidConfiguration);
    }
    Ok(if current + 1 == modulus {
        0
    } else {
        current + 1
    })
}

/// Canonical host-neutral meanings for the standing-patch signal taxonomy.
///
/// These descriptors are documentation as data: providers may choose another
/// representation, but they must prove the same event, held-state, sampled
/// control, and finite retained-state meanings before satisfying the type.
pub const EVENT_DESCRIPTOR: &str =
    "conduit.media/event|0|one-occurrence|tick-identity-order-delivery-policy";
pub const GATE_DESCRIPTOR: &str = "conduit.media/gate|0|held-activation|tick-transition-identity";
pub const CONTROL_DESCRIPTOR: &str =
    "conduit.media/control|0|typed-time-varying-level|tick-lane-unit";
pub const RETAINED_STATE_DESCRIPTOR: &str =
    "conduit.media/retained-state|0|finite-snapshot|tick-items-bytes-policy";

pub const EVENT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.media/event"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x3a, 0x4b, 0x41, 0x20, 0xae, 0x16, 0x6c, 0x08, 0x69, 0xc8, 0x18, 0x45, 0x0b, 0x65, 0xc3,
        0x7a, 0xe9, 0x46, 0x82, 0x71, 0x40, 0x91, 0x9e, 0x55, 0x3c, 0x6a, 0x54, 0x80, 0x2d, 0x3a,
        0x14, 0x47,
    ]),
};

pub const GATE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.media/gate"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x36, 0xdf, 0x9e, 0x6f, 0xd5, 0x95, 0x6b, 0xba, 0xea, 0x0d, 0x8a, 0x54, 0x8a, 0xa8, 0x62,
        0x94, 0x59, 0x3f, 0x76, 0x77, 0x6f, 0x6e, 0x69, 0xa7, 0x01, 0x73, 0x63, 0xa1, 0x7d, 0x1f,
        0x20, 0xfc,
    ]),
};

pub const CONTROL_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.media/control"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x45, 0xbc, 0x38, 0x32, 0x79, 0x2a, 0x43, 0x28, 0x25, 0x12, 0x64, 0x68, 0x85, 0x34, 0x4c,
        0x31, 0x99, 0x73, 0x81, 0x7f, 0xb3, 0xb3, 0x04, 0x42, 0xcd, 0xe2, 0xd5, 0x9f, 0x48, 0x1c,
        0x12, 0x93,
    ]),
};

pub const RETAINED_STATE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit.media/retained-state"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x54, 0xfd, 0xfb, 0x1c, 0xd0, 0x5a, 0x0f, 0x71, 0xdf, 0x65, 0x91, 0x2c, 0x6f, 0xc7, 0xe4,
        0xc0, 0x5b, 0x04, 0xd2, 0x99, 0x61, 0x40, 0x49, 0x3c, 0xe8, 0x49, 0xa3, 0xe2, 0xcf, 0x33,
        0x06, 0xdd,
    ]),
};

const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};

const U64_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/u64"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0xba, 0xd3, 0xea, 0x53, 0xd3, 0xca, 0x01, 0xa0, 0xa4, 0xd6, 0x9f, 0x86, 0xc8, 0x25,
        0x65, 0x17, 0x07, 0x16, 0x45, 0xea, 0x7d, 0x68, 0xef, 0x63, 0x6b, 0x6d, 0x94, 0x87, 0x70,
        0xf0, 0xec,
    ]),
};

const fn stream_port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: if matches!(direction, Direction::Input) {
            TerminalContract::Either
        } else {
            TerminalContract::OpenEnded
        },
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const fn optional_stream_port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Optional,
        connections: ConnectionCardinality::ZeroOrOne,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: if matches!(direction, Direction::Input) {
            TerminalContract::Either
        } else {
            TerminalContract::OpenEnded
        },
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const fn optional_state_port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Optional,
        connections: ConnectionCardinality::ZeroOrMore,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::LatestState,
        temporal: TemporalContract::RetainedState,
        terminal: if matches!(direction, Direction::Input) {
            TerminalContract::Either
        } else {
            TerminalContract::OpenEnded
        },
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const fn config_field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }
}

const TEXT_INPUT: [PortContract<'static>; 1] = [stream_port("tick", Direction::Input, TEXT_TYPE)];
const EVENT_INPUT: [PortContract<'static>; 1] =
    [stream_port("event", Direction::Input, EVENT_TYPE)];
const EVENT_OUTPUT: [PortContract<'static>; 1] =
    [stream_port("event", Direction::Output, EVENT_TYPE)];
const GATE_INPUT: [PortContract<'static>; 1] = [stream_port("gate", Direction::Input, GATE_TYPE)];
const GATE_OUTPUT: [PortContract<'static>; 1] = [stream_port("gate", Direction::Output, GATE_TYPE)];
const EVENT_TEE_OUTPUTS: [PortContract<'static>; 2] = [
    stream_port("fast", Direction::Output, EVENT_TYPE),
    stream_port("slow", Direction::Output, EVENT_TYPE),
];
const CONTROL_INPUT: [PortContract<'static>; 1] =
    [stream_port("control", Direction::Input, CONTROL_TYPE)];
const CONTROL_MERGE_INPUTS: [PortContract<'static>; 2] = [
    stream_port("left", Direction::Input, CONTROL_TYPE),
    stream_port("right", Direction::Input, CONTROL_TYPE),
];
const CONTROL_TEE_OUTPUTS: [PortContract<'static>; 2] = [
    stream_port("left", Direction::Output, CONTROL_TYPE),
    stream_port("right", Direction::Output, CONTROL_TYPE),
];
const CONTROL_OUTPUT: [PortContract<'static>; 1] =
    [stream_port("control", Direction::Output, CONTROL_TYPE)];
const CONTROL_AND_STATE_OUTPUTS: [PortContract<'static>; 2] = [
    stream_port("control", Direction::Output, CONTROL_TYPE),
    optional_state_port("state", Direction::Output, RETAINED_STATE_TYPE),
];
const RETAINED_CONTROL_AND_STATE_OUTPUTS: [PortContract<'static>; 2] = [
    PortContract {
        temporal: TemporalContract::RetainedState,
        ..stream_port("control", Direction::Output, CONTROL_TYPE)
    },
    optional_state_port("state", Direction::Output, RETAINED_STATE_TYPE),
];
const SAMPLE_HOLD_INPUTS: [PortContract<'static>; 2] = [
    stream_port("control", Direction::Input, CONTROL_TYPE),
    stream_port("trigger", Direction::Input, EVENT_TYPE),
];
const GATE_EVENT_INPUTS: [PortContract<'static>; 2] = [
    stream_port("gate", Direction::Input, GATE_TYPE),
    stream_port("trigger", Direction::Input, EVENT_TYPE),
];
const CROSSFADE_INPUTS: [PortContract<'static>; 3] = [
    stream_port("left", Direction::Input, CONTROL_TYPE),
    stream_port("right", Direction::Input, CONTROL_TYPE),
    stream_port("position", Direction::Input, CONTROL_TYPE),
];
const AUDIO_DELAY_INPUT: [PortContract<'static>; 1] = [stream_port(
    "frame",
    Direction::Input,
    crate::AUDIO_FRAME_TYPE,
)];
const CLOCK_INPUTS: [PortContract<'static>; 3] = [
    optional_stream_port("reset", Direction::Input, EVENT_TYPE),
    optional_stream_port("enable", Direction::Input, GATE_TYPE),
    optional_stream_port("rate", Direction::Input, CONTROL_TYPE),
];
const CLOCK_OUTPUTS: [PortContract<'static>; 5] = [
    optional_stream_port("pulse", Direction::Output, EVENT_TYPE),
    optional_stream_port("phase", Direction::Output, CONTROL_TYPE),
    optional_stream_port("rate", Direction::Output, CONTROL_TYPE),
    optional_state_port("enabled", Direction::Output, GATE_TYPE),
    optional_state_port("state", Direction::Output, RETAINED_STATE_TYPE),
];
const CONTROL_EVENT_OUTPUTS: [PortContract<'static>; 2] = [
    optional_stream_port("control", Direction::Output, CONTROL_TYPE),
    optional_stream_port("event", Direction::Output, EVENT_TYPE),
];
const AUDIO_CONTROL_INPUTS: [PortContract<'static>; 2] = [
    stream_port("frame", Direction::Input, crate::AUDIO_FRAME_TYPE),
    stream_port("gain", Direction::Input, CONTROL_TYPE),
];
const AUDIO_OUTPUT: [PortContract<'static>; 1] = [stream_port(
    "frame",
    Direction::Output,
    crate::AUDIO_FRAME_TYPE,
)];
const TEXT_OUTPUT: [PortContract<'static>; 1] = [stream_port("text", Direction::Output, TEXT_TYPE)];

const DIVIDER_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("divisor", U64_TYPE),
        config_field("phase", U64_TYPE),
    ],
};
const SEQUENCER_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("pattern", TEXT_TYPE),
        config_field("maximum_steps", U64_TYPE),
        config_field("lane", U64_TYPE),
        config_field("repeat", TEXT_TYPE),
    ],
};
const SLEW_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("initial", U64_TYPE),
        config_field("maximum_delta", U64_TYPE),
    ],
};
const MIXER_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("gain_numerator", U64_TYPE),
        config_field("gain_denominator", U64_TYPE),
        config_field("maximum_value", U64_TYPE),
    ],
};
const CONTROL_MERGE_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[config_field("ordering", TEXT_TYPE)],
};
const REGISTER_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("initial", U64_TYPE),
        config_field("maximum_value", U64_TYPE),
    ],
};
const CLOCK_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("time_basis", TEXT_TYPE),
        config_field("period_ticks", U64_TYPE),
        config_field("startup_phase", U64_TYPE),
        config_field("reset_phase", U64_TYPE),
        config_field("rate_mapping", TEXT_TYPE),
        config_field("enable_behavior", TEXT_TYPE),
        config_field("drift", TEXT_TYPE),
        config_field("discontinuity", TEXT_TYPE),
        config_field("missed_pulse", TEXT_TYPE),
        config_field("maximum_pending", U64_TYPE),
        config_field("pressure", TEXT_TYPE),
    ],
};
const COUNTER_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("initial", U64_TYPE),
        config_field("modulus", U64_TYPE),
        config_field("wrap", TEXT_TYPE),
    ],
};
const LFO_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("shape", TEXT_TYPE),
        config_field("minimum", U64_TYPE),
        config_field("maximum", U64_TYPE),
        config_field("period_ticks", U64_TYPE),
        config_field("startup_phase", U64_TYPE),
        config_field("discontinuity", TEXT_TYPE),
    ],
};
const ENVELOPE_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("initial", U64_TYPE),
        config_field("peak", U64_TYPE),
        config_field("attack_ticks", U64_TYPE),
        config_field("decay_ticks", U64_TYPE),
        config_field("sustain", U64_TYPE),
        config_field("release_ticks", U64_TYPE),
        config_field("retrigger", TEXT_TYPE),
        config_field("maximum_segments", U64_TYPE),
    ],
};
const SAMPLE_HOLD_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("initial", U64_TYPE),
        config_field("maximum_value", U64_TYPE),
        config_field("before_first_trigger", TEXT_TYPE),
    ],
};
const QUANTIZER_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("step", U64_TYPE),
        config_field("rounding", TEXT_TYPE),
        config_field("maximum_value", U64_TYPE),
    ],
};
const COMPARATOR_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("threshold", U64_TYPE),
        config_field("hysteresis", U64_TYPE),
        config_field("transition", TEXT_TYPE),
    ],
};
const DELAY_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("delay_ticks", U64_TYPE),
        config_field("initial", U64_TYPE),
        config_field("maximum_items", U64_TYPE),
        config_field("maximum_bytes", U64_TYPE),
        config_field("replay_gap", TEXT_TYPE),
        config_field("cancellation", TEXT_TYPE),
        config_field("terminal", TEXT_TYPE),
    ],
};
const HISTORY_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("maximum_items", U64_TYPE),
        config_field("maximum_bytes", U64_TYPE),
        config_field("eviction", TEXT_TYPE),
    ],
};
const DEPTH_BIAS_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("depth_numerator", U64_TYPE),
        config_field("depth_denominator", U64_TYPE),
        config_field("bias", U64_TYPE),
        config_field("maximum_value", U64_TYPE),
    ],
};
const SWITCH_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("positions", U64_TYPE),
        config_field("initial", U64_TYPE),
        config_field("transition", TEXT_TYPE),
    ],
};
const RAMP_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("initial", U64_TYPE),
        config_field("target", U64_TYPE),
        config_field("duration_ticks", U64_TYPE),
        config_field("rounding", TEXT_TYPE),
        config_field("discontinuity", TEXT_TYPE),
    ],
};
const TIMER_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("time_basis", TEXT_TYPE),
        config_field("delay_ticks", U64_TYPE),
        config_field("repeat", TEXT_TYPE),
        config_field("maximum_pending", U64_TYPE),
        config_field("missed_trigger", TEXT_TYPE),
        config_field("pressure", TEXT_TYPE),
    ],
};
const AUDIO_DELAY_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("delay_frames", U64_TYPE),
        config_field("maximum_frames", U64_TYPE),
        config_field("maximum_bytes", U64_TYPE),
        config_field("initial", TEXT_TYPE),
        config_field("saturation", TEXT_TYPE),
        config_field("flush", TEXT_TYPE),
        config_field("cancellation", TEXT_TYPE),
    ],
};
const PAN_MATRIX_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("input_layout", TEXT_TYPE),
        config_field("output_layout", TEXT_TYPE),
        config_field("matrix", TEXT_TYPE),
        config_field("maximum_channels", U64_TYPE),
        config_field("maximum_frames", U64_TYPE),
    ],
};
const OBSERVATION_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("maximum_history", U64_TYPE),
        config_field("maximum_bytes", U64_TYPE),
        config_field("cadence_ticks", U64_TYPE),
        config_field("retention", TEXT_TYPE),
    ],
};
const CONTROLLED_GAIN_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[
        config_field("numeric_profile", TEXT_TYPE),
        config_field("control_mapping", TEXT_TYPE),
        config_field("maximum_frames", U64_TYPE),
        config_field("maximum_work", U64_TYPE),
    ],
};

pub const EVENT_FROM_TICKER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/event/from-ticker"),
    config: ConfigContract { fields: &[] },
    inputs: &TEXT_INPUT,
    outputs: &EVENT_OUTPUT,
};
pub const CLOCK_DIVIDER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/clock-divider"),
    config: DIVIDER_CONFIG,
    inputs: &EVENT_INPUT,
    outputs: &EVENT_OUTPUT,
};
pub const EVENT_TEE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/event/tee"),
    config: ConfigContract { fields: &[] },
    inputs: &EVENT_INPUT,
    outputs: &EVENT_TEE_OUTPUTS,
};
pub const SEQUENCER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/sequencer"),
    config: SEQUENCER_CONFIG,
    inputs: &EVENT_INPUT,
    outputs: &CONTROL_OUTPUT,
};
pub const SLEW_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/slew"),
    config: SLEW_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &CONTROL_OUTPUT,
};
pub const MIXER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/mixer"),
    config: MIXER_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &CONTROL_OUTPUT,
};
pub const CONTROL_MERGE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/merge"),
    config: CONTROL_MERGE_CONFIG,
    inputs: &CONTROL_MERGE_INPUTS,
    outputs: &CONTROL_OUTPUT,
};
pub const CONTROL_TEE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/tee"),
    config: ConfigContract { fields: &[] },
    inputs: &CONTROL_INPUT,
    outputs: &CONTROL_TEE_OUTPUTS,
};
pub const REGISTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/register"),
    config: REGISTER_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &RETAINED_CONTROL_AND_STATE_OUTPUTS,
};
pub const SCOPE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/scope"),
    config: ConfigContract { fields: &[] },
    inputs: &CONTROL_INPUT,
    outputs: &TEXT_OUTPUT,
};

/// Rich clock contract. Reset, enable, and rate are typed live controls; the
/// pre-start fields describe one immutable plan epoch.
pub const CLOCK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/time/clock"),
    config: CLOCK_CONFIG,
    inputs: &CLOCK_INPUTS,
    outputs: &CLOCK_OUTPUTS,
};
pub const COUNTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/time/counter"),
    config: COUNTER_CONFIG,
    inputs: &EVENT_INPUT,
    outputs: &CONTROL_AND_STATE_OUTPUTS,
};
pub const PHASE_SOURCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/time/phase-source"),
    config: CLOCK_CONFIG,
    inputs: &CLOCK_INPUTS,
    outputs: &CONTROL_OUTPUT,
};
pub const TIMER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/time/timer"),
    config: TIMER_CONFIG,
    inputs: &GATE_INPUT,
    outputs: &CONTROL_EVENT_OUTPUTS,
};
pub const GATE_LATCH_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/gate/latch"),
    config: ConfigContract { fields: &[] },
    inputs: &EVENT_INPUT,
    outputs: &GATE_OUTPUT,
};
pub const LFO_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/lfo"),
    config: LFO_CONFIG,
    inputs: &EVENT_INPUT,
    outputs: &CONTROL_OUTPUT,
};
pub const RAMP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/ramp"),
    config: RAMP_CONFIG,
    inputs: &EVENT_INPUT,
    outputs: &CONTROL_AND_STATE_OUTPUTS,
};
pub const ENVELOPE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/envelope"),
    config: ENVELOPE_CONFIG,
    inputs: &EVENT_INPUT,
    outputs: &CONTROL_AND_STATE_OUTPUTS,
};
pub const SAMPLE_HOLD_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/sample-hold"),
    config: SAMPLE_HOLD_CONFIG,
    inputs: &SAMPLE_HOLD_INPUTS,
    outputs: &CONTROL_AND_STATE_OUTPUTS,
};
pub const QUANTIZER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/quantizer"),
    config: QUANTIZER_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &CONTROL_OUTPUT,
};
pub const COMPARATOR_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/comparator"),
    config: COMPARATOR_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &GATE_OUTPUT,
};
pub const DEPTH_BIAS_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/depth-bias"),
    config: DEPTH_BIAS_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &CONTROL_OUTPUT,
};
pub const CLOCKED_SWITCH_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/clocked-switch"),
    config: SWITCH_CONFIG,
    inputs: &SAMPLE_HOLD_INPUTS,
    outputs: &CONTROL_EVENT_OUTPUTS,
};
pub const TRIGGER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/event/from-gate"),
    config: ConfigContract { fields: &[] },
    inputs: &GATE_INPUT,
    outputs: &EVENT_OUTPUT,
};
pub const GATE_TRIGGER_LATCH_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/gate/trigger-latch"),
    config: ConfigContract { fields: &[] },
    inputs: &GATE_EVENT_INPUTS,
    outputs: &GATE_OUTPUT,
};
pub const CROSSFADE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/crossfade"),
    config: ConfigContract { fields: &[] },
    inputs: &CROSSFADE_INPUTS,
    outputs: &CONTROL_OUTPUT,
};
pub const ONE_TICK_DELAY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/state/one-tick-delay"),
    config: DELAY_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &RETAINED_CONTROL_AND_STATE_OUTPUTS,
};
pub const DELAY_LINE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/state/delay-line"),
    config: DELAY_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &RETAINED_CONTROL_AND_STATE_OUTPUTS,
};
pub const ACCUMULATOR_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/state/accumulator"),
    config: REGISTER_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &CONTROL_AND_STATE_OUTPUTS,
};
pub const HISTORY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/state/history"),
    config: HISTORY_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &CONTROL_AND_STATE_OUTPUTS,
};
pub const FEEDBACK_BOUNDARY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/state/feedback-boundary"),
    config: DELAY_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &RETAINED_CONTROL_AND_STATE_OUTPUTS,
};
pub const CONTROLLED_GAIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/controlled-gain"),
    config: CONTROLLED_GAIN_CONFIG,
    inputs: &AUDIO_CONTROL_INPUTS,
    outputs: &AUDIO_OUTPUT,
};
pub const AUDIO_DELAY_LINE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/delay-line"),
    config: AUDIO_DELAY_CONFIG,
    inputs: &AUDIO_DELAY_INPUT,
    outputs: &AUDIO_OUTPUT,
};
pub const PAN_MATRIX_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/pan-matrix"),
    config: PAN_MATRIX_CONFIG,
    inputs: &AUDIO_CONTROL_INPUTS,
    outputs: &AUDIO_OUTPUT,
};
pub const CONTROL_METER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/meter"),
    config: OBSERVATION_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &TEXT_OUTPUT,
};
pub const EVENT_LOG_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/event/log"),
    config: OBSERVATION_CONFIG,
    inputs: &EVENT_INPUT,
    outputs: &TEXT_OUTPUT,
};
pub const WAVEFORM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/waveform"),
    config: OBSERVATION_CONFIG,
    inputs: &AUDIO_DELAY_INPUT,
    outputs: &TEXT_OUTPUT,
};
pub const SPECTRUM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/audio/spectrum"),
    config: OBSERVATION_CONFIG,
    inputs: &AUDIO_DELAY_INPUT,
    outputs: &TEXT_OUTPUT,
};

fn integer(node: &Node, key: &str) -> Result<u64, ResolutionError> {
    let Some(SourceValue::Integer(value)) = node.config_value(key) else {
        return Err(ResolutionError::new(
            "CND-MSIG-001",
            format!("signal configuration `{key}` must be an integer"),
        ));
    };
    u64::try_from(*value).map_err(|_| {
        ResolutionError::new(
            "CND-MSIG-001",
            format!("signal configuration `{key}` must be nonnegative"),
        )
    })
}

fn validate_empty(node: &Node) -> Result<(), ResolutionError> {
    node.config
        .is_empty()
        .then_some(())
        .ok_or_else(|| ResolutionError::new("CND-MSIG-001", "signal adapter has no configuration"))
}

fn validate_divider(node: &Node) -> Result<(), ResolutionError> {
    let divisor = integer(node, "divisor")?;
    let phase = integer(node, "phase")?;
    if node.config.len() == 2 && (1..=1024).contains(&divisor) && phase < divisor {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-002",
            "clock divider requires divisor 1..1024 and phase below divisor",
        ))
    }
}

fn parse_pattern(node: &Node) -> Result<Vec<u32>, ResolutionError> {
    let Some(pattern) = node.config("pattern") else {
        return Err(ResolutionError::new(
            "CND-MSIG-003",
            "sequencer pattern must be comma-separated control levels",
        ));
    };
    let maximum_steps = usize::try_from(integer(node, "maximum_steps")?).map_err(|_| {
        ResolutionError::new("CND-MSIG-003", "sequencer step bound is not representable")
    })?;
    let steps = pattern
        .split(',')
        .map(str::trim)
        .map(|value| value.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            ResolutionError::new(
                "CND-MSIG-003",
                "sequencer pattern contains a non-numeric control level",
            )
        })?;
    let lane = integer(node, "lane")?;
    if node.config.len() != 4
        || steps.is_empty()
        || steps.len() > maximum_steps
        || maximum_steps > MAXIMUM_SEQUENCE_STEPS
        || steps.iter().any(|value| *value > MAXIMUM_CONTROL_LEVEL)
        || lane > 1
        || !matches!(node.config("repeat"), Some("repeat") | Some("stop-at-end"))
    {
        return Err(ResolutionError::new(
            "CND-MSIG-003",
            "sequencer pattern exceeds its finite step or control-level bound",
        ));
    }
    Ok(steps)
}

fn validate_sequencer(node: &Node) -> Result<(), ResolutionError> {
    parse_pattern(node).map(drop)
}

fn validate_slew(node: &Node) -> Result<(), ResolutionError> {
    let initial = integer(node, "initial")?;
    let maximum_delta = integer(node, "maximum_delta")?;
    if node.config.len() == 2
        && initial <= u64::from(MAXIMUM_CONTROL_LEVEL)
        && (1..=u64::from(MAXIMUM_CONTROL_LEVEL)).contains(&maximum_delta)
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-004",
            "slew initial and nonzero maximum delta must fit the control bound",
        ))
    }
}

fn validate_mixer(node: &Node) -> Result<(), ResolutionError> {
    let numerator = integer(node, "gain_numerator")?;
    let denominator = integer(node, "gain_denominator")?;
    let maximum = integer(node, "maximum_value")?;
    if node.config.len() == 3
        && (1..=u64::from(MAXIMUM_CONTROL_LEVEL)).contains(&numerator)
        && (1..=u64::from(MAXIMUM_CONTROL_LEVEL)).contains(&denominator)
        && (1..=u64::from(MAXIMUM_CONTROL_LEVEL)).contains(&maximum)
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-005",
            "mixer gain ratio and maximum value must be finite and nonzero",
        ))
    }
}

fn validate_control_merge(node: &Node) -> Result<(), ResolutionError> {
    if node.config.len() == 1 && node.config("ordering") == Some("round-robin") {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-007",
            "control merge requires explicit round-robin ordering",
        ))
    }
}

fn validate_register(node: &Node) -> Result<(), ResolutionError> {
    let initial = integer(node, "initial")?;
    let maximum = integer(node, "maximum_value")?;
    if node.config.len() == 2
        && maximum > 0
        && maximum <= u64::from(MAXIMUM_CONTROL_LEVEL)
        && initial <= maximum
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-006",
            "register initial value must fit its finite maximum",
        ))
    }
}

fn validate_lfo(node: &Node) -> Result<(), ResolutionError> {
    let minimum = integer(node, "minimum")?;
    let maximum = integer(node, "maximum")?;
    let period = integer(node, "period_ticks")?;
    let phase = integer(node, "startup_phase")?;
    if node.config.len() == 6
        && node.config("shape") == Some("triangle")
        && maximum <= u64::from(MAXIMUM_CONTROL_LEVEL)
        && minimum <= maximum
        && (2..=4096).contains(&period)
        && phase < period
        && node.config("discontinuity") == Some("reset-phase")
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-008",
            "LFO requires a bounded triangle, phase, range, period, and discontinuity policy",
        ))
    }
}

fn validate_envelope(node: &Node) -> Result<(), ResolutionError> {
    let initial = integer(node, "initial")?;
    let peak = integer(node, "peak")?;
    let attack = integer(node, "attack_ticks")?;
    let decay = integer(node, "decay_ticks")?;
    let sustain = integer(node, "sustain")?;
    let release = integer(node, "release_ticks")?;
    let segments = integer(node, "maximum_segments")?;
    if node.config.len() == 8
        && initial <= u64::from(MAXIMUM_CONTROL_LEVEL)
        && peak <= u64::from(MAXIMUM_CONTROL_LEVEL)
        && sustain <= peak
        && attack > 0
        && decay > 0
        && release > 0
        && segments == 4
        && node.config("retrigger") == Some("restart-from-current")
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-008",
            "envelope requires four finite segments and explicit retrigger policy",
        ))
    }
}

fn validate_sample_hold(node: &Node) -> Result<(), ResolutionError> {
    let initial = integer(node, "initial")?;
    let maximum = integer(node, "maximum_value")?;
    if node.config.len() == 3
        && maximum > 0
        && maximum <= u64::from(MAXIMUM_CONTROL_LEVEL)
        && initial <= maximum
        && node.config("before_first_trigger") == Some("emit-initial")
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-008",
            "sample-and-hold requires finite bounds and an explicit pre-trigger value",
        ))
    }
}

fn validate_controlled_gain(node: &Node) -> Result<(), ResolutionError> {
    let frames = integer(node, "maximum_frames")?;
    let work = integer(node, "maximum_work")?;
    if node.config.len() == 4
        && node.config("numeric_profile") == Some(crate::REFERENCE_NUMERIC_PROFILE)
        && node.config("control_mapping") == Some("unipolar-0-1024-to-q15-0-32768")
        && (1..=crate::MAXIMUM_PCM_FRAMES as u64).contains(&frames)
        && (1..=crate::MAXIMUM_AUDIO_WORK as u64).contains(&work)
    {
        Ok(())
    } else {
        Err(ResolutionError::new(
            "CND-MSIG-009",
            "controlled gain requires the exact bounded PCM and control mapping profile",
        ))
    }
}

#[derive(Clone, Copy)]
enum EventKind {
    Trigger,
    Release,
    SelectionMiss,
}

impl EventKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Trigger => 1,
            Self::Release => 2,
            Self::SelectionMiss => 3,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Trigger),
            2 => Some(Self::Release),
            3 => Some(Self::SelectionMiss),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct EventValue {
    tick: u64,
    kind: EventKind,
}

#[derive(Clone, Copy)]
struct GateValue {
    tick: u64,
    held: bool,
    transition: u32,
}

#[derive(Clone, Copy)]
struct ControlValue {
    tick: u64,
    level: u32,
    lane: u8,
}

#[derive(Clone, Copy)]
struct RetainedStateValue {
    tick: u64,
    retained_items: u32,
    retained_bytes: u32,
    level: u32,
}

fn event_value(event: EventValue) -> Value {
    let mut bytes = vec![0_u8; 16];
    bytes[..4].copy_from_slice(b"CME0");
    bytes[4..12].copy_from_slice(&event.tick.to_le_bytes());
    bytes[12] = event.kind.tag();
    Value {
        value_type: EVENT_TYPE,
        bytes,
    }
}

fn gate_value(gate: GateValue) -> Value {
    let mut bytes = vec![0_u8; 20];
    bytes[..4].copy_from_slice(b"CMG0");
    bytes[4..12].copy_from_slice(&gate.tick.to_le_bytes());
    bytes[12] = u8::from(gate.held);
    bytes[16..20].copy_from_slice(&gate.transition.to_le_bytes());
    Value {
        value_type: GATE_TYPE,
        bytes,
    }
}

#[cfg(test)]
fn parse_gate(value: &Value) -> Result<GateValue, RuntimeError> {
    if value.value_type != GATE_TYPE
        || value.bytes.len() != 20
        || !value.bytes.starts_with(b"CMG0")
        || value.bytes[12] > 1
        || value.bytes[13..16] != [0; 3]
    {
        return Err(RuntimeError::new(
            "CND-MSIG-013",
            "gate representation is invalid",
        ));
    }
    Ok(GateValue {
        tick: u64::from_le_bytes(value.bytes[4..12].try_into().expect("gate tick width")),
        held: value.bytes[12] == 1,
        transition: u32::from_le_bytes(
            value.bytes[16..20]
                .try_into()
                .expect("gate transition width"),
        ),
    })
}

fn parse_event(value: &Value) -> Result<EventValue, RuntimeError> {
    if value.value_type != EVENT_TYPE
        || value.bytes.len() != 16
        || !value.bytes.starts_with(b"CME0")
        || EventKind::from_tag(value.bytes[12]).is_none()
        || value.bytes[13..16] != [0; 3]
    {
        return Err(RuntimeError::new(
            "CND-MSIG-010",
            "event representation is invalid",
        ));
    }
    Ok(EventValue {
        tick: u64::from_le_bytes(value.bytes[4..12].try_into().expect("event tick width")),
        kind: EventKind::from_tag(value.bytes[12]).expect("event kind checked"),
    })
}

fn control_value(control: ControlValue) -> Value {
    let mut bytes = vec![0_u8; 16];
    bytes[..4].copy_from_slice(b"CMC0");
    bytes[4..12].copy_from_slice(&control.tick.to_le_bytes());
    let packed = control.level | (u32::from(control.lane) << 16);
    bytes[12..16].copy_from_slice(&packed.to_le_bytes());
    Value {
        value_type: CONTROL_TYPE,
        bytes,
    }
}

fn parse_control(value: &Value) -> Result<ControlValue, RuntimeError> {
    if value.value_type != CONTROL_TYPE
        || value.bytes.len() != 16
        || !value.bytes.starts_with(b"CMC0")
    {
        return Err(RuntimeError::new(
            "CND-MSIG-011",
            "control representation is invalid",
        ));
    }
    let packed = u32::from_le_bytes(
        value.bytes[12..16]
            .try_into()
            .expect("control payload width"),
    );
    let control = ControlValue {
        tick: u64::from_le_bytes(value.bytes[4..12].try_into().expect("control tick width")),
        level: packed & 0xffff,
        lane: u8::try_from(packed >> 16).unwrap_or(u8::MAX),
    };
    if control.level > MAXIMUM_CONTROL_LEVEL || control.lane > 1 {
        return Err(RuntimeError::new(
            "CND-MSIG-011",
            "control level or mixer lane exceeds its semantic bound",
        ));
    }
    Ok(control)
}

fn retained_state_value(state: RetainedStateValue) -> Value {
    let mut bytes = vec![0_u8; 24];
    bytes[..4].copy_from_slice(b"CMS0");
    bytes[4..12].copy_from_slice(&state.tick.to_le_bytes());
    bytes[12..16].copy_from_slice(&state.retained_items.to_le_bytes());
    bytes[16..20].copy_from_slice(&state.retained_bytes.to_le_bytes());
    bytes[20..24].copy_from_slice(&state.level.to_le_bytes());
    Value {
        value_type: RETAINED_STATE_TYPE,
        bytes,
    }
}

#[cfg(test)]
fn parse_retained_state(value: &Value) -> Result<RetainedStateValue, RuntimeError> {
    if value.value_type != RETAINED_STATE_TYPE
        || value.bytes.len() != 24
        || !value.bytes.starts_with(b"CMS0")
    {
        return Err(RuntimeError::new(
            "CND-MSIG-012",
            "retained-state representation is invalid",
        ));
    }
    Ok(RetainedStateValue {
        tick: u64::from_le_bytes(value.bytes[4..12].try_into().expect("state tick width")),
        retained_items: u32::from_le_bytes(
            value.bytes[12..16].try_into().expect("state item width"),
        ),
        retained_bytes: u32::from_le_bytes(
            value.bytes[16..20].try_into().expect("state byte width"),
        ),
        level: u32::from_le_bytes(value.bytes[20..24].try_into().expect("state level width")),
    })
}

fn runtime_integer(node: &Node, key: &str) -> Result<u64, RuntimeError> {
    integer(node, key).map_err(|error| RuntimeError::new(error.code, error.message))
}

struct EventFromTicker;
impl Handler for EventFromTicker {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-010",
                "ticker event input is missing",
            ));
        };
        if input.value_type != TEXT_TYPE {
            return Err(RuntimeError::new(
                "CND-MSIG-010",
                "ticker input is not text",
            ));
        }
        let tick = std::str::from_utf8(&input.bytes)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .ok_or_else(|| RuntimeError::new("CND-MSIG-010", "ticker text is not a u64"))?;
        Ok(HostedServiceStep::produced(vec![event_value(EventValue {
            tick,
            kind: EventKind::Trigger,
        })]))
    }
}

struct ClockDivider;
impl Handler for ClockDivider {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-010",
                "divider event is missing",
            ));
        };
        let input = parse_event(input)?;
        let divisor = runtime_integer(node, "divisor")?;
        let phase = runtime_integer(node, "phase")?;
        Ok(HostedServiceStep::produced(vec![event_value(EventValue {
            tick: input.tick,
            kind: if input.tick % divisor == phase {
                input.kind
            } else {
                EventKind::SelectionMiss
            },
        })]))
    }
}

#[derive(Default)]
struct GateLatch {
    held: bool,
    transition: u32,
}
impl Handler for GateLatch {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-013",
                "gate latch event is missing",
            ));
        };
        let event = parse_event(input)?;
        if matches!(event.kind, EventKind::Trigger) {
            self.held = !self.held;
            self.transition = self.transition.checked_add(1).ok_or_else(|| {
                RuntimeError::new("CND-MSIG-013", "gate transition identity overflow")
            })?;
        }
        Ok(HostedServiceStep::produced(vec![gate_value(GateValue {
            tick: event.tick,
            held: self.held,
            transition: self.transition,
        })]))
    }
}

struct Lfo;
impl Handler for Lfo {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new("CND-MSIG-011", "LFO event is missing"));
        };
        let event = parse_event(input)?;
        let minimum = u32::try_from(runtime_integer(node, "minimum")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "LFO minimum overflow"))?;
        let maximum = u32::try_from(runtime_integer(node, "maximum")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "LFO maximum overflow"))?;
        let period = runtime_integer(node, "period_ticks")?;
        let phase = runtime_integer(node, "startup_phase")?;
        let position = (event.tick + phase) % period;
        let doubled = position.saturating_mul(2);
        let distance = if doubled <= period {
            doubled
        } else {
            period.saturating_mul(2).saturating_sub(doubled)
        };
        let span = u64::from(maximum.saturating_sub(minimum));
        let level = u64::from(minimum).saturating_add(span.saturating_mul(distance) / period);
        Ok(HostedServiceStep::produced(vec![control_value(
            ControlValue {
                tick: event.tick,
                level: u32::try_from(level).unwrap_or(maximum).min(maximum),
                lane: 0,
            },
        )]))
    }
}

#[derive(Default)]
struct Envelope {
    level: Option<u32>,
}
impl Handler for Envelope {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-011",
                "envelope trigger is missing",
            ));
        };
        let event = parse_event(input)?;
        let initial = u32::try_from(runtime_integer(node, "initial")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "envelope initial overflow"))?;
        let peak = u32::try_from(runtime_integer(node, "peak")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "envelope peak overflow"))?;
        let sustain = u32::try_from(runtime_integer(node, "sustain")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "envelope sustain overflow"))?;
        let attack = u32::try_from(runtime_integer(node, "attack_ticks")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "envelope attack overflow"))?;
        let decay = u32::try_from(runtime_integer(node, "decay_ticks")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "envelope decay overflow"))?;
        let release = u32::try_from(runtime_integer(node, "release_ticks")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "envelope release overflow"))?;
        let current = self.level.get_or_insert(initial);
        match event.kind {
            EventKind::Trigger => {
                if *current < peak {
                    let delta = peak.saturating_sub(initial).div_ceil(attack).max(1);
                    *current = current.saturating_add(delta).min(peak);
                } else if *current > sustain {
                    let delta = peak.saturating_sub(sustain).div_ceil(decay).max(1);
                    *current = current.saturating_sub(delta).max(sustain);
                }
            }
            EventKind::Release => {
                let delta = current.saturating_sub(initial).div_ceil(release).max(1);
                *current = current.saturating_sub(delta).max(initial);
            }
            EventKind::SelectionMiss => {}
        }
        Ok(HostedServiceStep::produced(vec![
            control_value(ControlValue {
                tick: event.tick,
                level: *current,
                lane: 0,
            }),
            retained_state_value(RetainedStateValue {
                tick: event.tick,
                retained_items: 1,
                retained_bytes: 4,
                level: *current,
            }),
        ]))
    }
}

#[derive(Default)]
struct SampleHold {
    retained: Option<u32>,
}

struct ControlledGain;
impl Handler for ControlledGain {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [frame, gain] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-009",
                "controlled gain requires audio and control",
            ));
        };
        let input = crate::decode_pcm_chunk(frame).map_err(|reason| {
            RuntimeError::new(
                reason.code(),
                format!("controlled gain input failed: {reason:?}"),
            )
        })?;
        let gain = parse_control(gain)?;
        if input.frames() as u64 > runtime_integer(node, "maximum_frames")?
            || input.samples.len() as u64 > runtime_integer(node, "maximum_work")?
        {
            return Err(RuntimeError::new(
                "CND-MSIG-009",
                "controlled gain exceeded its exact frame or work bound",
            ));
        }
        let q15 = gain.level.saturating_mul(32);
        let output = crate::gain_pcm(&input, q15, q15, input.start_frame, input.start_frame)
            .and_then(|chunk| crate::encode_pcm_chunk(&chunk))
            .map_err(|reason| {
                RuntimeError::new(reason.code(), format!("controlled gain failed: {reason:?}"))
            })?;
        Ok(HostedServiceStep::produced(vec![Value {
            value_type: crate::AUDIO_FRAME_TYPE,
            bytes: output,
        }]))
    }
}
impl Handler for SampleHold {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [control, trigger] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-011",
                "sample-and-hold requires control and trigger",
            ));
        };
        let control = parse_control(control)?;
        let trigger = parse_event(trigger)?;
        let initial = u32::try_from(runtime_integer(node, "initial")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "sample initial overflow"))?;
        let maximum = u32::try_from(runtime_integer(node, "maximum_value")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-008", "sample maximum overflow"))?;
        if matches!(trigger.kind, EventKind::Trigger) {
            self.retained = Some(control.level.min(maximum));
        }
        let level = self.retained.unwrap_or(initial);
        Ok(HostedServiceStep::produced(vec![
            control_value(ControlValue {
                tick: trigger.tick.max(control.tick),
                level,
                lane: control.lane,
            }),
            retained_state_value(RetainedStateValue {
                tick: trigger.tick.max(control.tick),
                retained_items: u32::from(self.retained.is_some()),
                retained_bytes: if self.retained.is_some() { 4 } else { 0 },
                level,
            }),
        ]))
    }
}

struct EventTee;
impl Handler for EventTee {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-010",
                "event tee input is missing",
            ));
        };
        let event = parse_event(input)?;
        Ok(HostedServiceStep::produced(vec![
            event_value(event),
            event_value(event),
        ]))
    }
}

struct ControlTee;
impl Handler for ControlTee {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-011",
                "control tee input is missing",
            ));
        };
        parse_control(input)?;
        Ok(HostedServiceStep::produced(vec![
            input.clone(),
            input.clone(),
        ]))
    }
}

struct Sequencer;
impl Handler for Sequencer {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-010",
                "sequencer event is missing",
            ));
        };
        let event = parse_event(input)?;
        let pattern =
            parse_pattern(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let lane = u8::try_from(runtime_integer(node, "lane")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-003", "sequencer lane overflow"))?;
        let step = event.tick as usize;
        let level = if matches!(event.kind, EventKind::Trigger)
            && (node.config("repeat") == Some("repeat") || step < pattern.len())
        {
            pattern[step % pattern.len()]
        } else {
            0
        };
        Ok(HostedServiceStep::produced(vec![control_value(
            ControlValue {
                tick: event.tick,
                level,
                lane,
            },
        )]))
    }
}

#[derive(Default)]
struct Slew {
    level: Option<u32>,
}
impl Handler for Slew {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new("CND-MSIG-011", "slew control is missing"));
        };
        let input = parse_control(input)?;
        let initial = u32::try_from(runtime_integer(node, "initial")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-004", "slew initial overflow"))?;
        let delta = u32::try_from(runtime_integer(node, "maximum_delta")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-004", "slew delta overflow"))?;
        let current = self.level.get_or_insert(initial);
        *current = if input.level > *current {
            input.level.min(current.saturating_add(delta))
        } else {
            input.level.max(current.saturating_sub(delta))
        };
        Ok(HostedServiceStep::produced(vec![control_value(
            ControlValue {
                tick: input.tick,
                level: *current,
                lane: input.lane,
            },
        )]))
    }
}

#[derive(Default)]
struct Mixer {
    levels: [u32; 2],
}
impl Handler for Mixer {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-011",
                "mixer control is missing",
            ));
        };
        let input = parse_control(input)?;
        self.levels[usize::from(input.lane)] = input.level;
        let numerator = runtime_integer(node, "gain_numerator")?;
        let denominator = runtime_integer(node, "gain_denominator")?;
        let maximum = u32::try_from(runtime_integer(node, "maximum_value")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-005", "mixer maximum overflow"))?;
        let level = u64::from(self.levels[0])
            .checked_add(u64::from(self.levels[1]))
            .ok_or_else(|| RuntimeError::new("CND-MSIG-005", "mixer sum overflow"))?
            .checked_mul(numerator)
            .ok_or_else(|| RuntimeError::new("CND-MSIG-005", "mixer gain overflow"))?
            / denominator;
        Ok(HostedServiceStep::produced(vec![control_value(
            ControlValue {
                tick: input.tick,
                level: u32::try_from(level).unwrap_or(u32::MAX).min(maximum),
                lane: 0,
            },
        )]))
    }
}

#[derive(Default)]
struct Register {
    retained: Option<u32>,
}
impl Handler for Register {
    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-011",
                "register control is missing",
            ));
        };
        let input = parse_control(input)?;
        let initial = u32::try_from(runtime_integer(node, "initial")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-006", "register initial overflow"))?;
        let maximum = u32::try_from(runtime_integer(node, "maximum_value")?)
            .map_err(|_| RuntimeError::new("CND-MSIG-006", "register maximum overflow"))?;
        let output = self.retained.unwrap_or(initial);
        self.retained = Some(input.level.min(maximum));
        Ok(HostedServiceStep::produced(vec![
            control_value(ControlValue {
                tick: input.tick,
                level: output,
                lane: input.lane,
            }),
            retained_state_value(RetainedStateValue {
                tick: input.tick,
                retained_items: 1,
                retained_bytes: 16,
                level: input.level.min(maximum),
            }),
        ]))
    }
}

struct Scope;
impl Handler for Scope {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(RuntimeError::new(
                "CND-MSIG-011",
                "scope control is missing",
            ));
        };
        let input = parse_control(input)?;
        Ok(HostedServiceStep::produced(vec![Value::text(format!(
            "tick={} level={}\n",
            input.tick, input.level
        ))]))
    }
}

struct ControlMergeCompatibility;
impl Handler for ControlMergeCompatibility {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        inputs
            .first()
            .cloned()
            .map(|value| vec![value])
            .ok_or_else(|| RuntimeError::new("CND-MSIG-007", "control merge input is missing"))
    }
}

fn event_from_ticker() -> Box<dyn Handler> {
    Box::new(EventFromTicker)
}
fn clock_divider() -> Box<dyn Handler> {
    Box::new(ClockDivider)
}
fn gate_latch() -> Box<dyn Handler> {
    Box::new(GateLatch::default())
}
fn lfo() -> Box<dyn Handler> {
    Box::new(Lfo)
}
fn envelope() -> Box<dyn Handler> {
    Box::new(Envelope::default())
}
fn sample_hold() -> Box<dyn Handler> {
    Box::new(SampleHold::default())
}
fn controlled_gain() -> Box<dyn Handler> {
    Box::new(ControlledGain)
}
fn event_tee() -> Box<dyn Handler> {
    Box::new(EventTee)
}
fn control_tee() -> Box<dyn Handler> {
    Box::new(ControlTee)
}
fn sequencer() -> Box<dyn Handler> {
    Box::new(Sequencer)
}
fn slew() -> Box<dyn Handler> {
    Box::new(Slew::default())
}
fn mixer() -> Box<dyn Handler> {
    Box::new(Mixer::default())
}
fn control_merge() -> Box<dyn Handler> {
    Box::new(ControlMergeCompatibility)
}
fn register() -> Box<dyn Handler> {
    Box::new(Register::default())
}
fn scope() -> Box<dyn Handler> {
    Box::new(Scope)
}

pub fn register_media_signal_contracts(registry: &mut Registry) {
    for contract in [
        &EVENT_FROM_TICKER_CONTRACT,
        &CLOCK_CONTRACT,
        &TIMER_CONTRACT,
        &COUNTER_CONTRACT,
        &PHASE_SOURCE_CONTRACT,
        &CLOCK_DIVIDER_CONTRACT,
        &EVENT_TEE_CONTRACT,
        &CONTROL_TEE_CONTRACT,
        &TRIGGER_CONTRACT,
        &GATE_LATCH_CONTRACT,
        &GATE_TRIGGER_LATCH_CONTRACT,
        &SEQUENCER_CONTRACT,
        &LFO_CONTRACT,
        &RAMP_CONTRACT,
        &ENVELOPE_CONTRACT,
        &SLEW_CONTRACT,
        &SAMPLE_HOLD_CONTRACT,
        &QUANTIZER_CONTRACT,
        &COMPARATOR_CONTRACT,
        &DEPTH_BIAS_CONTRACT,
        &CLOCKED_SWITCH_CONTRACT,
        &CROSSFADE_CONTRACT,
        &CONTROL_MERGE_CONTRACT,
        &MIXER_CONTRACT,
        &REGISTER_CONTRACT,
        &ONE_TICK_DELAY_CONTRACT,
        &DELAY_LINE_CONTRACT,
        &AUDIO_DELAY_LINE_CONTRACT,
        &ACCUMULATOR_CONTRACT,
        &HISTORY_CONTRACT,
        &FEEDBACK_BOUNDARY_CONTRACT,
        &CONTROLLED_GAIN_CONTRACT,
        &PAN_MATRIX_CONTRACT,
        &SCOPE_CONTRACT,
        &CONTROL_METER_CONTRACT,
        &EVENT_LOG_CONTRACT,
        &WAVEFORM_CONTRACT,
        &SPECTRUM_CONTRACT,
    ] {
        registry.register_contract_only(contract);
    }
}

pub fn register_deterministic_signal_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_media_signal_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    registry.register_compiled_in_host_primitive(
        CompiledInHostService {
            contract: &CONTROL_MERGE_CONTRACT,
            implementation_id: "conduit.media/control-merge-round-robin",
            artifact_id: "conduit.media/control-merge-artifact",
            entrypoint: "media-control-merge",
            source_bytes: include_bytes!("signal.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: control_merge,
            validate_config: validate_control_merge,
        },
        HostedPrimitiveImplementation::ControlMerge,
    )?;
    for (contract, implementation_id, artifact_id, entrypoint, factory, validate_config) in [
        (
            &EVENT_FROM_TICKER_CONTRACT,
            "conduit.media/event-from-ticker-deterministic",
            "conduit.media/event-from-ticker-artifact",
            "media-event-from-ticker",
            event_from_ticker as conduit_runtime::HandlerFactory,
            validate_empty as conduit_runtime::ConfigValidator,
        ),
        (
            &CLOCK_DIVIDER_CONTRACT,
            "conduit.media/clock-divider-deterministic",
            "conduit.media/clock-divider-artifact",
            "media-clock-divider",
            clock_divider as conduit_runtime::HandlerFactory,
            validate_divider as conduit_runtime::ConfigValidator,
        ),
        (
            &GATE_LATCH_CONTRACT,
            "conduit.media/gate-latch-reference",
            "conduit.media/gate-latch-artifact",
            "media-gate-latch",
            gate_latch as conduit_runtime::HandlerFactory,
            validate_empty as conduit_runtime::ConfigValidator,
        ),
        (
            &LFO_CONTRACT,
            "conduit.media/lfo-reference",
            "conduit.media/lfo-reference-artifact",
            "media-lfo",
            lfo as conduit_runtime::HandlerFactory,
            validate_lfo as conduit_runtime::ConfigValidator,
        ),
        (
            &ENVELOPE_CONTRACT,
            "conduit.media/envelope-reference",
            "conduit.media/envelope-reference-artifact",
            "media-envelope",
            envelope as conduit_runtime::HandlerFactory,
            validate_envelope as conduit_runtime::ConfigValidator,
        ),
        (
            &SAMPLE_HOLD_CONTRACT,
            "conduit.media/sample-hold-reference",
            "conduit.media/sample-hold-reference-artifact",
            "media-sample-hold",
            sample_hold as conduit_runtime::HandlerFactory,
            validate_sample_hold as conduit_runtime::ConfigValidator,
        ),
        (
            &CONTROLLED_GAIN_CONTRACT,
            "conduit.media/controlled-gain-reference",
            "conduit.media/controlled-gain-reference-artifact",
            "media-controlled-gain",
            controlled_gain as conduit_runtime::HandlerFactory,
            validate_controlled_gain as conduit_runtime::ConfigValidator,
        ),
        (
            &EVENT_TEE_CONTRACT,
            "conduit.media/event-tee-deterministic",
            "conduit.media/event-tee-artifact",
            "media-event-tee",
            event_tee as conduit_runtime::HandlerFactory,
            validate_empty as conduit_runtime::ConfigValidator,
        ),
        (
            &CONTROL_TEE_CONTRACT,
            "conduit.media/control-tee-reference",
            "conduit.media/control-tee-reference-artifact",
            "media-control-tee",
            control_tee as conduit_runtime::HandlerFactory,
            validate_empty as conduit_runtime::ConfigValidator,
        ),
        (
            &SEQUENCER_CONTRACT,
            "conduit.media/sequencer-deterministic",
            "conduit.media/sequencer-artifact",
            "media-sequencer",
            sequencer as conduit_runtime::HandlerFactory,
            validate_sequencer as conduit_runtime::ConfigValidator,
        ),
        (
            &SLEW_CONTRACT,
            "conduit.media/slew-deterministic",
            "conduit.media/slew-artifact",
            "media-slew",
            slew as conduit_runtime::HandlerFactory,
            validate_slew as conduit_runtime::ConfigValidator,
        ),
        (
            &MIXER_CONTRACT,
            "conduit.media/control-mixer-deterministic",
            "conduit.media/control-mixer-artifact",
            "media-control-mixer",
            mixer as conduit_runtime::HandlerFactory,
            validate_mixer as conduit_runtime::ConfigValidator,
        ),
        (
            &REGISTER_CONTRACT,
            "conduit.media/control-register-deterministic",
            "conduit.media/control-register-artifact",
            "media-control-register",
            register as conduit_runtime::HandlerFactory,
            validate_register as conduit_runtime::ConfigValidator,
        ),
        (
            &SCOPE_CONTRACT,
            "conduit.media/control-scope-deterministic",
            "conduit.media/control-scope-artifact",
            "media-control-scope",
            scope as conduit_runtime::HandlerFactory,
            validate_empty as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("signal.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory,
            validate_config,
        })?;
    }
    Ok(())
}

/// Registers a second, separately identified implementation of the exact
/// triangle-LFO profile. Callers first install the signal contract catalog (or
/// the deterministic reference providers); provider selection remains an
/// explicit compile/host fact.
pub fn register_portable_lfo_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    const SOURCE: &[u8] = b"conduit.media/lfo-portable-integer|triangle|u64-rational|0";
    const PROFILE: &str = "conduit/media-portable-integer-lfo-profile";
    let digest = ArtifactDigest::from_bytes(Sha256::digest(SOURCE).into());
    registry.register_installed_implementation(InstalledImplementationRegistration {
        contract: &LFO_CONTRACT,
        implementation_id: "conduit.media/lfo-portable-integer".to_owned(),
        implementation_version: "triangle-u64-rational-0".to_owned(),
        executor: ExecutorKind::NativeInProcess,
        entrypoint_name: "media-lfo-portable-integer".to_owned(),
        entrypoint_adapter: "conduit/rust-native-in-process".to_owned(),
        entrypoint_abi: "conduit/rust-handler".to_owned(),
        entrypoint_protocol_version: 0,
        execution_profile: PinnedDescriptor {
            id: Id(PROFILE),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes(Sha256::digest(PROFILE).into()),
        },
        artifacts: vec![InstalledArtifactRegistration {
            id: "conduit.media/lfo-portable-integer-artifact".to_owned(),
            digest,
            media_type: "application/vnd.conduit.compiled-in-provider".to_owned(),
            byte_size: SOURCE.len() as u64,
            target: Some(std::env::consts::ARCH.to_owned()),
            abi: Some("conduit/rust-handler".to_owned()),
            builder: "conduit/rustc-workspace-build".to_owned(),
            source_digest: digest,
            build_recipe_digest: digest,
            reproducible: true,
            license_expressions: Vec::new(),
            role: "executable".to_owned(),
            required: true,
        }],
        required_authorities: Vec::new(),
        required_effects: Vec::new(),
        minimum_plan_version: 0,
        maximum_plan_version: u32::MAX,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        coexistence_memory_bytes: 0,
        managed_lifecycle: None,
        factory: lfo,
        validate_config: validate_lfo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(source: &str) -> Node {
        let mut panel = conduit_panel::parse(&format!("panel 0\nsignal: {source}\n"))
            .expect("signal source parses");
        panel.nodes.remove(0)
    }

    #[test]
    fn signal_types_are_semantically_distinct() {
        assert_ne!(EVENT_TYPE, GATE_TYPE);
        assert_ne!(EVENT_TYPE, CONTROL_TYPE);
        assert_ne!(CONTROL_TYPE, crate::AUDIO_FRAME_TYPE);
        assert_ne!(RETAINED_STATE_TYPE, CONTROL_TYPE);
        assert_eq!(CLOCK_CONTRACT.inputs[0].value_type, EVENT_TYPE);
        assert_eq!(CLOCK_CONTRACT.inputs[1].value_type, GATE_TYPE);
        assert_eq!(CLOCK_CONTRACT.inputs[2].value_type, CONTROL_TYPE);
        assert_eq!(CLOCK_CONTRACT.outputs[4].value_type, RETAINED_STATE_TYPE);
        assert_eq!(
            CONTROLLED_GAIN_CONTRACT.inputs[0].value_type,
            crate::AUDIO_FRAME_TYPE
        );
        assert_eq!(CONTROLLED_GAIN_CONTRACT.inputs[1].value_type, CONTROL_TYPE);
        let clock_fields = CLOCK_CONTRACT
            .config
            .fields
            .iter()
            .map(|field| field.key.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for required in [
            "time_basis",
            "period_ticks",
            "startup_phase",
            "reset_phase",
            "rate_mapping",
            "enable_behavior",
            "drift",
            "discontinuity",
            "missed_pulse",
            "maximum_pending",
            "pressure",
        ] {
            assert!(
                clock_fields.contains(required),
                "clock publishes {required}"
            );
        }
    }

    #[test]
    fn clock_start_reset_enable_rate_missed_and_discontinuity_cases_are_exact() {
        let mut clock = DeterministicClockState::new(4, 1, 2).unwrap();
        assert_eq!(
            clock.advance(1, 2, MissedPulsePolicy::Fail).unwrap(),
            ClockAdvance {
                pulses: vec![1],
                dropped: 0,
                discontinuity: false,
            }
        );
        clock.set_enabled(2, false).unwrap();
        assert!(
            clock
                .advance(9, 2, MissedPulsePolicy::Fail)
                .unwrap()
                .pulses
                .is_empty()
        );
        clock.set_enabled(9, true).unwrap();
        assert_eq!(
            clock
                .advance(13, 2, MissedPulsePolicy::Fail)
                .unwrap()
                .pulses,
            vec![13]
        );
        clock.reset(14).unwrap();
        let reset = clock.advance(16, 2, MissedPulsePolicy::Fail).unwrap();
        assert_eq!(reset.pulses, vec![16]);
        assert!(reset.discontinuity);
        clock.change_period(16, 2).unwrap();
        let rate = clock.advance(18, 2, MissedPulsePolicy::Fail).unwrap();
        assert_eq!(rate.pulses, vec![18]);
        assert!(rate.discontinuity);
        let missed = clock
            .advance(26, 2, MissedPulsePolicy::DropWithCount)
            .unwrap();
        assert_eq!(missed.pulses, vec![20, 22]);
        assert_eq!(missed.dropped, 2);
        assert_eq!(
            clock.advance(25, 2, MissedPulsePolicy::Fail),
            Err(ClockAdvanceReason::TimeReversal)
        );

        let mut coalesced = DeterministicClockState::new(2, 0, 0).unwrap();
        let slow = coalesced
            .advance(8, 1, MissedPulsePolicy::Coalesce)
            .unwrap();
        assert_eq!(slow.pulses, vec![0]);
        assert_eq!(slow.dropped, 4);
        let mut failed = DeterministicClockState::new(2, 0, 0).unwrap();
        assert_eq!(
            failed.advance(8, 1, MissedPulsePolicy::Fail),
            Err(ClockAdvanceReason::MissedPulse)
        );
        assert_eq!(bounded_counter_next(3, 4), Ok(0));
        assert_eq!(
            bounded_counter_next(4, 4),
            Err(ClockAdvanceReason::InvalidConfiguration)
        );
    }

    #[test]
    fn finite_signal_configuration_fails_closed() {
        assert!(
            validate_divider(&node(
                "conduit.media/control/clock-divider { divisor = 4 phase = 0 }"
            ))
            .is_ok()
        );
        assert_eq!(
            validate_divider(&node(
                "conduit.media/control/clock-divider { divisor = 0 phase = 0 }"
            ))
            .expect_err("zero divisor is rejected")
            .code,
            "CND-MSIG-002"
        );
        assert!(
            validate_sequencer(&node(
                "conduit.media/control/sequencer { pattern = \"0,1024,512,256\" maximum_steps = 4 lane = 0 repeat = \"repeat\" }"
            ))
            .is_ok()
        );
        assert!(
            validate_mixer(&node(
                "conduit.media/control/mixer { gain_numerator = 1 gain_denominator = 2 maximum_value = 1024 }"
            ))
            .is_ok()
        );
        assert_eq!(
            validate_mixer(&node(
                "conduit.media/control/mixer { gain_numerator = 1 gain_denominator = 0 maximum_value = 1024 }"
            ))
            .expect_err("zero gain denominator is rejected")
            .code,
            "CND-MSIG-005"
        );
        assert!(
            validate_lfo(&node(
                "conduit.media/control/lfo { shape = \"triangle\" minimum = 0 maximum = 1024 period_ticks = 8 startup_phase = 0 discontinuity = \"reset-phase\" }"
            ))
            .is_ok()
        );
        assert!(
            validate_envelope(&node(
                "conduit.media/control/envelope { initial = 0 peak = 1024 attack_ticks = 2 decay_ticks = 2 sustain = 512 release_ticks = 2 retrigger = \"restart-from-current\" maximum_segments = 4 }"
            ))
            .is_ok()
        );
        assert!(
            validate_sample_hold(&node(
                "conduit.media/control/sample-hold { initial = 0 maximum_value = 1024 before_first_trigger = \"emit-initial\" }"
            ))
            .is_ok()
        );
        assert!(
            validate_controlled_gain(&node(
                "conduit.media/audio/controlled-gain { numeric_profile = \"pcm-s16-q15-round-nearest-away-saturate-no-nan-no-denormal-bit-exact\" control_mapping = \"unipolar-0-1024-to-q15-0-32768\" maximum_frames = 16 maximum_work = 32 }"
            ))
            .is_ok()
        );
    }

    #[test]
    fn deterministic_signal_handlers_preserve_ticks_and_bounds() {
        let mut input = std::io::empty();
        let mut output = Vec::new();
        let mut error = Vec::new();
        let mut display = Vec::new();
        let mut io = RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        let context = HostedServiceStepContext { tick: 0 };
        let event = match EventFromTicker
            .step(
                &node("conduit.media/event/from-ticker"),
                &[Value::text("5\n")],
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => outputs[0].clone(),
            _ => panic!("adapter produces one event"),
        };
        let control = match Sequencer
            .step(
                &node(
                    "conduit.media/control/sequencer { pattern = \"0,1024,512,256\" maximum_steps = 4 lane = 0 repeat = \"repeat\" }",
                ),
                &[event],
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => parse_control(&outputs[0]).unwrap(),
            _ => panic!("sequencer produces one control"),
        };
        assert_eq!(control.tick, 5);
        assert_eq!(control.level, 1024);
        let stopped = match Sequencer
            .step(
                &node(
                    "conduit.media/control/sequencer { pattern = \"0,1024,512,256\" maximum_steps = 4 lane = 0 repeat = \"stop-at-end\" }",
                ),
                &[event_value(EventValue {
                    tick: 5,
                    kind: EventKind::Trigger,
                })],
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => parse_control(&outputs[0]).unwrap(),
            _ => panic!("finite sequencer publishes its stopped level"),
        };
        assert_eq!(stopped.level, 0);

        let mut mixer = Mixer::default();
        let first_mix = match mixer
            .step(
                &node(
                    "conduit.media/control/mixer { gain_numerator = 1 gain_denominator = 2 maximum_value = 1024 }",
                ),
                &[control_value(ControlValue {
                    tick: 7,
                    level: 1024,
                    lane: 0,
                })],
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => parse_control(&outputs[0]).unwrap(),
            _ => panic!("mixer produces the first bounded control"),
        };
        assert_eq!(first_mix.tick, 7);
        assert_eq!(first_mix.level, 512);

        let mixed = match mixer
            .step(
                &node(
                    "conduit.media/control/mixer { gain_numerator = 1 gain_denominator = 2 maximum_value = 1024 }",
                ),
                &[control_value(ControlValue {
                    tick: 8,
                    level: 512,
                    lane: 1,
                })],
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => parse_control(&outputs[0]).unwrap(),
            _ => panic!("mixer produces one bounded control"),
        };
        assert_eq!(mixed.tick, 8);
        assert_eq!(mixed.level, 768);
    }

    #[test]
    fn gate_envelope_and_sample_hold_publish_exact_retained_state() {
        let mut input = std::io::empty();
        let mut output = Vec::new();
        let mut error = Vec::new();
        let mut display = Vec::new();
        let mut io = RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        let context = HostedServiceStepContext { tick: 0 };
        let trigger = event_value(EventValue {
            tick: 3,
            kind: EventKind::Trigger,
        });

        let gate = match GateLatch::default()
            .step(
                &node("conduit.media/gate/latch"),
                std::slice::from_ref(&trigger),
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => parse_gate(&outputs[0]).unwrap(),
            _ => panic!("gate latch produces held state"),
        };
        assert!(gate.held);
        assert_eq!(gate.transition, 1);

        let mut envelope_provider = Envelope::default();
        let envelope = match envelope_provider
            .step(
                &node(
                    "conduit.media/control/envelope { initial = 0 peak = 1024 attack_ticks = 2 decay_ticks = 2 sustain = 512 release_ticks = 2 retrigger = \"restart-from-current\" maximum_segments = 4 }",
                ),
                std::slice::from_ref(&trigger),
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => outputs,
            _ => panic!("envelope produces control and state"),
        };
        assert_eq!(parse_control(&envelope[0]).unwrap().level, 512);
        let state = parse_retained_state(&envelope[1]).unwrap();
        assert_eq!(state.retained_items, 1);
        assert_eq!(state.retained_bytes, 4);
        assert_eq!(state.level, 512);
        let retriggered = match envelope_provider
            .step(
                &node(
                    "conduit.media/control/envelope { initial = 0 peak = 1024 attack_ticks = 2 decay_ticks = 2 sustain = 512 release_ticks = 2 retrigger = \"restart-from-current\" maximum_segments = 4 }",
                ),
                &[event_value(EventValue {
                    tick: 4,
                    kind: EventKind::Trigger,
                })],
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => outputs,
            _ => panic!("envelope retrigger remains bounded"),
        };
        assert_eq!(parse_control(&retriggered[0]).unwrap().level, 1024);

        let control = control_value(ControlValue {
            tick: 3,
            level: 768,
            lane: 0,
        });
        let release = event_value(EventValue {
            tick: 3,
            kind: EventKind::Release,
        });
        let mut sample = SampleHold::default();
        let before = match sample
            .step(
                &node(
                    "conduit.media/control/sample-hold { initial = 64 maximum_value = 1024 before_first_trigger = \"emit-initial\" }",
                ),
                &[control.clone(), release],
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => outputs,
            _ => panic!("sample-and-hold produces initial state"),
        };
        assert_eq!(parse_control(&before[0]).unwrap().level, 64);
        assert_eq!(parse_retained_state(&before[1]).unwrap().retained_items, 0);

        let held = match sample
            .step(
                &node(
                    "conduit.media/control/sample-hold { initial = 64 maximum_value = 1024 before_first_trigger = \"emit-initial\" }",
                ),
                &[control, trigger],
                context,
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => outputs,
            _ => panic!("sample-and-hold captures on the explicit trigger"),
        };
        assert_eq!(parse_control(&held[0]).unwrap().level, 768);
        assert_eq!(parse_retained_state(&held[1]).unwrap().retained_items, 1);
    }

    #[test]
    fn live_control_drives_audio_gain_without_mutating_prestart_configuration() {
        let mut input = std::io::empty();
        let mut output = Vec::new();
        let mut error = Vec::new();
        let mut display = Vec::new();
        let mut io = RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        let source = Value {
            value_type: crate::AUDIO_FRAME_TYPE,
            bytes: crate::AUDIO_VALUE.to_vec(),
        };
        let gain = control_value(ControlValue {
            tick: 4,
            level: 512,
            lane: 0,
        });
        let outputs = match ControlledGain
            .step(
                &node(
                    "conduit.media/audio/controlled-gain { numeric_profile = \"pcm-s16-q15-round-nearest-away-saturate-no-nan-no-denormal-bit-exact\" control_mapping = \"unipolar-0-1024-to-q15-0-32768\" maximum_frames = 16 maximum_work = 32 }",
                ),
                &[source, gain],
                HostedServiceStepContext { tick: 4 },
                &mut io,
            )
            .unwrap()
        {
            HostedServiceStep::Produced { outputs } => outputs,
            _ => panic!("standing controlled gain remains live"),
        };
        let chunk = crate::decode_pcm_chunk(&outputs[0]).unwrap();
        assert_eq!(chunk.samples[0], 6_000);
        assert_eq!(chunk.samples[2], -6_000);
    }
}
