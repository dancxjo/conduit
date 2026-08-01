//! Bounded streaming spatial data above the frame and transform foundation.

use crate::{
    FrameIdentity, LinearUnit, NumericProfile, SpatialReason, StampedPoint3, Transform3, Validity,
    apply_transform,
};

pub const MAXIMUM_SCAN_POINTS: usize = 8;
pub const MAXIMUM_SCAN_CHUNKS: usize = 4;
pub const MAXIMUM_GRID_CELLS: usize = 16;
pub const MAXIMUM_TRAJECTORY_POSES: usize = 8;
pub const MAXIMUM_SPATIAL_DATA_BYTES: usize = 4096;
pub const MAXIMUM_SPATIAL_DATA_WORK: usize = 128;

pub const SCAN_SCHEMA_IDENTITY: [u8; 32] = [0xa1; 32];
pub const GRID_SCHEMA_IDENTITY: [u8; 32] = [0xa2; 32];
pub const TRAJECTORY_SCHEMA_IDENTITY: [u8; 32] = [0xa3; 32];
pub const REPRESENTATION_IDENTITY: [u8; 32] = [0xa4; 32];
pub const SNAPSHOT_IDENTITY: [u8; 32] = [0xa5; 32];
pub const PROVIDER_IDENTITY: [u8; 32] = [0xa6; 32];
pub const DATA_PROVENANCE_IDENTITY: [u8; 32] = [0xa7; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialDataReason {
    SchemaMismatch,
    SnapshotMismatch,
    RepresentationMismatch,
    ProviderUnavailable,
    WrongFrame,
    WrongUnit,
    WrongClock,
    CalibrationMismatch,
    StaleTransform,
    ExcessiveUncertainty,
    PointOverflow,
    GridOverflow,
    TrajectoryOverflow,
    ByteOverflow,
    WorkOverflow,
    ChunkGap,
    ChunkReordered,
    PartialCoverage,
    Cancellation,
    WrongType,
}

impl SpatialDataReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "CND-SPATIAL-DATA-001",
            Self::SnapshotMismatch | Self::RepresentationMismatch => "CND-SPATIAL-DATA-002",
            Self::ProviderUnavailable => "CND-SPATIAL-DATA-003",
            Self::WrongFrame | Self::WrongUnit | Self::WrongClock => "CND-SPATIAL-DATA-004",
            Self::CalibrationMismatch => "CND-SPATIAL-DATA-005",
            Self::StaleTransform => "CND-SPATIAL-DATA-006",
            Self::ExcessiveUncertainty => "CND-SPATIAL-DATA-007",
            Self::PointOverflow
            | Self::GridOverflow
            | Self::TrajectoryOverflow
            | Self::ByteOverflow
            | Self::WorkOverflow => "CND-SPATIAL-DATA-008",
            Self::ChunkGap | Self::ChunkReordered => "CND-SPATIAL-DATA-009",
            Self::PartialCoverage => "CND-SPATIAL-DATA-010",
            Self::Cancellation => "CND-SPATIAL-DATA-011",
            Self::WrongType => "CND-SPATIAL-DATA-012",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialRepresentation {
    SignedMicrometreXyz,
    OccupancyU8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanPoint {
    pub xyz_um: [i64; 3],
    pub uncertainty_um: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanChunk {
    pub schema: [u8; 32],
    pub schema_version: u32,
    pub snapshot: [u8; 32],
    pub representation: [u8; 32],
    pub provider: [u8; 32],
    pub provenance: [u8; 32],
    pub frame: FrameIdentity,
    pub validity: Validity,
    pub calibration: [u8; 32],
    pub chunk_index: usize,
    pub chunk_count: usize,
    pub points: Vec<ScanPoint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RangeScan {
    pub schema: [u8; 32],
    pub schema_version: u32,
    pub snapshot: [u8; 32],
    pub representation: [u8; 32],
    pub provider: [u8; 32],
    pub provenance: [u8; 32],
    pub frame: FrameIdentity,
    pub validity: Validity,
    pub calibration: [u8; 32],
    pub coverage_complete: bool,
    pub points: Vec<ScanPoint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OccupancyGrid {
    pub schema: [u8; 32],
    pub schema_version: u32,
    pub snapshot: [u8; 32],
    pub representation: [u8; 32],
    pub provider: [u8; 32],
    pub provenance: [u8; 32],
    pub frame: FrameIdentity,
    pub validity: Validity,
    pub calibration: [u8; 32],
    pub coverage_complete: bool,
    pub width: usize,
    pub height: usize,
    pub resolution_um: u64,
    pub cells: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trajectory {
    pub schema: [u8; 32],
    pub schema_version: u32,
    pub snapshot: [u8; 32],
    pub frame: FrameIdentity,
    pub clock: String,
    pub interpolation: &'static str,
    pub poses: Vec<crate::StampedPose3>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpatialDataLimits {
    pub maximum_points: usize,
    pub maximum_chunks: usize,
    pub maximum_grid_cells: usize,
    pub maximum_trajectory_poses: usize,
    pub maximum_bytes: usize,
    pub maximum_work: usize,
    pub maximum_uncertainty_um: u64,
}

impl SpatialDataLimits {
    pub const FIRST_PROOF: Self = Self {
        maximum_points: MAXIMUM_SCAN_POINTS,
        maximum_chunks: MAXIMUM_SCAN_CHUNKS,
        maximum_grid_cells: MAXIMUM_GRID_CELLS,
        maximum_trajectory_poses: MAXIMUM_TRAJECTORY_POSES,
        maximum_bytes: MAXIMUM_SPATIAL_DATA_BYTES,
        maximum_work: MAXIMUM_SPATIAL_DATA_WORK,
        maximum_uncertainty_um: 10,
    };

    fn validate(self) -> Result<(), SpatialDataReason> {
        if self.maximum_points == 0 || self.maximum_points > MAXIMUM_SCAN_POINTS {
            return Err(SpatialDataReason::PointOverflow);
        }
        if self.maximum_chunks == 0 || self.maximum_chunks > MAXIMUM_SCAN_CHUNKS {
            return Err(SpatialDataReason::ChunkGap);
        }
        if self.maximum_grid_cells == 0 || self.maximum_grid_cells > MAXIMUM_GRID_CELLS {
            return Err(SpatialDataReason::GridOverflow);
        }
        if self.maximum_trajectory_poses == 0
            || self.maximum_trajectory_poses > MAXIMUM_TRAJECTORY_POSES
        {
            return Err(SpatialDataReason::TrajectoryOverflow);
        }
        if self.maximum_bytes == 0 || self.maximum_bytes > MAXIMUM_SPATIAL_DATA_BYTES {
            return Err(SpatialDataReason::ByteOverflow);
        }
        if self.maximum_work == 0 || self.maximum_work > MAXIMUM_SPATIAL_DATA_WORK {
            return Err(SpatialDataReason::WorkOverflow);
        }
        Ok(())
    }
}

fn validate_chunk_identity(chunk: &ScanChunk) -> Result<(), SpatialDataReason> {
    if chunk.schema != SCAN_SCHEMA_IDENTITY || chunk.schema_version != 0 {
        return Err(SpatialDataReason::SchemaMismatch);
    }
    if chunk.snapshot != SNAPSHOT_IDENTITY {
        return Err(SpatialDataReason::SnapshotMismatch);
    }
    if chunk.representation != REPRESENTATION_IDENTITY {
        return Err(SpatialDataReason::RepresentationMismatch);
    }
    if chunk.provider != PROVIDER_IDENTITY {
        return Err(SpatialDataReason::ProviderUnavailable);
    }
    if chunk.frame.unit != LinearUnit::Micrometre {
        return Err(SpatialDataReason::WrongUnit);
    }
    if chunk.validity.valid_from_tick > chunk.validity.stamp_tick
        || chunk.validity.stamp_tick > chunk.validity.valid_until_tick
    {
        return Err(SpatialDataReason::StaleTransform);
    }
    Ok(())
}

pub fn normalize_scan_chunks(
    chunks: &[ScanChunk],
    limits: SpatialDataLimits,
    cancelled: bool,
) -> Result<RangeScan, SpatialDataReason> {
    limits.validate()?;
    if cancelled {
        return Err(SpatialDataReason::Cancellation);
    }
    let first = chunks.first().ok_or(SpatialDataReason::PartialCoverage)?;
    validate_chunk_identity(first)?;
    if first.chunk_count == 0 || first.chunk_count > limits.maximum_chunks {
        return Err(SpatialDataReason::ChunkGap);
    }
    if chunks.len() != first.chunk_count {
        return Err(SpatialDataReason::PartialCoverage);
    }
    let mut points = Vec::new();
    let mut work = 0_usize;
    for (expected, chunk) in chunks.iter().enumerate() {
        validate_chunk_identity(chunk)?;
        if chunk.chunk_index < expected {
            return Err(SpatialDataReason::ChunkReordered);
        }
        if chunk.chunk_index > expected || chunk.chunk_count != first.chunk_count {
            return Err(SpatialDataReason::ChunkGap);
        }
        if chunk.schema != first.schema
            || chunk.snapshot != first.snapshot
            || chunk.representation != first.representation
            || chunk.provider != first.provider
            || chunk.provenance != first.provenance
            || chunk.frame != first.frame
            || chunk.validity != first.validity
            || chunk.calibration != first.calibration
        {
            return Err(SpatialDataReason::SnapshotMismatch);
        }
        work = work
            .checked_add(chunk.points.len())
            .ok_or(SpatialDataReason::WorkOverflow)?;
        if work > limits.maximum_work {
            return Err(SpatialDataReason::WorkOverflow);
        }
        points.extend_from_slice(&chunk.points);
        if points.len() > limits.maximum_points {
            return Err(SpatialDataReason::PointOverflow);
        }
    }
    if points
        .iter()
        .any(|point| point.uncertainty_um > limits.maximum_uncertainty_um)
    {
        return Err(SpatialDataReason::ExcessiveUncertainty);
    }
    let encoded_bytes = points
        .len()
        .checked_mul(32)
        .ok_or(SpatialDataReason::ByteOverflow)?;
    if encoded_bytes > limits.maximum_bytes {
        return Err(SpatialDataReason::ByteOverflow);
    }
    Ok(RangeScan {
        schema: first.schema,
        schema_version: first.schema_version,
        snapshot: first.snapshot,
        representation: first.representation,
        provider: first.provider,
        provenance: first.provenance,
        frame: first.frame.clone(),
        validity: first.validity.clone(),
        calibration: first.calibration,
        coverage_complete: true,
        points,
    })
}

fn map_spatial_reason(reason: SpatialReason) -> SpatialDataReason {
    match reason {
        SpatialReason::WrongFrame | SpatialReason::UnknownFrame | SpatialReason::SameFrame => {
            SpatialDataReason::WrongFrame
        }
        SpatialReason::UnitMismatch
        | SpatialReason::HandednessMismatch
        | SpatialReason::AxisMismatch => SpatialDataReason::WrongUnit,
        SpatialReason::ClockMismatch | SpatialReason::MissingClockConversion => {
            SpatialDataReason::WrongClock
        }
        SpatialReason::CalibrationMismatch | SpatialReason::InvalidCalibration => {
            SpatialDataReason::CalibrationMismatch
        }
        SpatialReason::StaleTransform | SpatialReason::InterpolationBoundary => {
            SpatialDataReason::StaleTransform
        }
        SpatialReason::ExcessiveUncertainty => SpatialDataReason::ExcessiveUncertainty,
        SpatialReason::WorkOverflow => SpatialDataReason::WorkOverflow,
        _ => SpatialDataReason::WrongType,
    }
}

pub fn transform_scan(
    scan: &RangeScan,
    transform: &Transform3,
    limits: SpatialDataLimits,
) -> Result<RangeScan, SpatialDataReason> {
    limits.validate()?;
    if !scan.coverage_complete {
        return Err(SpatialDataReason::PartialCoverage);
    }
    if scan.calibration != transform.calibration_identity {
        return Err(SpatialDataReason::CalibrationMismatch);
    }
    if scan.points.len() > limits.maximum_points || scan.points.len() > limits.maximum_work {
        return Err(SpatialDataReason::WorkOverflow);
    }
    let mut points = Vec::with_capacity(scan.points.len());
    for point in &scan.points {
        let stamped = StampedPoint3 {
            frame_id: scan.frame.id.clone(),
            xyz_um: point.xyz_um,
            clock: scan.validity.clock.clone(),
            tick: scan.validity.stamp_tick,
            uncertainty_um: point.uncertainty_um,
            provenance_identity: scan.provenance,
        };
        let profile = NumericProfile {
            maximum_work: limits.maximum_work,
            ..NumericProfile::FIRST_PROOF
        };
        let transformed =
            apply_transform(transform, &stamped, profile, limits.maximum_uncertainty_um)
                .map_err(map_spatial_reason)?;
        points.push(ScanPoint {
            xyz_um: transformed.xyz_um,
            uncertainty_um: transformed.uncertainty_um,
        });
    }
    Ok(RangeScan {
        frame: transform.target.clone(),
        points,
        ..scan.clone()
    })
}

pub fn grid_from_scan(
    scan: &RangeScan,
    width: usize,
    height: usize,
    resolution_um: u64,
    limits: SpatialDataLimits,
) -> Result<OccupancyGrid, SpatialDataReason> {
    limits.validate()?;
    if !scan.coverage_complete {
        return Err(SpatialDataReason::PartialCoverage);
    }
    let cell_count = width
        .checked_mul(height)
        .ok_or(SpatialDataReason::GridOverflow)?;
    if width == 0 || height == 0 || resolution_um == 0 || cell_count > limits.maximum_grid_cells {
        return Err(SpatialDataReason::GridOverflow);
    }
    if scan.points.len() > limits.maximum_work {
        return Err(SpatialDataReason::WorkOverflow);
    }
    let mut cells = vec![0_u8; cell_count];
    for point in &scan.points {
        let x = point.xyz_um[0].unsigned_abs() / resolution_um;
        let y = point.xyz_um[1].unsigned_abs() / resolution_um;
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return Err(SpatialDataReason::GridOverflow);
        };
        if x < width && y < height {
            cells[y * width + x] = 255;
        }
    }
    Ok(OccupancyGrid {
        schema: GRID_SCHEMA_IDENTITY,
        schema_version: 0,
        snapshot: scan.snapshot,
        representation: REPRESENTATION_IDENTITY,
        provider: scan.provider,
        provenance: scan.provenance,
        frame: scan.frame.clone(),
        validity: scan.validity.clone(),
        calibration: scan.calibration,
        coverage_complete: true,
        width,
        height,
        resolution_um,
        cells,
    })
}

pub fn validate_trajectory(
    trajectory: &Trajectory,
    limits: SpatialDataLimits,
) -> Result<(), SpatialDataReason> {
    limits.validate()?;
    if trajectory.schema != TRAJECTORY_SCHEMA_IDENTITY || trajectory.schema_version != 0 {
        return Err(SpatialDataReason::SchemaMismatch);
    }
    if trajectory.snapshot != SNAPSHOT_IDENTITY {
        return Err(SpatialDataReason::SnapshotMismatch);
    }
    if trajectory.interpolation != "linear-q30-shortest" {
        return Err(SpatialDataReason::RepresentationMismatch);
    }
    if trajectory.poses.len() > limits.maximum_trajectory_poses {
        return Err(SpatialDataReason::TrajectoryOverflow);
    }
    if trajectory
        .poses
        .iter()
        .any(|pose| pose.pose.frame != trajectory.frame)
    {
        return Err(SpatialDataReason::WrongFrame);
    }
    if trajectory
        .poses
        .iter()
        .any(|pose| pose.validity.clock != trajectory.clock)
    {
        return Err(SpatialDataReason::WrongClock);
    }
    Ok(())
}

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

pub const SCAN_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("spatial/range-scan"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0xb1; 32]),
};
pub const GRID_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("spatial/occupancy-grid"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0xb2; 32]),
};
pub const TRAJECTORY_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("spatial/trajectory"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0xb3; 32]),
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

pub const SCAN_SCHEMA_IDENTITY_TEXT: &str =
    "sha256:a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
pub const GRID_SCHEMA_IDENTITY_TEXT: &str =
    "sha256:a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2";
pub const TRAJECTORY_SCHEMA_IDENTITY_TEXT: &str =
    "sha256:a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3";
pub const REPRESENTATION_IDENTITY_TEXT: &str =
    "sha256:a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4";
pub const SNAPSHOT_IDENTITY_TEXT: &str =
    "sha256:a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5";
pub const PROVIDER_IDENTITY_TEXT: &str =
    "sha256:a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6";
pub const DATA_PROVENANCE_IDENTITY_TEXT: &str =
    "sha256:a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7";

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

const fn port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections: if matches!(direction, Direction::Input) {
            ConnectionCardinality::ExactlyOne
        } else {
            ConnectionCardinality::OneOrMore
        },
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

const FIXTURE_FIELDS: [ConfigFieldContract<'static>; 16] = [
    field("scan_schema_identity", TEXT_TYPE),
    field("scan_schema_version", U64_TYPE),
    field("snapshot_identity", TEXT_TYPE),
    field("representation_identity", TEXT_TYPE),
    field("provider_identity", TEXT_TYPE),
    field("provenance_identity", TEXT_TYPE),
    field("frame", TEXT_TYPE),
    field("unit", TEXT_TYPE),
    field("clock", TEXT_TYPE),
    field("tick", U64_TYPE),
    field("valid_until_tick", U64_TYPE),
    field("calibration_identity", TEXT_TYPE),
    field("maximum_points", U64_TYPE),
    field("maximum_chunks", U64_TYPE),
    field("maximum_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const TRANSFORM_FIELDS: [ConfigFieldContract<'static>; 4] = [
    field("maximum_points", U64_TYPE),
    field("maximum_uncertainty_um", U64_TYPE),
    field("maximum_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const GRID_FIELDS: [ConfigFieldContract<'static>; 10] = [
    field("grid_schema_identity", TEXT_TYPE),
    field("snapshot_identity", TEXT_TYPE),
    field("representation_identity", TEXT_TYPE),
    field("width", U64_TYPE),
    field("height", U64_TYPE),
    field("resolution_um", U64_TYPE),
    field("maximum_grid_cells", U64_TYPE),
    field("maximum_points", U64_TYPE),
    field("maximum_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const TRAJECTORY_FIELDS: [ConfigFieldContract<'static>; 9] = [
    field("trajectory_schema_identity", TEXT_TYPE),
    field("trajectory_schema_version", U64_TYPE),
    field("snapshot_identity", TEXT_TYPE),
    field("frame", TEXT_TYPE),
    field("clock", TEXT_TYPE),
    field("interpolation", TEXT_TYPE),
    field("maximum_trajectory_poses", U64_TYPE),
    field("maximum_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
];

const SCAN_OUTPUT: [PortContract<'static>; 1] = [port("scan", Direction::Output, SCAN_TYPE)];
const TRANSFORM_INPUTS: [PortContract<'static>; 2] = [
    port("scan", Direction::Input, SCAN_TYPE),
    port("transform", Direction::Input, crate::TRANSFORM_TYPE),
];
const SCAN_INPUT: [PortContract<'static>; 1] = [port("scan", Direction::Input, SCAN_TYPE)];
const GRID_OUTPUT: [PortContract<'static>; 1] = [port("grid", Direction::Output, GRID_TYPE)];
const GRID_INPUT: [PortContract<'static>; 1] = [port("grid", Direction::Input, GRID_TYPE)];
const TEXT_OUTPUT: [PortContract<'static>; 1] = [PortContract {
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    ..port("summary", Direction::Output, TEXT_TYPE)
}];
const TRAJECTORY_OUTPUT: [PortContract<'static>; 1] =
    [port("trajectory", Direction::Output, TRAJECTORY_TYPE)];
const TRAJECTORY_INPUT: [PortContract<'static>; 1] =
    [port("trajectory", Direction::Input, TRAJECTORY_TYPE)];

pub const SCAN_FIXTURE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/scan/fixture"),
    config: ConfigContract {
        fields: &FIXTURE_FIELDS,
    },
    inputs: &[],
    outputs: &SCAN_OUTPUT,
};
pub const SCAN_TRANSFORM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/scan/transform"),
    config: ConfigContract {
        fields: &TRANSFORM_FIELDS,
    },
    inputs: &TRANSFORM_INPUTS,
    outputs: &SCAN_OUTPUT,
};
pub const GRID_FROM_SCAN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/grid/from-scan"),
    config: ConfigContract {
        fields: &GRID_FIELDS,
    },
    inputs: &SCAN_INPUT,
    outputs: &GRID_OUTPUT,
};
pub const GRID_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/grid/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &GRID_INPUT,
    outputs: &TEXT_OUTPUT,
};
pub const TRAJECTORY_FIXTURE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/trajectory/fixture"),
    config: ConfigContract {
        fields: &TRAJECTORY_FIELDS,
    },
    inputs: &[],
    outputs: &TRAJECTORY_OUTPUT,
};
pub const TRAJECTORY_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("spatial/trajectory/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &TRAJECTORY_INPUT,
    outputs: &TEXT_OUTPUT,
};

pub const SPATIAL_DATA_CONTRACTS: [&NodeContract<'static>; 6] = [
    &SCAN_FIXTURE_CONTRACT,
    &SCAN_TRANSFORM_CONTRACT,
    &GRID_FROM_SCAN_CONTRACT,
    &GRID_INSPECT_CONTRACT,
    &TRAJECTORY_FIXTURE_CONTRACT,
    &TRAJECTORY_INSPECT_CONTRACT,
];

pub fn register_spatial_data_contracts(registry: &mut Registry) {
    for contract in SPATIAL_DATA_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

fn exact_u64(node: &Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn exact_text(node: &Node, key: &str, value: &str) -> bool {
    node.config(key) == Some(value)
}

fn exact_limits(node: &Node) -> bool {
    exact_u64(node, "maximum_points") == Some(MAXIMUM_SCAN_POINTS as u64)
        && exact_u64(node, "maximum_bytes") == Some(MAXIMUM_SPATIAL_DATA_BYTES as u64)
        && exact_u64(node, "maximum_work") == Some(MAXIMUM_SPATIAL_DATA_WORK as u64)
}

fn validate_fixture_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == FIXTURE_FIELDS.len()
        && exact_text(node, "scan_schema_identity", SCAN_SCHEMA_IDENTITY_TEXT)
        && exact_u64(node, "scan_schema_version") == Some(0)
        && exact_text(node, "snapshot_identity", SNAPSHOT_IDENTITY_TEXT)
        && exact_text(
            node,
            "representation_identity",
            REPRESENTATION_IDENTITY_TEXT,
        )
        && exact_text(node, "provider_identity", PROVIDER_IDENTITY_TEXT)
        && exact_text(node, "provenance_identity", DATA_PROVENANCE_IDENTITY_TEXT)
        && exact_text(node, "frame", "sensor")
        && exact_text(node, "unit", "um")
        && exact_text(node, "clock", "clock/fixture")
        && exact_u64(node, "tick") == Some(10)
        && exact_u64(node, "valid_until_tick") == Some(20)
        && exact_text(
            node,
            "calibration_identity",
            crate::runtime_nodes::CALIBRATION_IDENTITY_TEXT,
        )
        && exact_u64(node, "maximum_chunks") == Some(2)
        && exact_limits(node))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-SPATIAL-DATA-001",
            "scan fixture requires exact identities and finite limits",
        )
    })
}

fn validate_transform_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == TRANSFORM_FIELDS.len()
        && exact_limits(node)
        && exact_u64(node, "maximum_uncertainty_um") == Some(10))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-SPATIAL-DATA-008",
            "scan transform limits are not exact",
        )
    })
}

