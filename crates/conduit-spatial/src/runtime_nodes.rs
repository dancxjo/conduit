use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};

use crate::{
    AxisConvention, FrameIdentity, Handedness, LinearUnit, NumericProfile, PinholeCalibration,
    PixelPoint, QuaternionQ30, SpatialReason, StampedPoint3, Transform3, Uncertainty, Validity,
    apply_transform, compose, interpolate, invert, lookup_transform, project, unproject,
};

pub const CALIBRATION_IDENTITY_TEXT: &str =
    "sha256:5151515151515151515151515151515151515151515151515151515151515151";
pub const PROVENANCE_IDENTITY_TEXT: &str =
    "sha256:5252525252525252525252525252525252525252525252525252525252525252";
pub const TRANSFORM_DESCRIPTOR: &str = "conduit.spatial/transform3|0|source,target,um,right-handed,x-right-y-forward-z-up,q30,clock,validity,uncertainty,calibration,provenance|finite";
pub const POINT_DESCRIPTOR: &str =
    "conduit.spatial/stamped-point3|0|frame,um,clock,uncertainty,provenance|finite";
pub const PIXEL_DESCRIPTOR: &str =
    "conduit.spatial/pixel-point|0|frame,millipixel,depth-um,clock,calibration|finite";

pub const TRANSFORM_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("spatial/transform3"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x1c, 0xba, 0xea, 0xf1, 0xec, 0x22, 0x1c, 0x4f, 0xac, 0x1a, 0x92, 0xf1, 0xee, 0x31, 0xe6,
        0x0b, 0x79, 0x04, 0x0c, 0xe2, 0x08, 0x9d, 0x38, 0x77, 0x84, 0xf8, 0x50, 0x8f, 0x9b, 0xdb,
        0x9d, 0x67,
    ]),
};
pub const POINT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("spatial/stamped-point3"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xe8, 0x71, 0x4f, 0x34, 0x9b, 0xfd, 0xb0, 0xad, 0x69, 0x58, 0x66, 0x2a, 0x5a, 0x15, 0x59,
        0x35, 0x05, 0xc9, 0x48, 0x75, 0x8b, 0x82, 0x23, 0x84, 0x12, 0xb1, 0x59, 0x9b, 0x8c, 0x6f,
        0x79, 0xa3,
    ]),
};
pub const PIXEL_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("spatial/pixel-point"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x61, 0x0b, 0xd4, 0x9f, 0xff, 0x1d, 0x33, 0x15, 0xb4, 0xd9, 0x62, 0x1f, 0x42, 0xab, 0x1d,
        0xfd, 0x0e, 0xe3, 0xdf, 0x1e, 0x3b, 0x76, 0x95, 0xba, 0x94, 0xd1, 0xf3, 0xc1, 0xf1, 0xb6,
        0x7e, 0x66,
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
const fn field(
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

const TRANSFORM_FIELDS: [ConfigFieldContract<'static>; 17] = [
    field("source_frame", TEXT_TYPE),
    field("target_frame", TEXT_TYPE),
    field("unit", TEXT_TYPE),
    field("handedness", TEXT_TYPE),
    field("axes", TEXT_TYPE),
    field("translation_x_um", U64_TYPE),
    field("translation_y_um", U64_TYPE),
    field("translation_z_um", U64_TYPE),
    field("quarter_turns_z", U64_TYPE),
    field("clock", TEXT_TYPE),
    field("stamp_tick", U64_TYPE),
    field("valid_from_tick", U64_TYPE),
    field("valid_until_tick", U64_TYPE),
    field("uncertainty_um", U64_TYPE),
    field("calibration_identity", TEXT_TYPE),
    field("provenance_identity", TEXT_TYPE),
    field("maximum_output_bytes", U64_TYPE),
];
const POINT_FIELDS: [ConfigFieldContract<'static>; 11] = [
    field("frame", TEXT_TYPE),
    field("x_um", U64_TYPE),
    field("y_um", U64_TYPE),
    field("z_um", U64_TYPE),
    field("clock", TEXT_TYPE),
    field("tick", U64_TYPE),
    field("uncertainty_um", U64_TYPE),
    field("provenance_identity", TEXT_TYPE),
    field("unit", TEXT_TYPE),
    field("axes", TEXT_TYPE),
    field("maximum_output_bytes", U64_TYPE),
];
const OP_FIELDS: [ConfigFieldContract<'static>; 2] = [
    field("maximum_uncertainty_um", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const INTERPOLATE_FIELDS: [ConfigFieldContract<'static>; 5] = [
    field("tick", U64_TYPE),
    field("maximum_window_ticks", U64_TYPE),
    field("maximum_history_values", U64_TYPE),
    field("maximum_uncertainty_um", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const LOOKUP_FIELDS: [ConfigFieldContract<'static>; 5] = [
    field("source_frame", TEXT_TYPE),
    field("target_frame", TEXT_TYPE),
    field("maximum_edges", U64_TYPE),
    field("maximum_uncertainty_um", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const CALIBRATION_FIELDS: [ConfigFieldContract<'static>; 11] = [
    field("camera_frame", TEXT_TYPE),
    field("calibration_identity", TEXT_TYPE),
    field("fx_millipixel", U64_TYPE),
    field("fy_millipixel", U64_TYPE),
    field("cx_millipixel", U64_TYPE),
    field("cy_millipixel", U64_TYPE),
    field("width_pixels", U64_TYPE),
    field("height_pixels", U64_TYPE),
    field("valid_until_tick", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("maximum_output_bytes", U64_TYPE),
];

const fn port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    connections: ConnectionCardinality,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections,
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const TRANSFORM_OUTPUT: [PortContract<'static>; 1] = [port(
    "transform",
    Direction::Output,
    TRANSFORM_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const POINT_OUTPUT: [PortContract<'static>; 1] = [port(
    "point",
    Direction::Output,
    POINT_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const PIXEL_OUTPUT: [PortContract<'static>; 1] = [port(
    "pixel",
    Direction::Output,
    PIXEL_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const TRANSFORM_INPUT: [PortContract<'static>; 1] = [port(
    "transform",
    Direction::Input,
    TRANSFORM_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const COMPOSE_INPUTS: [PortContract<'static>; 2] = [
    port(
        "first",
        Direction::Input,
        TRANSFORM_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    port(
        "second",
        Direction::Input,
        TRANSFORM_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const INTERPOLATE_INPUTS: [PortContract<'static>; 2] = [
    port(
        "before",
        Direction::Input,
        TRANSFORM_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    port(
        "after",
        Direction::Input,
        TRANSFORM_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const APPLY_INPUTS: [PortContract<'static>; 2] = [
    port(
        "transform",
        Direction::Input,
        TRANSFORM_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    port(
        "point",
        Direction::Input,
        POINT_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const POINT_INPUT: [PortContract<'static>; 1] = [port(
    "point",
    Direction::Input,
    POINT_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const PIXEL_INPUT: [PortContract<'static>; 1] = [port(
    "pixel",
    Direction::Input,
    PIXEL_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const SUMMARY_OUTPUT: [PortContract<'static>; 1] = [PortContract {
    id: Id("summary"),
    direction: Direction::Output,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::OneOrMore,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
}];

pub const TRANSFORM_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/transform/literal"),
    config: ConfigContract {
        fields: &TRANSFORM_FIELDS,
    },
    inputs: &[],
    outputs: &TRANSFORM_OUTPUT,
};
pub const POINT_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/point/literal"),
    config: ConfigContract {
        fields: &POINT_FIELDS,
    },
    inputs: &[],
    outputs: &POINT_OUTPUT,
};
pub const COMPOSE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/transform/compose"),
    config: ConfigContract { fields: &OP_FIELDS },
    inputs: &COMPOSE_INPUTS,
    outputs: &TRANSFORM_OUTPUT,
};
pub const LOOKUP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/transform/lookup"),
    config: ConfigContract {
        fields: &LOOKUP_FIELDS,
    },
    inputs: &COMPOSE_INPUTS,
    outputs: &TRANSFORM_OUTPUT,
};
pub const INVERT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/transform/invert"),
    config: ConfigContract { fields: &OP_FIELDS },
    inputs: &TRANSFORM_INPUT,
    outputs: &TRANSFORM_OUTPUT,
};
pub const INTERPOLATE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/transform/interpolate"),
    config: ConfigContract {
        fields: &INTERPOLATE_FIELDS,
    },
    inputs: &INTERPOLATE_INPUTS,
    outputs: &TRANSFORM_OUTPUT,
};
pub const APPLY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/transform/apply"),
    config: ConfigContract { fields: &OP_FIELDS },
    inputs: &APPLY_INPUTS,
    outputs: &POINT_OUTPUT,
};
pub const PROJECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/camera/project"),
    config: ConfigContract {
        fields: &CALIBRATION_FIELDS,
    },
    inputs: &POINT_INPUT,
    outputs: &PIXEL_OUTPUT,
};
pub const UNPROJECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/camera/unproject"),
    config: ConfigContract {
        fields: &CALIBRATION_FIELDS,
    },
    inputs: &PIXEL_INPUT,
    outputs: &POINT_OUTPUT,
};
pub const POINT_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/point/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &POINT_INPUT,
    outputs: &SUMMARY_OUTPUT,
};

pub const SPATIAL_CONTRACTS: [&NodeContract<'static>; 10] = [
    &TRANSFORM_LITERAL_CONTRACT,
    &POINT_LITERAL_CONTRACT,
    &COMPOSE_CONTRACT,
    &LOOKUP_CONTRACT,
    &INVERT_CONTRACT,
    &INTERPOLATE_CONTRACT,
    &APPLY_CONTRACT,
    &PROJECT_CONTRACT,
    &UNPROJECT_CONTRACT,
    &POINT_INSPECT_CONTRACT,
];

const MAXIMUM_VALUE_BYTES: usize = 256;

fn runtime(reason: SpatialReason) -> RuntimeError {
    RuntimeError::new(
        reason.code(),
        format!("bounded spatial operation failed: {reason:?}"),
    )
}

fn resolution(reason: SpatialReason, message: &'static str) -> ResolutionError {
    ResolutionError::new(reason.code(), message)
}

fn text<'a>(node: &'a Node, key: &str) -> Result<&'a str, SpatialReason> {
    node.config(key).ok_or(SpatialReason::WrongFrame)
}

fn integer(node: &Node, key: &str) -> Result<i64, SpatialReason> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => {
            i64::try_from(*value).map_err(|_| SpatialReason::NumericOverflow)
        }
        _ => Err(SpatialReason::NumericOverflow),
    }
}

fn unsigned(node: &Node, key: &str) -> Result<u64, SpatialReason> {
    u64::try_from(integer(node, key)?).map_err(|_| SpatialReason::NumericOverflow)
}

fn exact_identity(value: &str) -> Result<[u8; 32], SpatialReason> {
    if value == CALIBRATION_IDENTITY_TEXT {
        Ok([0x51; 32])
    } else if value == PROVENANCE_IDENTITY_TEXT {
        Ok([0x52; 32])
    } else {
        Err(SpatialReason::CalibrationMismatch)
    }
}

fn configured_frame(node: &Node, key: &str) -> Result<FrameIdentity, SpatialReason> {
    let mut frame = FrameIdentity::new(text(node, key)?)?;
    frame.unit = match text(node, "unit")? {
        "um" => LinearUnit::Micrometre,
        "mm" => LinearUnit::Millimetre,
        _ => return Err(SpatialReason::UnitMismatch),
    };
    frame.handedness = match text(node, "handedness")? {
        "right" => Handedness::Right,
        "left" => Handedness::Left,
        _ => return Err(SpatialReason::HandednessMismatch),
    };
    frame.axes = match text(node, "axes")? {
        "x-right-y-forward-z-up" => AxisConvention::XRightYForwardZUp,
        "x-forward-y-left-z-up" => AxisConvention::XForwardYLeftZUp,
        _ => return Err(SpatialReason::AxisMismatch),
    };
    Ok(frame)
}

fn transform_from_node(node: &Node) -> Result<Transform3, SpatialReason> {
    let source = configured_frame(node, "source_frame")?;
    let target = configured_frame(node, "target_frame")?;
    let turns = u8::try_from(unsigned(node, "quarter_turns_z")?)
        .map_err(|_| SpatialReason::InvalidQuaternion)?;
    let transform = Transform3 {
        source,
        target,
        translation_um: [
            integer(node, "translation_x_um")?,
            integer(node, "translation_y_um")?,
            integer(node, "translation_z_um")?,
        ],
        rotation: QuaternionQ30::quarter_turn_z(turns).ok_or(SpatialReason::InvalidQuaternion)?,
        quarter_turns_z: turns,
        validity: Validity {
            clock: text(node, "clock")?.to_owned(),
            stamp_tick: unsigned(node, "stamp_tick")?,
            valid_from_tick: unsigned(node, "valid_from_tick")?,
            valid_until_tick: unsigned(node, "valid_until_tick")?,
        },
        uncertainty: Uncertainty {
            translation_um: unsigned(node, "uncertainty_um")?,
            ..Uncertainty::EXACT
        },
        calibration_identity: exact_identity(text(node, "calibration_identity")?)?,
        provenance_identity: exact_identity(text(node, "provenance_identity")?)?,
    };
    transform.validate(u64::MAX)?;
    Ok(transform)
}

fn point_from_node(node: &Node) -> Result<StampedPoint3, SpatialReason> {
    if text(node, "unit")? != "um" {
        return Err(SpatialReason::UnitMismatch);
    }
    if text(node, "axes")? != "x-right-y-forward-z-up" {
        return Err(SpatialReason::AxisMismatch);
    }
    FrameIdentity::new(text(node, "frame")?)?;
    Ok(StampedPoint3 {
        frame_id: text(node, "frame")?.to_owned(),
        xyz_um: [
            integer(node, "x_um")?,
            integer(node, "y_um")?,
            integer(node, "z_um")?,
        ],
        clock: text(node, "clock")?.to_owned(),
        tick: unsigned(node, "tick")?,
        uncertainty_um: unsigned(node, "uncertainty_um")?,
        provenance_identity: exact_identity(text(node, "provenance_identity")?)?,
    })
}

fn calibration_from_node(node: &Node) -> Result<PinholeCalibration, SpatialReason> {
    Ok(PinholeCalibration {
        frame_id: text(node, "camera_frame")?.to_owned(),
        calibration_identity: exact_identity(text(node, "calibration_identity")?)?,
        fx_millipixel: integer(node, "fx_millipixel")?,
        fy_millipixel: integer(node, "fy_millipixel")?,
        cx_millipixel: integer(node, "cx_millipixel")?,
        cy_millipixel: integer(node, "cy_millipixel")?,
        width_pixels: u32::try_from(unsigned(node, "width_pixels")?)
            .map_err(|_| SpatialReason::InvalidCalibration)?,
        height_pixels: u32::try_from(unsigned(node, "height_pixels")?)
            .map_err(|_| SpatialReason::InvalidCalibration)?,
        valid_until_tick: unsigned(node, "valid_until_tick")?,
    })
}

fn validate_literal(node: &Node, fields: usize, transform: bool) -> Result<(), ResolutionError> {
    if node.config.len() != fields {
        return Err(resolution(
            SpatialReason::WrongFrame,
            "spatial literal configuration is incomplete",
        ));
    }
    let value = if transform {
        transform_from_node(node).map(|_| ())
    } else {
        point_from_node(node).map(|_| ())
    };
    value.map_err(|reason| resolution(reason, "spatial literal configuration is invalid"))?;
    if unsigned(node, "maximum_output_bytes") != Ok(MAXIMUM_VALUE_BYTES as u64) {
        return Err(resolution(
            SpatialReason::WorkOverflow,
            "spatial literal requires the exact output bound",
        ));
    }
    Ok(())
}

fn validate_operation(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == OP_FIELDS.len()
        && unsigned(node, "maximum_uncertainty_um") == Ok(10)
        && unsigned(node, "maximum_work") == Ok(256))
    .then_some(())
    .ok_or_else(|| {
        resolution(
            SpatialReason::WorkOverflow,
            "spatial operation requires exact finite bounds",
        )
    })
}

fn validate_interpolate(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == INTERPOLATE_FIELDS.len()
        && unsigned(node, "maximum_history_values") == Ok(2)
        && unsigned(node, "maximum_work") == Ok(256)
        && unsigned(node, "maximum_window_ticks").is_ok()
        && unsigned(node, "maximum_uncertainty_um").is_ok()
        && unsigned(node, "tick").is_ok())
    .then_some(())
    .ok_or_else(|| {
        resolution(
            SpatialReason::HistoryOverflow,
            "interpolation requires exact finite history and work bounds",
        )
    })
}

fn validate_lookup(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == LOOKUP_FIELDS.len()
        && text(node, "source_frame").is_ok()
        && text(node, "target_frame").is_ok()
        && unsigned(node, "maximum_edges") == Ok(2)
        && unsigned(node, "maximum_work") == Ok(256)
        && unsigned(node, "maximum_uncertainty_um").is_ok())
    .then_some(())
    .ok_or_else(|| {
        resolution(
            SpatialReason::WorkOverflow,
            "transform lookup requires exact frames and finite graph bounds",
        )
    })
}

fn validate_calibration(node: &Node) -> Result<(), ResolutionError> {
    if node.config.len() != CALIBRATION_FIELDS.len()
        || unsigned(node, "maximum_work") != Ok(256)
        || unsigned(node, "maximum_output_bytes") != Ok(MAXIMUM_VALUE_BYTES as u64)
    {
        return Err(resolution(
            SpatialReason::WorkOverflow,
            "projection requires exact finite bounds",
        ));
    }
    calibration_from_node(node)
        .map(|_| ())
        .map_err(|reason| resolution(reason, "projection calibration is invalid"))
}

fn push_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), SpatialReason> {
    let length = u8::try_from(value.len()).map_err(|_| SpatialReason::FrameTooLong)?;
    bytes.push(length);
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_frame(bytes: &mut Vec<u8>, frame: &FrameIdentity) -> Result<(), SpatialReason> {
    push_string(bytes, &frame.id)?;
    bytes.push(match frame.unit {
        LinearUnit::Micrometre => 0,
        LinearUnit::Millimetre => 1,
    });
    bytes.push(match frame.handedness {
        Handedness::Right => 0,
        Handedness::Left => 1,
    });
    bytes.push(match frame.axes {
        AxisConvention::XRightYForwardZUp => 0,
        AxisConvention::XForwardYLeftZUp => 1,
    });
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], magic: &[u8; 4]) -> Result<Self, SpatialReason> {
        if bytes.len() > MAXIMUM_VALUE_BYTES || bytes.get(..4) != Some(magic) {
            return Err(SpatialReason::WrongFrame);
        }
        Ok(Self { bytes, offset: 4 })
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], SpatialReason> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(SpatialReason::NumericOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(SpatialReason::WrongFrame)?;
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, SpatialReason> {
        Ok(self.take(1)?[0])
    }
    fn u64(&mut self) -> Result<u64, SpatialReason> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, SpatialReason> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String, SpatialReason> {
        let length = usize::from(self.u8()?);
        String::from_utf8(self.take(length)?.to_vec()).map_err(|_| SpatialReason::WrongFrame)
    }
    fn identity(&mut self) -> Result<[u8; 32], SpatialReason> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    fn finish(self) -> Result<(), SpatialReason> {
        (self.offset == self.bytes.len())
            .then_some(())
            .ok_or(SpatialReason::WrongFrame)
    }
}

fn read_frame(cursor: &mut Cursor<'_>) -> Result<FrameIdentity, SpatialReason> {
    let id = cursor.string()?;
    let unit = match cursor.u8()? {
        0 => LinearUnit::Micrometre,
        1 => LinearUnit::Millimetre,
        _ => return Err(SpatialReason::UnitMismatch),
    };
    let handedness = match cursor.u8()? {
        0 => Handedness::Right,
        1 => Handedness::Left,
        _ => return Err(SpatialReason::HandednessMismatch),
    };
    let axes = match cursor.u8()? {
        0 => AxisConvention::XRightYForwardZUp,
        1 => AxisConvention::XForwardYLeftZUp,
        _ => return Err(SpatialReason::AxisMismatch),
    };
    let mut frame = FrameIdentity::new(id)?;
    frame.unit = unit;
    frame.handedness = handedness;
    frame.axes = axes;
    Ok(frame)
}

fn encode_transform(value: &Transform3) -> Result<Vec<u8>, SpatialReason> {
    let mut bytes = b"CST0".to_vec();
    push_frame(&mut bytes, &value.source)?;
    push_frame(&mut bytes, &value.target)?;
    for coordinate in value.translation_um {
        bytes.extend_from_slice(&coordinate.to_le_bytes());
    }
    bytes.push(value.quarter_turns_z);
    push_string(&mut bytes, &value.validity.clock)?;
    for tick in [
        value.validity.stamp_tick,
        value.validity.valid_from_tick,
        value.validity.valid_until_tick,
    ] {
        bytes.extend_from_slice(&tick.to_le_bytes());
    }
    bytes.extend_from_slice(&value.uncertainty.translation_um.to_le_bytes());
    bytes.extend_from_slice(&value.calibration_identity);
    bytes.extend_from_slice(&value.provenance_identity);
    if bytes.len() > MAXIMUM_VALUE_BYTES {
        return Err(SpatialReason::WorkOverflow);
    }
    Ok(bytes)
}

fn decode_transform(bytes: &[u8]) -> Result<Transform3, SpatialReason> {
    let mut cursor = Cursor::new(bytes, b"CST0")?;
    let source = read_frame(&mut cursor)?;
    let target = read_frame(&mut cursor)?;
    let translation_um = [cursor.i64()?, cursor.i64()?, cursor.i64()?];
    let quarter_turns_z = cursor.u8()?;
    let rotation =
        QuaternionQ30::quarter_turn_z(quarter_turns_z).ok_or(SpatialReason::InvalidQuaternion)?;
    let validity = Validity {
        clock: cursor.string()?,
        stamp_tick: cursor.u64()?,
        valid_from_tick: cursor.u64()?,
        valid_until_tick: cursor.u64()?,
    };
    let uncertainty = Uncertainty {
        translation_um: cursor.u64()?,
        ..Uncertainty::EXACT
    };
    let calibration_identity = cursor.identity()?;
    let provenance_identity = cursor.identity()?;
    cursor.finish()?;
    let value = Transform3 {
        source,
        target,
        translation_um,
        rotation,
        quarter_turns_z,
        validity,
        uncertainty,
        calibration_identity,
        provenance_identity,
    };
    value.validate(u64::MAX)?;
    Ok(value)
}

fn encode_point(value: &StampedPoint3) -> Result<Vec<u8>, SpatialReason> {
    let mut bytes = b"CSP0".to_vec();
    push_string(&mut bytes, &value.frame_id)?;
    for coordinate in value.xyz_um {
        bytes.extend_from_slice(&coordinate.to_le_bytes());
    }
    push_string(&mut bytes, &value.clock)?;
    bytes.extend_from_slice(&value.tick.to_le_bytes());
    bytes.extend_from_slice(&value.uncertainty_um.to_le_bytes());
    bytes.extend_from_slice(&value.provenance_identity);
    if bytes.len() > MAXIMUM_VALUE_BYTES {
        return Err(SpatialReason::WorkOverflow);
    }
    Ok(bytes)
}

fn decode_point(bytes: &[u8]) -> Result<StampedPoint3, SpatialReason> {
    let mut cursor = Cursor::new(bytes, b"CSP0")?;
    let value = StampedPoint3 {
        frame_id: cursor.string()?,
        xyz_um: [cursor.i64()?, cursor.i64()?, cursor.i64()?],
        clock: cursor.string()?,
        tick: cursor.u64()?,
        uncertainty_um: cursor.u64()?,
        provenance_identity: cursor.identity()?,
    };
    cursor.finish()?;
    FrameIdentity::new(value.frame_id.clone())?;
    Ok(value)
}

fn encode_pixel(value: &PixelPoint) -> Result<Vec<u8>, SpatialReason> {
    let mut bytes = b"CSX0".to_vec();
    push_string(&mut bytes, &value.frame_id)?;
    for coordinate in [value.x_millipixel, value.y_millipixel, value.depth_um] {
        bytes.extend_from_slice(&coordinate.to_le_bytes());
    }
    push_string(&mut bytes, &value.clock)?;
    bytes.extend_from_slice(&value.tick.to_le_bytes());
    bytes.extend_from_slice(&value.calibration_identity);
    if bytes.len() > MAXIMUM_VALUE_BYTES {
        return Err(SpatialReason::WorkOverflow);
    }
    Ok(bytes)
}

fn decode_pixel(bytes: &[u8]) -> Result<PixelPoint, SpatialReason> {
    let mut cursor = Cursor::new(bytes, b"CSX0")?;
    let value = PixelPoint {
        frame_id: cursor.string()?,
        x_millipixel: cursor.i64()?,
        y_millipixel: cursor.i64()?,
        depth_um: cursor.i64()?,
        clock: cursor.string()?,
        tick: cursor.u64()?,
        calibration_identity: cursor.identity()?,
    };
    cursor.finish()?;
    Ok(value)
}

fn typed<'a>(
    value: &'a Value,
    expected: TypeContractRef<'static>,
) -> Result<&'a [u8], SpatialReason> {
    (value.value_type == expected)
        .then_some(value.bytes.as_slice())
        .ok_or(SpatialReason::WrongFrame)
}

struct TransformLiteral;
impl Handler for TransformLiteral {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(SpatialReason::WrongFrame));
        }
        let value = transform_from_node(node).map_err(runtime)?;
        Ok(vec![Value {
            value_type: TRANSFORM_TYPE,
            bytes: encode_transform(&value).map_err(runtime)?,
        }])
    }
}

