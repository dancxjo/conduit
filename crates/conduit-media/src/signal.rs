//! Bounded standing-patch signal contracts and deterministic reference providers.
//!
//! Events, gates, controls, and audio frames remain different value contracts.
//! Nothing here starts a run, chooses a host clock, or grants device authority.

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, HostedPrimitiveImplementation, HostedServiceStep,
    HostedServiceStepContext, Registry, RegistryError, ResolutionError, RunIo, RuntimeError, Value,
};

pub const MAXIMUM_CONTROL_LEVEL: u32 = 1024;
pub const MAXIMUM_SEQUENCE_STEPS: usize = 16;

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
const CONTROL_OUTPUT: [PortContract<'static>; 1] =
    [stream_port("control", Direction::Output, CONTROL_TYPE)];
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
pub const REGISTER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/register"),
    config: REGISTER_CONFIG,
    inputs: &CONTROL_INPUT,
    outputs: &CONTROL_OUTPUT,
};
pub const SCOPE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit.media/control/scope"),
    config: ConfigContract { fields: &[] },
    inputs: &CONTROL_INPUT,
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
    if node.config.len() != 3
        || steps.is_empty()
        || steps.len() > maximum_steps
        || maximum_steps > MAXIMUM_SEQUENCE_STEPS
        || steps.iter().any(|value| *value > MAXIMUM_CONTROL_LEVEL)
        || lane > 1
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

#[derive(Clone, Copy)]
struct EventValue {
    tick: u64,
    active: bool,
}

#[derive(Clone, Copy)]
struct ControlValue {
    tick: u64,
    level: u32,
    lane: u8,
}

fn event_value(event: EventValue) -> Value {
    let mut bytes = vec![0_u8; 16];
    bytes[..4].copy_from_slice(b"CME0");
    bytes[4..12].copy_from_slice(&event.tick.to_le_bytes());
    bytes[12] = u8::from(event.active);
    Value {
        value_type: EVENT_TYPE,
        bytes,
    }
}

fn parse_event(value: &Value) -> Result<EventValue, RuntimeError> {
    if value.value_type != EVENT_TYPE
        || value.bytes.len() != 16
        || !value.bytes.starts_with(b"CME0")
        || value.bytes[12] > 1
    {
        return Err(RuntimeError::new(
            "CND-MSIG-010",
            "event representation is invalid",
        ));
    }
    Ok(EventValue {
        tick: u64::from_le_bytes(value.bytes[4..12].try_into().expect("event tick width")),
        active: value.bytes[12] == 1,
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
            active: true,
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
            active: input.active && input.tick % divisor == phase,
        })]))
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
        let level = if event.active {
            pattern[event.tick as usize % pattern.len()]
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
        Ok(HostedServiceStep::produced(vec![control_value(
            ControlValue {
                tick: input.tick,
                level: output,
                lane: input.lane,
            },
        )]))
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
fn event_tee() -> Box<dyn Handler> {
    Box::new(EventTee)
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
        &CLOCK_DIVIDER_CONTRACT,
        &EVENT_TEE_CONTRACT,
        &SEQUENCER_CONTRACT,
        &SLEW_CONTRACT,
        &CONTROL_MERGE_CONTRACT,
        &MIXER_CONTRACT,
        &REGISTER_CONTRACT,
        &SCOPE_CONTRACT,
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
            &EVENT_TEE_CONTRACT,
            "conduit.media/event-tee-deterministic",
            "conduit.media/event-tee-artifact",
            "media-event-tee",
            event_tee as conduit_runtime::HandlerFactory,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn node(source: &str) -> Node {
        let mut panel = conduit_panel::parse(&format!("panel 0\nnode signal : {source}\n"))
            .expect("signal source parses");
        panel.nodes.remove(0)
    }

    #[test]
    fn signal_types_are_semantically_distinct() {
        assert_ne!(EVENT_TYPE, GATE_TYPE);
        assert_ne!(EVENT_TYPE, CONTROL_TYPE);
        assert_ne!(CONTROL_TYPE, crate::AUDIO_FRAME_TYPE);
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
                "conduit.media/control/sequencer { pattern = \"0,1024,512,256\" maximum_steps = 4 lane = 0 }"
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
                    "conduit.media/control/sequencer { pattern = \"0,1024,512,256\" maximum_steps = 4 lane = 0 }",
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
}