fn validate_grid_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == GRID_FIELDS.len()
        && exact_text(node, "grid_schema_identity", GRID_SCHEMA_IDENTITY_TEXT)
        && exact_text(node, "snapshot_identity", SNAPSHOT_IDENTITY_TEXT)
        && exact_text(
            node,
            "representation_identity",
            REPRESENTATION_IDENTITY_TEXT,
        )
        && exact_u64(node, "width") == Some(2)
        && exact_u64(node, "height") == Some(2)
        && exact_u64(node, "resolution_um") == Some(1000)
        && exact_u64(node, "maximum_grid_cells") == Some(MAXIMUM_GRID_CELLS as u64)
        && exact_limits(node))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-SPATIAL-DATA-008",
            "grid requires exact snapshot, representation, dimensions, and limits",
        )
    })
}

fn validate_trajectory_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == TRAJECTORY_FIELDS.len()
        && exact_text(
            node,
            "trajectory_schema_identity",
            TRAJECTORY_SCHEMA_IDENTITY_TEXT,
        )
        && exact_u64(node, "trajectory_schema_version") == Some(0)
        && exact_text(node, "snapshot_identity", SNAPSHOT_IDENTITY_TEXT)
        && exact_text(node, "frame", "map")
        && exact_text(node, "clock", "clock/fixture")
        && exact_text(node, "interpolation", "linear-q30-shortest")
        && exact_u64(node, "maximum_trajectory_poses") == Some(MAXIMUM_TRAJECTORY_POSES as u64)
        && exact_u64(node, "maximum_bytes") == Some(MAXIMUM_SPATIAL_DATA_BYTES as u64)
        && exact_u64(node, "maximum_work") == Some(MAXIMUM_SPATIAL_DATA_WORK as u64))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-SPATIAL-DATA-008",
            "trajectory requires exact identity, interpolation, and finite limits",
        )
    })
}