struct PointLiteral;
impl Handler for PointLiteral {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(SpatialReason::WrongFrame));
        }
        let value = point_from_node(node).map_err(runtime)?;
        Ok(vec![Value {
            value_type: POINT_TYPE,
            bytes: encode_point(&value).map_err(runtime)?,
        }])
    }
}

fn operation_bounds(node: &Node) -> Result<(NumericProfile, u64), RuntimeError> {
    if unsigned(node, "maximum_work").map_err(runtime)? != 256 {
        return Err(runtime(SpatialReason::WorkOverflow));
    }
    Ok((
        NumericProfile::FIRST_PROOF,
        unsigned(node, "maximum_uncertainty_um").map_err(runtime)?,
    ))
}

struct Compose;
impl Handler for Compose {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [first, second] = inputs else {
            return Err(runtime(SpatialReason::WrongFrame));
        };
        let first =
            decode_transform(typed(first, TRANSFORM_TYPE).map_err(runtime)?).map_err(runtime)?;
        let second =
            decode_transform(typed(second, TRANSFORM_TYPE).map_err(runtime)?).map_err(runtime)?;
        let (profile, maximum) = operation_bounds(node)?;
        let value = compose(&first, &second, profile, maximum).map_err(runtime)?;
        Ok(vec![Value {
            value_type: TRANSFORM_TYPE,
            bytes: encode_transform(&value).map_err(runtime)?,
        }])
    }
}