fn no_config(node: &Node) -> Result<(), ResolutionError> {
    node.config.is_empty().then_some(()).ok_or_else(|| {
        ResolutionError::new("CND-SPATIAL-DATA-012", "inspector accepts no configuration")
    })
}

fn runtime(reason: SpatialDataReason) -> RuntimeError {
    RuntimeError::new(
        reason.code(),
        format!("bounded spatial-data operation failed: {reason:?}"),
    )
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn take_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, SpatialDataReason> {
    let end = cursor
        .checked_add(8)
        .ok_or(SpatialDataReason::ByteOverflow)?;
    let slice = bytes
        .get(*cursor..end)
        .ok_or(SpatialDataReason::WrongType)?;
    *cursor = end;
    Ok(u64::from_le_bytes(
        slice.try_into().map_err(|_| SpatialDataReason::WrongType)?,
    ))
}
fn push_text(output: &mut Vec<u8>, value: &str) -> Result<(), SpatialDataReason> {
    push_u64(
        output,
        u64::try_from(value.len()).map_err(|_| SpatialDataReason::ByteOverflow)?,
    );
    output.extend_from_slice(value.as_bytes());
    Ok(())
}
fn take_text(bytes: &[u8], cursor: &mut usize) -> Result<String, SpatialDataReason> {
    let length =
        usize::try_from(take_u64(bytes, cursor)?).map_err(|_| SpatialDataReason::ByteOverflow)?;
    let end = cursor
        .checked_add(length)
        .ok_or(SpatialDataReason::ByteOverflow)?;
    let value = core::str::from_utf8(
        bytes
            .get(*cursor..end)
            .ok_or(SpatialDataReason::WrongType)?,
    )
    .map_err(|_| SpatialDataReason::WrongType)?;
    *cursor = end;
    Ok(value.to_owned())
}

fn encode_scan(scan: &RangeScan) -> Result<Vec<u8>, SpatialDataReason> {
    let mut output = b"SDS0".to_vec();
    push_text(&mut output, &scan.frame.id)?;
    push_text(&mut output, &scan.validity.clock)?;
    push_u64(&mut output, scan.validity.stamp_tick);
    push_u64(&mut output, scan.validity.valid_from_tick);
    push_u64(&mut output, scan.validity.valid_until_tick);
    output.extend_from_slice(&scan.calibration);
    push_u64(
        &mut output,
        u64::try_from(scan.points.len()).map_err(|_| SpatialDataReason::PointOverflow)?,
    );
    for point in &scan.points {
        for coordinate in point.xyz_um {
            output.extend_from_slice(&coordinate.to_le_bytes());
        }
        push_u64(&mut output, point.uncertainty_um);
    }
    if output.len() > MAXIMUM_SPATIAL_DATA_BYTES {
        return Err(SpatialDataReason::ByteOverflow);
    }
    Ok(output)
}

fn decode_scan(bytes: &[u8]) -> Result<RangeScan, SpatialDataReason> {
    if !bytes.starts_with(b"SDS0") || bytes.len() > MAXIMUM_SPATIAL_DATA_BYTES {
        return Err(SpatialDataReason::WrongType);
    }
    let mut cursor = 4;
    let frame = FrameIdentity::new(take_text(bytes, &mut cursor)?).map_err(map_spatial_reason)?;
    let clock = take_text(bytes, &mut cursor)?;
    let stamp_tick = take_u64(bytes, &mut cursor)?;
    let valid_from_tick = take_u64(bytes, &mut cursor)?;
    let valid_until_tick = take_u64(bytes, &mut cursor)?;
    let calibration_end = cursor
        .checked_add(32)
        .ok_or(SpatialDataReason::ByteOverflow)?;
    let calibration = bytes
        .get(cursor..calibration_end)
        .ok_or(SpatialDataReason::WrongType)?
        .try_into()
        .map_err(|_| SpatialDataReason::WrongType)?;
    cursor = calibration_end;
    let count = usize::try_from(take_u64(bytes, &mut cursor)?)
        .map_err(|_| SpatialDataReason::PointOverflow)?;
    if count > MAXIMUM_SCAN_POINTS {
        return Err(SpatialDataReason::PointOverflow);
    }
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        let mut xyz_um = [0_i64; 3];
        for coordinate in &mut xyz_um {
            let end = cursor
                .checked_add(8)
                .ok_or(SpatialDataReason::ByteOverflow)?;
            *coordinate = i64::from_le_bytes(
                bytes
                    .get(cursor..end)
                    .ok_or(SpatialDataReason::WrongType)?
                    .try_into()
                    .map_err(|_| SpatialDataReason::WrongType)?,
            );
            cursor = end;
        }
        points.push(ScanPoint {
            xyz_um,
            uncertainty_um: take_u64(bytes, &mut cursor)?,
        });
    }
    if cursor != bytes.len() {
        return Err(SpatialDataReason::WrongType);
    }
    Ok(RangeScan {
        schema: SCAN_SCHEMA_IDENTITY,
        schema_version: 0,
        snapshot: SNAPSHOT_IDENTITY,
        representation: REPRESENTATION_IDENTITY,
        provider: PROVIDER_IDENTITY,
        provenance: DATA_PROVENANCE_IDENTITY,
        frame,
        validity: Validity {
            clock,
            stamp_tick,
            valid_from_tick,
            valid_until_tick,
        },
        calibration,
        coverage_complete: true,
        points,
    })
}

fn encode_grid(grid: &OccupancyGrid) -> Result<Vec<u8>, SpatialDataReason> {
    let mut output = b"SDG0".to_vec();
    push_text(&mut output, &grid.frame.id)?;
    push_u64(
        &mut output,
        u64::try_from(grid.width).map_err(|_| SpatialDataReason::GridOverflow)?,
    );
    push_u64(
        &mut output,
        u64::try_from(grid.height).map_err(|_| SpatialDataReason::GridOverflow)?,
    );
    push_u64(&mut output, grid.resolution_um);
    output.extend_from_slice(&grid.cells);
    Ok(output)
}

fn decode_grid(bytes: &[u8]) -> Result<OccupancyGrid, SpatialDataReason> {
    if !bytes.starts_with(b"SDG0") || bytes.len() > MAXIMUM_SPATIAL_DATA_BYTES {
        return Err(SpatialDataReason::WrongType);
    }
    let mut cursor = 4;
    let frame = FrameIdentity::new(take_text(bytes, &mut cursor)?).map_err(map_spatial_reason)?;
    let width = usize::try_from(take_u64(bytes, &mut cursor)?)
        .map_err(|_| SpatialDataReason::GridOverflow)?;
    let height = usize::try_from(take_u64(bytes, &mut cursor)?)
        .map_err(|_| SpatialDataReason::GridOverflow)?;
    let resolution_um = take_u64(bytes, &mut cursor)?;
    let count = width
        .checked_mul(height)
        .ok_or(SpatialDataReason::GridOverflow)?;
    if count > MAXIMUM_GRID_CELLS || bytes.len() != cursor + count {
        return Err(SpatialDataReason::GridOverflow);
    }
    Ok(OccupancyGrid {
        schema: GRID_SCHEMA_IDENTITY,
        schema_version: 0,
        snapshot: SNAPSHOT_IDENTITY,
        representation: REPRESENTATION_IDENTITY,
        provider: PROVIDER_IDENTITY,
        provenance: DATA_PROVENANCE_IDENTITY,
        frame,
        validity: Validity {
            clock: "clock/fixture".into(),
            stamp_tick: 10,
            valid_from_tick: 0,
            valid_until_tick: 20,
        },
        calibration: crate::CALIBRATION_IDENTITY,
        coverage_complete: true,
        width,
        height,
        resolution_um,
        cells: bytes[cursor..].to_vec(),
    })
}