struct Invert;
impl Handler for Invert {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [value] = inputs else {
            return Err(runtime(SpatialReason::WrongFrame));
        };
        let value =
            decode_transform(typed(value, TRANSFORM_TYPE).map_err(runtime)?).map_err(runtime)?;
        let (profile, maximum) = operation_bounds(node)?;
        let value = invert(&value, profile, maximum).map_err(runtime)?;
        Ok(vec![Value {
            value_type: TRANSFORM_TYPE,
            bytes: encode_transform(&value).map_err(runtime)?,
        }])
    }
}

struct Lookup;
impl Handler for Lookup {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [first, second] = inputs else {
            return Err(runtime(SpatialReason::WrongFrame));
        };
        if unsigned(node, "maximum_edges").map_err(runtime)? != 2
            || unsigned(node, "maximum_work").map_err(runtime)? != 256
        {
            return Err(runtime(SpatialReason::WorkOverflow));
        }
        let first =
            decode_transform(typed(first, TRANSFORM_TYPE).map_err(runtime)?).map_err(runtime)?;
        let second =
            decode_transform(typed(second, TRANSFORM_TYPE).map_err(runtime)?).map_err(runtime)?;
        let value = lookup_transform(
            &[first, second],
            text(node, "source_frame").map_err(runtime)?,
            text(node, "target_frame").map_err(runtime)?,
            NumericProfile::FIRST_PROOF,
            unsigned(node, "maximum_uncertainty_um").map_err(runtime)?,
        )
        .map_err(runtime)?;
        Ok(vec![Value {
            value_type: TRANSFORM_TYPE,
            bytes: encode_transform(&value).map_err(runtime)?,
        }])
    }
}

struct Interpolate;
impl Handler for Interpolate {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [before, after] = inputs else {
            return Err(runtime(SpatialReason::WrongFrame));
        };
        if unsigned(node, "maximum_history_values").map_err(runtime)? != 2
            || unsigned(node, "maximum_work").map_err(runtime)? != 256
        {
            return Err(runtime(SpatialReason::HistoryOverflow));
        }
        let before =
            decode_transform(typed(before, TRANSFORM_TYPE).map_err(runtime)?).map_err(runtime)?;
        let after =
            decode_transform(typed(after, TRANSFORM_TYPE).map_err(runtime)?).map_err(runtime)?;
        let value = interpolate(
            &before,
            &after,
            unsigned(node, "tick").map_err(runtime)?,
            unsigned(node, "maximum_window_ticks").map_err(runtime)?,
            unsigned(node, "maximum_uncertainty_um").map_err(runtime)?,
        )
        .map_err(runtime)?;
        Ok(vec![Value {
            value_type: TRANSFORM_TYPE,
            bytes: encode_transform(&value).map_err(runtime)?,
        }])
    }
}

struct Apply;
impl Handler for Apply {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [transform_value, point_value] = inputs else {
            return Err(runtime(SpatialReason::WrongFrame));
        };
        let transform = decode_transform(typed(transform_value, TRANSFORM_TYPE).map_err(runtime)?)
            .map_err(runtime)?;
        let point =
            decode_point(typed(point_value, POINT_TYPE).map_err(runtime)?).map_err(runtime)?;
        let (profile, maximum) = operation_bounds(node)?;
        let value = apply_transform(&transform, &point, profile, maximum).map_err(runtime)?;
        Ok(vec![Value {
            value_type: POINT_TYPE,
            bytes: encode_point(&value).map_err(runtime)?,
        }])
    }
}