fn encode_trajectory(trajectory: &Trajectory) -> Result<Vec<u8>, SpatialDataReason> {
    validate_trajectory(trajectory, SpatialDataLimits::FIRST_PROOF)?;
    let mut output = b"SDT0".to_vec();
    push_text(&mut output, &trajectory.frame.id)?;
    push_text(&mut output, &trajectory.clock)?;
    push_text(&mut output, trajectory.interpolation)?;
    push_u64(
        &mut output,
        u64::try_from(trajectory.poses.len()).map_err(|_| SpatialDataReason::TrajectoryOverflow)?,
    );
    for stamped in &trajectory.poses {
        for coordinate in stamped.pose.translation_um {
            output.extend_from_slice(&coordinate.to_le_bytes());
        }
        push_u64(&mut output, stamped.validity.stamp_tick);
        push_u64(&mut output, stamped.validity.valid_from_tick);
        push_u64(&mut output, stamped.validity.valid_until_tick);
    }
    if output.len() > MAXIMUM_SPATIAL_DATA_BYTES {
        return Err(SpatialDataReason::ByteOverflow);
    }
    Ok(output)
}

fn decode_trajectory(bytes: &[u8]) -> Result<Trajectory, SpatialDataReason> {
    if !bytes.starts_with(b"SDT0") || bytes.len() > MAXIMUM_SPATIAL_DATA_BYTES {
        return Err(SpatialDataReason::WrongType);
    }
    let mut cursor = 4;
    let frame = FrameIdentity::new(take_text(bytes, &mut cursor)?).map_err(map_spatial_reason)?;
    let clock = take_text(bytes, &mut cursor)?;
    let interpolation = take_text(bytes, &mut cursor)?;
    if interpolation != "linear-q30-shortest" {
        return Err(SpatialDataReason::RepresentationMismatch);
    }
    let count = usize::try_from(take_u64(bytes, &mut cursor)?)
        .map_err(|_| SpatialDataReason::TrajectoryOverflow)?;
    if count > MAXIMUM_TRAJECTORY_POSES {
        return Err(SpatialDataReason::TrajectoryOverflow);
    }
    let mut poses = Vec::with_capacity(count);
    for _ in 0..count {
        let mut translation_um = [0_i64; 3];
        for coordinate in &mut translation_um {
            let end = cursor
                .checked_add(8)
                .ok_or(SpatialDataReason::ByteOverflow)?;
            *coordinate = i64::from_le_bytes(
                bytes
                    .get(cursor..end)
                    .ok_or(SpatialDataReason::WrongType)?
                    .try_into()
                    .map_err(|_| SpatialDataReason::WrongType)?,
            );
            cursor = end;
        }
        let stamp_tick = take_u64(bytes, &mut cursor)?;
        let valid_from_tick = take_u64(bytes, &mut cursor)?;
        let valid_until_tick = take_u64(bytes, &mut cursor)?;
        poses.push(crate::StampedPose3 {
            pose: crate::Pose3 {
                frame: frame.clone(),
                translation_um,
                rotation: crate::QuaternionQ30::IDENTITY,
                uncertainty: crate::Uncertainty::EXACT,
                calibration_identity: crate::CALIBRATION_IDENTITY,
                provenance_identity: crate::PROVENANCE_IDENTITY,
            },
            validity: Validity {
                clock: clock.clone(),
                stamp_tick,
                valid_from_tick,
                valid_until_tick,
            },
        });
    }
    if cursor != bytes.len() {
        return Err(SpatialDataReason::WrongType);
    }
    let trajectory = Trajectory {
        schema: TRAJECTORY_SCHEMA_IDENTITY,
        schema_version: 0,
        snapshot: SNAPSHOT_IDENTITY,
        frame,
        clock,
        interpolation: "linear-q30-shortest",
        poses,
    };
    validate_trajectory(&trajectory, SpatialDataLimits::FIRST_PROOF)?;
    Ok(trajectory)
}