struct Project;
impl Handler for Project {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [point] = inputs else {
            return Err(runtime(SpatialReason::WrongFrame));
        };
        if unsigned(node, "maximum_work").map_err(runtime)? != 256 {
            return Err(runtime(SpatialReason::WorkOverflow));
        }
        let point = decode_point(typed(point, POINT_TYPE).map_err(runtime)?).map_err(runtime)?;
        let calibration = calibration_from_node(node).map_err(runtime)?;
        let value = project(&point, &calibration).map_err(runtime)?;
        Ok(vec![Value {
            value_type: PIXEL_TYPE,
            bytes: encode_pixel(&value).map_err(runtime)?,
        }])
    }
}

struct Unproject;
impl Handler for Unproject {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [pixel] = inputs else {
            return Err(runtime(SpatialReason::WrongFrame));
        };
        if unsigned(node, "maximum_work").map_err(runtime)? != 256 {
            return Err(runtime(SpatialReason::WorkOverflow));
        }
        let pixel = decode_pixel(typed(pixel, PIXEL_TYPE).map_err(runtime)?).map_err(runtime)?;
        let calibration = calibration_from_node(node).map_err(runtime)?;
        let value = unproject(&pixel, &calibration).map_err(runtime)?;
        Ok(vec![Value {
            value_type: POINT_TYPE,
            bytes: encode_point(&value).map_err(runtime)?,
        }])
    }
}