fn fixture_chunks() -> Result<[ScanChunk; 2], SpatialDataReason> {
    let frame = FrameIdentity::new("sensor").map_err(map_spatial_reason)?;
    let validity = Validity {
        clock: "clock/fixture".into(),
        stamp_tick: 10,
        valid_from_tick: 0,
        valid_until_tick: 20,
    };
    let base = |chunk_index, points| ScanChunk {
        schema: SCAN_SCHEMA_IDENTITY,
        schema_version: 0,
        snapshot: SNAPSHOT_IDENTITY,
        representation: REPRESENTATION_IDENTITY,
        provider: PROVIDER_IDENTITY,
        provenance: DATA_PROVENANCE_IDENTITY,
        frame: frame.clone(),
        validity: validity.clone(),
        calibration: crate::CALIBRATION_IDENTITY,
        chunk_index,
        chunk_count: 2,
        points,
    };
    Ok([
        base(
            0,
            vec![ScanPoint {
                xyz_um: [0, 0, 1_000],
                uncertainty_um: 0,
            }],
        ),
        base(
            1,
            vec![ScanPoint {
                xyz_um: [1_000, 1_000, 1_000],
                uncertainty_um: 0,
            }],
        ),
    ])
}

struct ScanFixture;
impl Handler for ScanFixture {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(SpatialDataReason::WrongType));
        }
        let scan = normalize_scan_chunks(
            &fixture_chunks().map_err(runtime)?,
            SpatialDataLimits::FIRST_PROOF,
            false,
        )
        .map_err(runtime)?;
        Ok(vec![Value {
            value_type: SCAN_TYPE,
            bytes: encode_scan(&scan).map_err(runtime)?,
        }])
    }
}
struct ScanTransform;
impl Handler for ScanTransform {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [scan, transform] = inputs else {
            return Err(runtime(SpatialDataReason::WrongType));
        };
        if scan.value_type != SCAN_TYPE || transform.value_type != crate::TRANSFORM_TYPE {
            return Err(runtime(SpatialDataReason::WrongType));
        }
        let scan = decode_scan(&scan.bytes).map_err(runtime)?;
        let transform = crate::runtime_nodes::decode_transform(&transform.bytes)
            .map_err(|reason| runtime(map_spatial_reason(reason)))?;
        let transformed =
            transform_scan(&scan, &transform, SpatialDataLimits::FIRST_PROOF).map_err(runtime)?;
        Ok(vec![Value {
            value_type: SCAN_TYPE,
            bytes: encode_scan(&transformed).map_err(runtime)?,
        }])
    }
}
struct GridFromScan;
impl Handler for GridFromScan {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [scan] = inputs else {
            return Err(runtime(SpatialDataReason::WrongType));
        };
        if scan.value_type != SCAN_TYPE {
            return Err(runtime(SpatialDataReason::WrongType));
        }
        let grid = grid_from_scan(
            &decode_scan(&scan.bytes).map_err(runtime)?,
            2,
            2,
            1000,
            SpatialDataLimits::FIRST_PROOF,
        )
        .map_err(runtime)?;
        Ok(vec![Value {
            value_type: GRID_TYPE,
            bytes: encode_grid(&grid).map_err(runtime)?,
        }])
    }
}
struct GridInspect;
impl Handler for GridInspect {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [grid] = inputs else {
            return Err(runtime(SpatialDataReason::WrongType));
        };
        if grid.value_type != GRID_TYPE {
            return Err(runtime(SpatialDataReason::WrongType));
        }
        let grid = decode_grid(&grid.bytes).map_err(runtime)?;
        let occupied = grid.cells.iter().filter(|cell| **cell != 0).count();
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: format!(
                "spatial:grid:{}:{}x{}:occupied={occupied}:coverage=complete",
                grid.frame.id, grid.width, grid.height
            )
            .into_bytes(),
        }])
    }
}