struct PointInspect;
impl Handler for PointInspect {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [point] = inputs else {
            return Err(runtime(SpatialReason::WrongFrame));
        };
        let point = decode_point(typed(point, POINT_TYPE).map_err(runtime)?).map_err(runtime)?;
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: format!(
                "spatial:point:{}:[{},{},{}]:{}@{}:uncertainty={}",
                point.frame_id,
                point.xyz_um[0],
                point.xyz_um[1],
                point.xyz_um[2],
                point.clock,
                point.tick,
                point.uncertainty_um
            )
            .into_bytes(),
        }])
    }
}

pub fn register_spatial_contracts(registry: &mut Registry) {
    for contract in SPATIAL_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

pub fn register_deterministic_spatial_provider(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_spatial_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validator) in [
        (
            &TRANSFORM_LITERAL_CONTRACT,
            "conduit.spatial/transform-literal-deterministic",
            "conduit.spatial/transform-literal-artifact",
            "spatial-transform-literal",
            (|| Box::new(TransformLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            (|node: &Node| validate_literal(node, TRANSFORM_FIELDS.len(), true))
                as conduit_runtime::ConfigValidator,
        ),
        (
            &POINT_LITERAL_CONTRACT,
            "conduit.spatial/point-literal-deterministic",
            "conduit.spatial/point-literal-artifact",
            "spatial-point-literal",
            (|| Box::new(PointLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            (|node: &Node| validate_literal(node, POINT_FIELDS.len(), false))
                as conduit_runtime::ConfigValidator,
        ),
        (
            &COMPOSE_CONTRACT,
            "conduit.spatial/compose-deterministic",
            "conduit.spatial/compose-artifact",
            "spatial-compose",
            (|| Box::new(Compose) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_operation as conduit_runtime::ConfigValidator,
        ),
        (
            &INVERT_CONTRACT,
            "conduit.spatial/invert-deterministic",
            "conduit.spatial/invert-artifact",
            "spatial-invert",
            (|| Box::new(Invert) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_operation as conduit_runtime::ConfigValidator,
        ),
        (
            &LOOKUP_CONTRACT,
            "conduit.spatial/lookup-deterministic",
            "conduit.spatial/lookup-artifact",
            "spatial-lookup",
            (|| Box::new(Lookup) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_lookup as conduit_runtime::ConfigValidator,
        ),
        (
            &INTERPOLATE_CONTRACT,
            "conduit.spatial/interpolate-deterministic",
            "conduit.spatial/interpolate-artifact",
            "spatial-interpolate",
            (|| Box::new(Interpolate) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_interpolate as conduit_runtime::ConfigValidator,
        ),
        (
            &APPLY_CONTRACT,
            "conduit.spatial/apply-deterministic",
            "conduit.spatial/apply-artifact",
            "spatial-apply",
            (|| Box::new(Apply) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_operation as conduit_runtime::ConfigValidator,
        ),
        (
            &PROJECT_CONTRACT,
            "conduit.spatial/project-deterministic",
            "conduit.spatial/project-artifact",
            "spatial-project",
            (|| Box::new(Project) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_calibration as conduit_runtime::ConfigValidator,
        ),
        (
            &UNPROJECT_CONTRACT,
            "conduit.spatial/unproject-deterministic",
            "conduit.spatial/unproject-artifact",
            "spatial-unproject",
            (|| Box::new(Unproject) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_calibration as conduit_runtime::ConfigValidator,
        ),
        (
            &POINT_INSPECT_CONTRACT,
            "conduit.spatial/point-inspect-deterministic",
            "conduit.spatial/point-inspect-artifact",
            "spatial-point-inspect",
            (|| Box::new(PointInspect) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            (|node: &Node| {
                node.config.is_empty().then_some(()).ok_or_else(|| {
                    resolution(
                        SpatialReason::WrongFrame,
                        "point inspector has no configuration",
                    )
                })
            }) as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory,
            validate_config: validator,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codecs_are_exact_and_bounded() {
        let transform = Transform3 {
            source: FrameIdentity::new("sensor").unwrap(),
            target: FrameIdentity::new("camera").unwrap(),
            translation_um: [10, 20, 30],
            rotation: QuaternionQ30::quarter_turn_z(1).unwrap(),
            quarter_turns_z: 1,
            validity: Validity {
                clock: "clock/fixture".to_owned(),
                stamp_tick: 10,
                valid_from_tick: 9,
                valid_until_tick: 11,
            },
            uncertainty: Uncertainty {
                translation_um: 1,
                ..Uncertainty::EXACT
            },
            calibration_identity: [0x51; 32],
            provenance_identity: [0x52; 32],
        };
        assert_eq!(
            decode_transform(&encode_transform(&transform).unwrap()).unwrap(),
            transform
        );
        let point = StampedPoint3 {
            frame_id: "camera".to_owned(),
            xyz_um: [1, 2, 3],
            clock: "clock/fixture".to_owned(),
            tick: 10,
            uncertainty_um: 1,
            provenance_identity: [0x52; 32],
        };
        assert_eq!(decode_point(&encode_point(&point).unwrap()).unwrap(), point);
        let pixel = PixelPoint {
            frame_id: "camera".to_owned(),
            x_millipixel: 1,
            y_millipixel: 2,
            depth_um: 3,
            clock: "clock/fixture".to_owned(),
            tick: 10,
            calibration_identity: [0x51; 32],
        };
        assert_eq!(decode_pixel(&encode_pixel(&pixel).unwrap()).unwrap(), pixel);
    }

    #[test]
    fn contracts_alone_install_no_spatial_provider() {
        let mut registry = Registry::default();
        register_spatial_contracts(&mut registry);
        assert!(
            registry
                .installed_providers()
                .iter()
                .all(|provider| !SPATIAL_CONTRACTS.contains(&provider.contract))
        );
    }
}