struct TrajectoryFixture;
impl Handler for TrajectoryFixture {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(SpatialDataReason::WrongType));
        }
        let frame = FrameIdentity::new("map")
            .map_err(map_spatial_reason)
            .map_err(runtime)?;
        let poses = [10_u64, 12]
            .into_iter()
            .map(|tick| crate::StampedPose3 {
                pose: crate::Pose3 {
                    frame: frame.clone(),
                    translation_um: [i64::try_from(tick - 10).unwrap_or(0) * 1000, 0, 0],
                    rotation: crate::QuaternionQ30::IDENTITY,
                    uncertainty: crate::Uncertainty::EXACT,
                    calibration_identity: crate::CALIBRATION_IDENTITY,
                    provenance_identity: crate::PROVENANCE_IDENTITY,
                },
                validity: Validity {
                    clock: "clock/fixture".into(),
                    stamp_tick: tick,
                    valid_from_tick: 10,
                    valid_until_tick: 12,
                },
            })
            .collect();
        let trajectory = Trajectory {
            schema: TRAJECTORY_SCHEMA_IDENTITY,
            schema_version: 0,
            snapshot: SNAPSHOT_IDENTITY,
            frame,
            clock: "clock/fixture".into(),
            interpolation: "linear-q30-shortest",
            poses,
        };
        Ok(vec![Value {
            value_type: TRAJECTORY_TYPE,
            bytes: encode_trajectory(&trajectory).map_err(runtime)?,
        }])
    }
}

struct TrajectoryInspect;
impl Handler for TrajectoryInspect {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [trajectory] = inputs else {
            return Err(runtime(SpatialDataReason::WrongType));
        };
        if trajectory.value_type != TRAJECTORY_TYPE {
            return Err(runtime(SpatialDataReason::WrongType));
        }
        let trajectory = decode_trajectory(&trajectory.bytes).map_err(runtime)?;
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: format!(
                "spatial:trajectory:{}:{}:{}:{}",
                trajectory.frame.id,
                trajectory.poses.len(),
                trajectory.clock,
                trajectory.interpolation
            )
            .into_bytes(),
        }])
    }
}

pub fn register_deterministic_spatial_data_provider(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_spatial_data_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validator) in [
        (
            &SCAN_FIXTURE_CONTRACT,
            "conduit.spatial/scan-fixture",
            "conduit.spatial/scan-fixture-artifact",
            "spatial-scan-fixture",
            (|| Box::new(ScanFixture) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_fixture_config as conduit_runtime::ConfigValidator,
        ),
        (
            &SCAN_TRANSFORM_CONTRACT,
            "conduit.spatial/scan-transform-reference",
            "conduit.spatial/scan-transform-reference-artifact",
            "spatial-scan-transform",
            (|| Box::new(ScanTransform) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_transform_config as conduit_runtime::ConfigValidator,
        ),
        (
            &GRID_FROM_SCAN_CONTRACT,
            "conduit.spatial/grid-from-scan-reference",
            "conduit.spatial/grid-from-scan-reference-artifact",
            "spatial-grid-from-scan",
            (|| Box::new(GridFromScan) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_grid_config as conduit_runtime::ConfigValidator,
        ),
        (
            &GRID_INSPECT_CONTRACT,
            "conduit.spatial/grid-inspect",
            "conduit.spatial/grid-inspect-artifact",
            "spatial-grid-inspect",
            (|| Box::new(GridInspect) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            no_config as conduit_runtime::ConfigValidator,
        ),
        (
            &TRAJECTORY_FIXTURE_CONTRACT,
            "conduit.spatial/trajectory-fixture",
            "conduit.spatial/trajectory-fixture-artifact",
            "spatial-trajectory-fixture",
            (|| Box::new(TrajectoryFixture) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_trajectory_config as conduit_runtime::ConfigValidator,
        ),
        (
            &TRAJECTORY_INSPECT_CONTRACT,
            "conduit.spatial/trajectory-inspect",
            "conduit.spatial/trajectory-inspect-artifact",
            "spatial-trajectory-inspect",
            (|| Box::new(TrajectoryInspect) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            no_config as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("data.rs"),
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
    use crate::Uncertainty;

    fn limits() -> SpatialDataLimits {
        SpatialDataLimits::FIRST_PROOF
    }

    fn transform() -> Transform3 {
        Transform3 {
            source: FrameIdentity::new("sensor").unwrap(),
            target: FrameIdentity::new("map").unwrap(),
            translation_um: [0; 3],
            rotation: crate::QuaternionQ30::IDENTITY,
            quarter_turns_z: 0,
            validity: Validity {
                clock: "clock/fixture".into(),
                stamp_tick: 10,
                valid_from_tick: 0,
                valid_until_tick: 20,
            },
            uncertainty: Uncertainty::EXACT,
            calibration_identity: crate::CALIBRATION_IDENTITY,
            provenance_identity: crate::PROVENANCE_IDENTITY,
        }
    }

    #[test]
    fn chunking_normalizes_without_changing_scan_semantics() {
        let chunks = fixture_chunks().unwrap();
        let split = normalize_scan_chunks(&chunks, limits(), false).unwrap();
        let one = ScanChunk {
            chunk_index: 0,
            chunk_count: 1,
            points: chunks
                .iter()
                .flat_map(|chunk| chunk.points.clone())
                .collect(),
            ..chunks[0].clone()
        };
        let coalesced = normalize_scan_chunks(&[one], limits(), false).unwrap();
        assert_eq!(split, coalesced);
        assert_eq!(decode_scan(&encode_scan(&split).unwrap()).unwrap(), split);
    }

    #[test]
    fn exact_scan_transform_grid_path_is_finite() {
        let scan = normalize_scan_chunks(&fixture_chunks().unwrap(), limits(), false).unwrap();
        let transformed = transform_scan(&scan, &transform(), limits()).unwrap();
        assert_eq!(transformed.frame.id, "map");
        let grid = grid_from_scan(&transformed, 2, 2, 1000, limits()).unwrap();
        assert_eq!(grid.cells, vec![255, 0, 0, 255]);
        assert_eq!(
            decode_grid(&encode_grid(&grid).unwrap()).unwrap().cells,
            grid.cells
        );
    }

    #[test]
    fn schema_snapshot_provider_frame_time_calibration_and_coverage_fail_closed() {
        let chunks = fixture_chunks().unwrap();
        for (mutation, expected) in [
            (0_u8, SpatialDataReason::SchemaMismatch),
            (1, SpatialDataReason::SnapshotMismatch),
            (2, SpatialDataReason::RepresentationMismatch),
            (3, SpatialDataReason::ProviderUnavailable),
            (4, SpatialDataReason::WrongUnit),
            (5, SpatialDataReason::StaleTransform),
        ] {
            let mut changed = chunks.clone();
            match mutation {
                0 => changed[0].schema[0] ^= 1,
                1 => changed[0].snapshot[0] ^= 1,
                2 => changed[0].representation[0] ^= 1,
                3 => changed[0].provider[0] ^= 1,
                4 => changed[0].frame.unit = LinearUnit::Millimetre,
                5 => changed[0].validity.valid_until_tick = 9,
                _ => unreachable!(),
            }
            assert_eq!(
                normalize_scan_chunks(&changed, limits(), false),
                Err(expected)
            );
        }
        assert_eq!(
            normalize_scan_chunks(&chunks[..1], limits(), false),
            Err(SpatialDataReason::PartialCoverage)
        );
        assert_eq!(
            normalize_scan_chunks(&chunks, limits(), true),
            Err(SpatialDataReason::Cancellation)
        );

        let mut scan = normalize_scan_chunks(&chunks, limits(), false).unwrap();
        scan.coverage_complete = false;
        assert_eq!(
            grid_from_scan(&scan, 2, 2, 1000, limits()),
            Err(SpatialDataReason::PartialCoverage)
        );
        let mut bad = transform();
        bad.calibration_identity[0] ^= 1;
        scan.coverage_complete = true;
        assert_eq!(
            transform_scan(&scan, &bad, limits()),
            Err(SpatialDataReason::CalibrationMismatch)
        );
    }

    #[test]
    fn chunk_order_uncertainty_and_every_finite_dimension_fail_closed() {
        let chunks = fixture_chunks().unwrap();
        let mut gap = chunks.clone();
        gap[1].chunk_index = 2;
        assert_eq!(
            normalize_scan_chunks(&gap, limits(), false),
            Err(SpatialDataReason::ChunkGap)
        );
        let mut reordered = chunks.clone();
        reordered[1].chunk_index = 0;
        assert_eq!(
            normalize_scan_chunks(&reordered, limits(), false),
            Err(SpatialDataReason::ChunkReordered)
        );
        let mut uncertain = chunks.clone();
        uncertain[0].points[0].uncertainty_um = 11;
        assert_eq!(
            normalize_scan_chunks(&uncertain, limits(), false),
            Err(SpatialDataReason::ExcessiveUncertainty)
        );

        for (changed, expected) in [
            (
                SpatialDataLimits {
                    maximum_points: 0,
                    ..limits()
                },
                SpatialDataReason::PointOverflow,
            ),
            (
                SpatialDataLimits {
                    maximum_chunks: 0,
                    ..limits()
                },
                SpatialDataReason::ChunkGap,
            ),
            (
                SpatialDataLimits {
                    maximum_grid_cells: 0,
                    ..limits()
                },
                SpatialDataReason::GridOverflow,
            ),
            (
                SpatialDataLimits {
                    maximum_trajectory_poses: 0,
                    ..limits()
                },
                SpatialDataReason::TrajectoryOverflow,
            ),
            (
                SpatialDataLimits {
                    maximum_bytes: 0,
                    ..limits()
                },
                SpatialDataReason::ByteOverflow,
            ),
            (
                SpatialDataLimits {
                    maximum_work: 0,
                    ..limits()
                },
                SpatialDataReason::WorkOverflow,
            ),
        ] {
            assert_eq!(
                normalize_scan_chunks(&chunks, changed, false),
                Err(expected)
            );
        }
        let scan = normalize_scan_chunks(&chunks, limits(), false).unwrap();
        assert_eq!(
            grid_from_scan(&scan, 5, 5, 1000, limits()),
            Err(SpatialDataReason::GridOverflow)
        );
    }

    #[test]
    fn trajectory_identity_interpolation_frame_clock_and_history_are_exact() {
        let stamped = crate::StampedPose3 {
            pose: crate::Pose3 {
                frame: FrameIdentity::new("map").unwrap(),
                translation_um: [0; 3],
                rotation: crate::QuaternionQ30::IDENTITY,
                uncertainty: Uncertainty::EXACT,
                calibration_identity: crate::CALIBRATION_IDENTITY,
                provenance_identity: crate::PROVENANCE_IDENTITY,
            },
            validity: Validity {
                clock: "clock/fixture".into(),
                stamp_tick: 10,
                valid_from_tick: 0,
                valid_until_tick: 20,
            },
        };
        let trajectory = Trajectory {
            schema: TRAJECTORY_SCHEMA_IDENTITY,
            schema_version: 0,
            snapshot: SNAPSHOT_IDENTITY,
            frame: FrameIdentity::new("map").unwrap(),
            clock: "clock/fixture".into(),
            interpolation: "linear-q30-shortest",
            poses: vec![stamped],
        };
        assert_eq!(validate_trajectory(&trajectory, limits()), Ok(()));
        assert_eq!(
            decode_trajectory(&encode_trajectory(&trajectory).unwrap()).unwrap(),
            trajectory
        );
        let mut wrong = trajectory.clone();
        wrong.schema[0] ^= 1;
        assert_eq!(
            validate_trajectory(&wrong, limits()),
            Err(SpatialDataReason::SchemaMismatch)
        );
        let mut wrong = trajectory.clone();
        wrong.interpolation = "ambient";
        assert_eq!(
            validate_trajectory(&wrong, limits()),
            Err(SpatialDataReason::RepresentationMismatch)
        );
        let mut wrong = trajectory;
        wrong.poses[0].validity.clock = "clock/other".into();
        assert_eq!(
            validate_trajectory(&wrong, limits()),
            Err(SpatialDataReason::WrongClock)
        );
    }

    #[test]
    fn contracts_do_not_install_a_scan_or_map_provider() {
        let mut registry = Registry::default();
        register_spatial_data_contracts(&mut registry);
        for contract in SPATIAL_DATA_CONTRACTS {
            assert_eq!(
                registry.node_availability(contract.id.as_str()).state,
                conduit_runtime::AvailabilityState::ContractOnly
            );
        }
    }

    #[test]
    fn conformance_fixture_names_the_complete_spatial_data_matrix() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/c4/spatial-data.json"))
                .unwrap();
        assert_eq!(fixture["schema"], "conduit.spatial-data-conformance");
        assert_eq!(fixture["schema_version"], 0);
        assert_eq!(fixture["positive_cases"].as_array().unwrap().len(), 4);
        let reasons = fixture["negative_cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| case["reason"].as_str().unwrap().to_owned())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            reasons,
            (1..=12)
                .map(|number| format!("CND-SPATIAL-DATA-{number:03}"))
                .collect::<std::collections::BTreeSet<_>>()
        );
        let cases = fixture["negative_cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| case["id"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        for required in [
            "oversized-cloud",
            "oversized-grid",
            "wrong-frame-unit-time",
            "calibration-mismatch",
            "missing-transform",
            "stale-transform",
            "partial-map-coverage",
            "chunk-reordering-or-gap",
            "uncertainty-threshold",
            "pressure",
            "cancellation",
            "unsupported-provider",
        ] {
            assert!(cases.contains(required), "fixture covers {required}");
        }
        assert_eq!(
            fixture["negative_cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["id"] == "missing-transform")
                .unwrap()["resolution_reason"],
            "CND-PRT-002"
        );
    }
}
