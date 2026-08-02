//! Host-neutral, effect-free robotics profile descriptions.
//!
//! This package owns domain semantics and validation only. Implementations,
//! artifacts, host observations, exact bindings, authority, run evidence, and
//! presentation remain separate. It deliberately contains no robot driver,
//! device handle, discovery, resolver, scheduler, possession service, or
//! actuation path.

use conduit_core::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition,
    HOST_CONFORMANCE_PROFILE_SCHEMA_VERSION, HostClass, HostConformanceProfile,
    HostConformanceReason, HostExecutionMode, HostExtension, HostExtensionKind, Id, MapField,
    PinnedDescriptor, ProviderInventory, ProviderInventoryState, SemanticHash,
    validate_host_conformance_profile,
};
use core::convert::Infallible;

pub const ROBOTICS_PROFILE_SCHEMA_VERSION: u32 = 0;
pub const MAXIMUM_PROFILE_TYPES: usize = 8;
pub const MAXIMUM_PROFILE_QUANTITIES: usize = 16;
pub const MAXIMUM_LOGICAL_RELATIONSHIPS: usize = 8;
pub const MAXIMUM_PROVIDER_STATES: usize = 8;
pub const MAXIMUM_CARRIER_CANDIDATES: usize = 8;
pub const MAXIMUM_PATH_OBSERVATIONS: usize = 8;

pub const ROBOTICS_PROFILE_CONTRACT: PinnedDescriptor<'static> = pin(
    "conduit.robotics/profile",
    hash_bytes("8186fdd2be75eb23f4343f15b82a336bad951c19c803308162d150b45f67fd1e"),
);
pub const LINUX_IMPLEMENTATION: PinnedDescriptor<'static> = pin(
    "netherwick/implementation/pete-linux-robotics-describe",
    hash_bytes("ca1176529b26599dfbcc16d5b9155ef979274d2bb3eccb6540375f1ec7e414a5"),
);
pub const PICO_IMPLEMENTATION: PinnedDescriptor<'static> = pin(
    "netherwick/implementation/pete-pico-robotics-describe",
    hash_bytes("9fb84fad0d18e673a7de4ed2c2676d4adedb210ffb6c9f57b53f82b965ae60ea"),
);
pub const LINUX_ARTIFACT: PinnedDescriptor<'static> = pin(
    "netherwick/artifact/pete-brainstem-linux",
    hash_bytes("5f661f42416ce1c99ad008da5242d6dbe1ac9277b794c07e2bf490a09d2b284f"),
);
pub const PICO_ARTIFACT: PinnedDescriptor<'static> = pin(
    "netherwick/artifact/pete-brainstem-pico-w",
    hash_bytes("76a9f275e22a56a2ed9be1ab59091d756d0509ba0f6d9d2396b60b145a2fef3e"),
);
pub const LINUX_PROVIDER_BUNDLE: PinnedDescriptor<'static> = pin(
    "netherwick/provider/pete-linux-robotics-describe",
    hash_bytes("a09ded7cbcce5bddee388041d6425f0ea674d1b92254c4f1aa7141fbf0dceeae"),
);
pub const PICO_PROVIDER_BUNDLE: PinnedDescriptor<'static> = pin(
    "netherwick/provider/pete-pico-robotics-describe",
    hash_bytes("d044a691cf807732418c4dd8517dc378c6a95fcf0cfecccc3e00aaa99e0595cb"),
);
pub const DESCRIBE_ADAPTER: PinnedDescriptor<'static> = pin(
    "conduit.adapter/robotics-profile-describe",
    hash_bytes("a446ad531079a0f5e8622302ed6f59592d2594b4eb5ed314060f09fede2f4c51"),
);

pub const OBSERVATION_TYPE: PinnedDescriptor<'static> = pin(
    "conduit.robotics/observation",
    hash_bytes("f9a5f7a64f9c590d32e9e768ebf85c99f403b1e7b5fa6f818ad00ad626b3a61d"),
);
pub const COMMAND_TYPE: PinnedDescriptor<'static> = pin(
    "conduit.robotics/command",
    hash_bytes("91b6e322d47863cde4769c1173a601bd41d1128f246266105227812dc19f4b79"),
);
pub const ACKNOWLEDGEMENT_TYPE: PinnedDescriptor<'static> = pin(
    "conduit.robotics/acknowledgement",
    hash_bytes("5ca5190fc94997c45fe7ccf437f71a105609deeff6deef08fbcec1185e355981"),
);
pub const SAFE_OUTCOME_TYPE: PinnedDescriptor<'static> = pin(
    "conduit.robotics/safe-outcome",
    hash_bytes("a66a4fdc328f6e1ce01e2ee72a5001cd67e6c0685b0217122ee6033650ec6a4a"),
);
pub const POSSESSION_TYPE: PinnedDescriptor<'static> = pin(
    "conduit.robotics/possession",
    hash_bytes("5ea91a556e18dbd01790acb97ee3635f0dd439939dfd3bb871f889b834873806"),
);
pub const TERMINAL_TYPE: PinnedDescriptor<'static> = pin(
    "conduit.robotics/terminal",
    hash_bytes("d73fc8ee718b65c13c6b42c09d764ed43c176c436e8c729dd161e86992da6dae"),
);
pub const FAULT_TYPE: PinnedDescriptor<'static> = pin(
    "conduit.robotics/fault",
    hash_bytes("1c129536beafc20aac526f0ef48088e0fa57d5c730a3554c85a0c558165efbff"),
);

pub const MOTION_AUTHORITY: PinnedDescriptor<'static> = pin(
    "netherwick/authority/motion",
    hash_bytes("94810623f6037b0aaaddb80f2c3777120ccf7353c12d0502b18427481e930f91"),
);
pub const POSSESSION_REQUIREMENT: PinnedDescriptor<'static> = pin(
    "netherwick/requirement/current-possession",
    hash_bytes("0b0be9aa9a60fe9a188ba47f8f8684640cc355aa85ef0e4c9501fd7a5addab6c"),
);
pub const STOP_REQUIREMENT: PinnedDescriptor<'static> = pin(
    "netherwick/requirement/bounded-stop",
    hash_bytes("0cb18d7af1e8bc0bac2cec9038cc519a2bc3c614e26d9ca349996ba5b8961eef"),
);
pub const ESTOP_REQUIREMENT: PinnedDescriptor<'static> = pin(
    "netherwick/requirement/emergency-stop",
    hash_bytes("66b8180569a824d08ac3194540579995fb9027dd6e91f4ef9f3e35124d63c105"),
);
pub const CHARGING_REQUIREMENT: PinnedDescriptor<'static> = pin(
    "netherwick/requirement/not-charging",
    hash_bytes("18e90f33eab66ae8ead2a9430b35c40aadbcf381463eee9c5b60ce1ee8d3149d"),
);
pub const INTERLOCK_REQUIREMENT: PinnedDescriptor<'static> = pin(
    "netherwick/requirement/charging-interlock",
    hash_bytes("81e97b15ae9b810cb1b3b381c6ee1df8bb5e34d16323d4a35f3c885948fd7404"),
);
pub const INHIBIT_REQUIREMENT: PinnedDescriptor<'static> = pin(
    "netherwick/requirement/safety-inhibit-clear",
    hash_bytes("0ee0899ae2f028c92c6494dc0b1893bd89b393ca211ef9560d6d53ed252acf57"),
);
pub const MOTION_CAPABILITY: PinnedDescriptor<'static> = pin(
    "netherwick/capability/create-motion",
    hash_bytes("ff3f11b1f6b4ba0be351769088db52eae257fbd3c71eb050975ce15a8a78acd1"),
);

const ZERO_HASH: SemanticHash = SemanticHash::from_bytes([0; 32]);

const fn pin(id: &'static str, bytes: [u8; 32]) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes(bytes),
    }
}

const fn hash_bytes(hex: &str) -> [u8; 32] {
    let source = hex.as_bytes();
    assert!(
        source.len() == 64,
        "semantic hash must contain 64 hexadecimal digits"
    );
    let mut bytes = [0_u8; 32];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = (hex_nibble(source[index * 2]) << 4) | hex_nibble(source[index * 2 + 1]);
        index += 1;
    }
    bytes
}

const fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("semantic hash must use lowercase hexadecimal digits"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoboticsValueRole {
    Observation,
    Command,
    Acknowledgement,
    SafeOutcome,
    Possession,
    Terminal,
    Fault,
}

impl RoboticsValueRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Command => "command",
            Self::Acknowledgement => "acknowledgement",
            Self::SafeOutcome => "safe-outcome",
            Self::Possession => "possession",
            Self::Terminal => "terminal",
            Self::Fault => "fault",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoboticsValueType<'a> {
    pub role: RoboticsValueRole,
    pub descriptor: PinnedDescriptor<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantityContract<'a> {
    pub id: Id<'a>,
    pub units: Id<'a>,
    pub frame: Id<'a>,
    pub clock: Id<'a>,
    pub uncertainty_bound: u64,
    pub maximum_age_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SafetyEnvelope<'a> {
    pub clock: Id<'a>,
    pub command_ttl_ticks: u64,
    pub maximum_linear_velocity_mm_per_second: u32,
    pub maximum_angular_velocity_milliradians_per_second: u32,
    pub maximum_command_queue: u16,
    pub possession: PinnedDescriptor<'a>,
    pub motion_authority: PinnedDescriptor<'a>,
    pub stop: PinnedDescriptor<'a>,
    pub emergency_stop: PinnedDescriptor<'a>,
    pub not_charging: PinnedDescriptor<'a>,
    pub charging_interlock: PinnedDescriptor<'a>,
    pub inhibit_clear: PinnedDescriptor<'a>,
    pub capability: PinnedDescriptor<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalRelationship<'a> {
    pub id: Id<'a>,
    pub source_entity: Id<'a>,
    pub target_entity: Id<'a>,
    pub role: Id<'a>,
    pub allowed_carriers: &'a [Id<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoboticsProfile<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub value_types: &'a [RoboticsValueType<'a>],
    pub quantities: &'a [QuantityContract<'a>],
    pub safety: SafetyEnvelope<'a>,
    pub logical_relationships: &'a [LogicalRelationship<'a>],
    pub role_descriptor: PinnedDescriptor<'a>,
    pub checkpoint_descriptor: PinnedDescriptor<'a>,
    pub redaction_policy: PinnedDescriptor<'a>,
}

const VALUE_TYPES: [RoboticsValueType<'static>; 7] = [
    RoboticsValueType {
        role: RoboticsValueRole::Observation,
        descriptor: OBSERVATION_TYPE,
    },
    RoboticsValueType {
        role: RoboticsValueRole::Command,
        descriptor: COMMAND_TYPE,
    },
    RoboticsValueType {
        role: RoboticsValueRole::Acknowledgement,
        descriptor: ACKNOWLEDGEMENT_TYPE,
    },
    RoboticsValueType {
        role: RoboticsValueRole::SafeOutcome,
        descriptor: SAFE_OUTCOME_TYPE,
    },
    RoboticsValueType {
        role: RoboticsValueRole::Possession,
        descriptor: POSSESSION_TYPE,
    },
    RoboticsValueType {
        role: RoboticsValueRole::Terminal,
        descriptor: TERMINAL_TYPE,
    },
    RoboticsValueType {
        role: RoboticsValueRole::Fault,
        descriptor: FAULT_TYPE,
    },
];

const QUANTITIES: [QuantityContract<'static>; 8] = [
    quantity("linear-velocity", "mm-per-second", "create-body", 25, 250),
    quantity(
        "angular-velocity",
        "milliradians-per-second",
        "create-body",
        50,
        250,
    ),
    quantity("distance", "millimetres", "create-body", 10, 250),
    quantity("heading", "milliradians", "create-body", 25, 250),
    quantity(
        "acceleration",
        "millimetres-per-second-squared",
        "imu-mount",
        100,
        100,
    ),
    quantity("voltage", "millivolts", "electrical", 50, 1_000),
    quantity("current", "milliamperes", "electrical", 100, 1_000),
    quantity("charge", "milliampere-hours", "battery", 100, 5_000),
];

const fn quantity(
    id: &'static str,
    units: &'static str,
    frame: &'static str,
    uncertainty_bound: u64,
    maximum_age_ticks: u64,
) -> QuantityContract<'static> {
    QuantityContract {
        id: Id(id),
        units: Id(units),
        frame: Id(frame),
        clock: Id("brainstem-monotonic-milliseconds"),
        uncertainty_bound,
        maximum_age_ticks,
    }
}

const BRAINSTEM_CARRIERS: [Id<'static>; 3] = [Id("usb"), Id("ethernet"), Id("wifi")];
const RELATIONSHIPS: [LogicalRelationship<'static>; 2] = [
    LogicalRelationship {
        id: Id("motherbrain-to-brainstem"),
        source_entity: Id("netherwick/motherbrain"),
        target_entity: Id("netherwick/brainstem"),
        role: Id("command-and-observation"),
        allowed_carriers: &BRAINSTEM_CARRIERS,
    },
    LogicalRelationship {
        id: Id("brainstem-to-body"),
        source_entity: Id("netherwick/brainstem"),
        target_entity: Id("netherwick/create-body"),
        role: Id("create-oi-control"),
        allowed_carriers: &[Id("uart")],
    },
];

const SAFETY: SafetyEnvelope<'static> = SafetyEnvelope {
    clock: Id("brainstem-monotonic-milliseconds"),
    command_ttl_ticks: 250,
    maximum_linear_velocity_mm_per_second: 500,
    maximum_angular_velocity_milliradians_per_second: 2_000,
    maximum_command_queue: 1,
    possession: POSSESSION_REQUIREMENT,
    motion_authority: MOTION_AUTHORITY,
    stop: STOP_REQUIREMENT,
    emergency_stop: ESTOP_REQUIREMENT,
    not_charging: CHARGING_REQUIREMENT,
    charging_interlock: INTERLOCK_REQUIREMENT,
    inhibit_clear: INHIBIT_REQUIREMENT,
    capability: MOTION_CAPABILITY,
};

const ROLE_DESCRIPTOR: PinnedDescriptor<'static> = pin(
    "netherwick/role/katra-custodian",
    hash_bytes("57c20210c045280516b7ca66767fd4e88b88c1e848e2f98172359b9966e79b81"),
);
const CHECKPOINT_DESCRIPTOR: PinnedDescriptor<'static> = pin(
    "netherwick/checkpoint/organism-runtime-continuation",
    hash_bytes("1186ca536202312758e4449333dd138eda8268591aec5abae3c19df4ac88231c"),
);
const REDACTION_POLICY: PinnedDescriptor<'static> = pin(
    "netherwick/redaction/describe-only",
    hash_bytes("587eb8b0ad80945f40421b90aeb3fe9a97bf177ca42e2d6e003dc01d609961c4"),
);

#[must_use]
pub fn audited_profile() -> RoboticsProfile<'static> {
    let mut profile = RoboticsProfile {
        schema_version: ROBOTICS_PROFILE_SCHEMA_VERSION,
        identity: ZERO_HASH,
        id: Id("netherwick/pete-audited-robotics"),
        value_types: &VALUE_TYPES,
        quantities: &QUANTITIES,
        safety: SAFETY,
        logical_relationships: &RELATIONSHIPS,
        role_descriptor: ROLE_DESCRIPTOR,
        checkpoint_descriptor: CHECKPOINT_DESCRIPTOR,
        redaction_policy: REDACTION_POLICY,
    };
    profile.identity = profile
        .semantic_hash()
        .expect("built-in robotics profile is canonical");
    profile
}

impl RoboticsProfile<'_> {
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        if self.value_types.len() > MAXIMUM_PROFILE_TYPES
            || self.quantities.len() > MAXIMUM_PROFILE_QUANTITIES
            || self.logical_relationships.len() > MAXIMUM_LOGICAL_RELATIONSHIPS
            || self.logical_relationships.iter().any(|relationship| {
                relationship.allowed_carriers.len() > MAXIMUM_CARRIER_CANDIDATES
            })
        {
            return Err(CanonicalError::LengthOverflow);
        }
        let mut value_type_hashes = [ZERO_HASH; MAXIMUM_PROFILE_TYPES];
        for (index, value_type) in self.value_types.iter().copied().enumerate() {
            value_type_hashes[index] = hash_value_type(value_type)?;
        }
        let mut value_type_values =
            [CanonicalValue::Bytes(ZERO_HASH.as_bytes()); MAXIMUM_PROFILE_TYPES];
        for (index, hash) in value_type_hashes[..self.value_types.len()]
            .iter()
            .enumerate()
        {
            value_type_values[index] = CanonicalValue::Bytes(hash.as_bytes());
        }

        let mut quantity_hashes = [ZERO_HASH; MAXIMUM_PROFILE_QUANTITIES];
        for (index, quantity) in self.quantities.iter().copied().enumerate() {
            quantity_hashes[index] = hash_quantity(quantity)?;
        }
        let mut quantity_values =
            [CanonicalValue::Bytes(ZERO_HASH.as_bytes()); MAXIMUM_PROFILE_QUANTITIES];
        for (index, hash) in quantity_hashes[..self.quantities.len()].iter().enumerate() {
            quantity_values[index] = CanonicalValue::Bytes(hash.as_bytes());
        }

        let mut relationship_hashes = [ZERO_HASH; MAXIMUM_LOGICAL_RELATIONSHIPS];
        for (index, relationship) in self.logical_relationships.iter().copied().enumerate() {
            relationship_hashes[index] = hash_relationship(relationship)?;
        }
        let mut relationship_values =
            [CanonicalValue::Bytes(ZERO_HASH.as_bytes()); MAXIMUM_LOGICAL_RELATIONSHIPS];
        for (index, hash) in relationship_hashes[..self.logical_relationships.len()]
            .iter()
            .enumerate()
        {
            relationship_values[index] = CanonicalValue::Bytes(hash.as_bytes());
        }

        let possession = hash_pin("conduit.robotics/safety-possession", self.safety.possession)?;
        let motion_authority = hash_pin(
            "conduit.robotics/safety-motion-authority",
            self.safety.motion_authority,
        )?;
        let stop = hash_pin("conduit.robotics/safety-stop", self.safety.stop)?;
        let emergency_stop = hash_pin(
            "conduit.robotics/safety-emergency-stop",
            self.safety.emergency_stop,
        )?;
        let not_charging = hash_pin(
            "conduit.robotics/safety-not-charging",
            self.safety.not_charging,
        )?;
        let charging_interlock = hash_pin(
            "conduit.robotics/safety-charging-interlock",
            self.safety.charging_interlock,
        )?;
        let inhibit_clear = hash_pin(
            "conduit.robotics/safety-inhibit-clear",
            self.safety.inhibit_clear,
        )?;
        let capability = hash_pin("conduit.robotics/safety-capability", self.safety.capability)?;
        let role_descriptor = hash_pin("conduit.robotics/role", self.role_descriptor)?;
        let checkpoint_descriptor =
            hash_pin("conduit.robotics/checkpoint", self.checkpoint_descriptor)?;
        let redaction_policy = hash_pin("conduit.robotics/redaction", self.redaction_policy)?;
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic(
                "value_types",
                CanonicalValue::List(&value_type_values[..self.value_types.len()]),
            ),
            semantic(
                "quantities",
                CanonicalValue::List(&quantity_values[..self.quantities.len()]),
            ),
            semantic(
                "logical_relationships",
                CanonicalValue::List(&relationship_values[..self.logical_relationships.len()]),
            ),
            semantic("clock", CanonicalValue::Identifier(self.safety.clock)),
            semantic(
                "command_ttl_ticks",
                CanonicalValue::Integer(i128::from(self.safety.command_ttl_ticks)),
            ),
            semantic(
                "maximum_linear_velocity_mm_per_second",
                CanonicalValue::Integer(i128::from(
                    self.safety.maximum_linear_velocity_mm_per_second,
                )),
            ),
            semantic(
                "maximum_angular_velocity_milliradians_per_second",
                CanonicalValue::Integer(i128::from(
                    self.safety.maximum_angular_velocity_milliradians_per_second,
                )),
            ),
            semantic(
                "maximum_command_queue",
                CanonicalValue::Integer(i128::from(self.safety.maximum_command_queue)),
            ),
            semantic("possession", CanonicalValue::Bytes(possession.as_bytes())),
            semantic(
                "motion_authority",
                CanonicalValue::Bytes(motion_authority.as_bytes()),
            ),
            semantic("stop", CanonicalValue::Bytes(stop.as_bytes())),
            semantic(
                "emergency_stop",
                CanonicalValue::Bytes(emergency_stop.as_bytes()),
            ),
            semantic(
                "not_charging",
                CanonicalValue::Bytes(not_charging.as_bytes()),
            ),
            semantic(
                "charging_interlock",
                CanonicalValue::Bytes(charging_interlock.as_bytes()),
            ),
            semantic(
                "inhibit_clear",
                CanonicalValue::Bytes(inhibit_clear.as_bytes()),
            ),
            semantic("capability", CanonicalValue::Bytes(capability.as_bytes())),
            semantic(
                "role_descriptor",
                CanonicalValue::Bytes(role_descriptor.as_bytes()),
            ),
            semantic(
                "checkpoint_descriptor",
                CanonicalValue::Bytes(checkpoint_descriptor.as_bytes()),
            ),
            semantic(
                "redaction_policy",
                CanonicalValue::Bytes(redaction_policy.as_bytes()),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit.robotics/profile"),
            schema_version: self.schema_version,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

fn hash_value_type(
    value_type: RoboticsValueType<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let descriptor = hash_pin("conduit.robotics/value-type-pin", value_type.descriptor)?;
    hash_fields(
        "conduit.robotics/value-type",
        &[
            semantic(
                "role",
                CanonicalValue::Identifier(Id(value_type.role.as_str())),
            ),
            semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
        ],
    )
}

fn hash_quantity(
    quantity: QuantityContract<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    hash_fields(
        "conduit.robotics/quantity",
        &[
            semantic("id", CanonicalValue::Identifier(quantity.id)),
            semantic("units", CanonicalValue::Identifier(quantity.units)),
            semantic("frame", CanonicalValue::Identifier(quantity.frame)),
            semantic("clock", CanonicalValue::Identifier(quantity.clock)),
            semantic(
                "uncertainty_bound",
                CanonicalValue::Integer(i128::from(quantity.uncertainty_bound)),
            ),
            semantic(
                "maximum_age_ticks",
                CanonicalValue::Integer(i128::from(quantity.maximum_age_ticks)),
            ),
        ],
    )
}

fn hash_relationship(
    relationship: LogicalRelationship<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let mut carriers = [CanonicalValue::Identifier(Id("unused")); MAXIMUM_CARRIER_CANDIDATES];
    for (index, carrier) in relationship.allowed_carriers.iter().copied().enumerate() {
        carriers[index] = CanonicalValue::Identifier(carrier);
    }
    hash_fields(
        "conduit.robotics/logical-relationship",
        &[
            semantic("id", CanonicalValue::Identifier(relationship.id)),
            semantic(
                "source_entity",
                CanonicalValue::Identifier(relationship.source_entity),
            ),
            semantic(
                "target_entity",
                CanonicalValue::Identifier(relationship.target_entity),
            ),
            semantic("role", CanonicalValue::Identifier(relationship.role)),
            semantic(
                "allowed_carriers",
                CanonicalValue::List(&carriers[..relationship.allowed_carriers.len()]),
            ),
        ],
    )
}

fn hash_pin(
    kind: &str,
    pin: PinnedDescriptor<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    hash_fields(
        kind,
        &[
            semantic("id", CanonicalValue::Identifier(pin.id)),
            semantic(
                "schema_version",
                CanonicalValue::Integer(i128::from(pin.schema_version)),
            ),
            semantic(
                "semantic_hash",
                CanonicalValue::Bytes(pin.semantic_hash.as_bytes()),
            ),
        ],
    )
}

fn hash_fields(
    kind: &str,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id(kind),
        schema_version: 0,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompiledProviderState {
    Unsupported,
    Compiled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledProvider<'a> {
    pub capability: PinnedDescriptor<'a>,
    pub state: CompiledProviderState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierCandidate<'a> {
    pub relationship: Id<'a>,
    pub carrier: Id<'a>,
    pub provider: PinnedDescriptor<'a>,
    pub compiled: bool,
    pub admitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PathObservation<'a> {
    pub relationship: Id<'a>,
    pub carrier: Id<'a>,
    pub provider: PinnedDescriptor<'a>,
    pub generation: u32,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub sensitivity: Id<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescribeEffectAudit {
    pub device_open_count: u16,
    pub network_join_count: u16,
    pub relay_count: u16,
    pub possession_count: u16,
    pub role_promotion_count: u16,
    pub plan_activation_count: u16,
    pub actuation_count: u16,
}

impl DescribeEffectAudit {
    pub const NONE: Self = Self {
        device_open_count: 0,
        network_join_count: 0,
        relay_count: 0,
        possession_count: 0,
        role_promotion_count: 0,
        plan_activation_count: 0,
        actuation_count: 0,
    };

    #[must_use]
    pub const fn is_effect_free(self) -> bool {
        self.device_open_count == 0
            && self.network_join_count == 0
            && self.relay_count == 0
            && self.possession_count == 0
            && self.role_promotion_count == 0
            && self.plan_activation_count == 0
            && self.actuation_count == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoboticsHostReport<'a> {
    pub profile: RoboticsProfile<'a>,
    pub generic_host: HostConformanceProfile<'a>,
    pub implementation: PinnedDescriptor<'a>,
    pub artifact: PinnedDescriptor<'a>,
    pub adapter: PinnedDescriptor<'a>,
    pub entity: PinnedDescriptor<'a>,
    pub boot: Option<PinnedDescriptor<'a>>,
    pub role: PinnedDescriptor<'a>,
    pub possession: Option<PinnedDescriptor<'a>>,
    pub authority: Option<PinnedDescriptor<'a>>,
    pub compiled_providers: &'a [CompiledProvider<'a>],
    pub carrier_candidates: &'a [CarrierCandidate<'a>],
    pub path_observations: &'a [PathObservation<'a>],
    pub hidden_device_handles: u16,
    pub secret_or_topology_fields: u16,
    pub effect_audit: DescribeEffectAudit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoboticsObservation<'a> {
    pub value_type: PinnedDescriptor<'a>,
    pub profile: SemanticHash,
    pub quantity: Id<'a>,
    pub units: Id<'a>,
    pub frame: Id<'a>,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub uncertainty: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionAdmission<'a> {
    pub command_type: PinnedDescriptor<'a>,
    pub profile: SemanticHash,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub linear_velocity_mm_per_second: i32,
    pub angular_velocity_milliradians_per_second: i32,
    pub possession: Option<PinnedDescriptor<'a>>,
    pub authority: Option<PinnedDescriptor<'a>>,
    pub capability: Option<PinnedDescriptor<'a>>,
    pub stop_available: bool,
    pub emergency_stop_available: bool,
    pub charging: bool,
    pub charging_interlock_active: bool,
    pub inhibit_clear: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoboticsReason {
    UnsupportedVersion,
    InvalidDescriptor,
    ValueRoleMismatch,
    QuantityMismatch,
    ObservationStale,
    CommandExpired,
    AuthorityInvalid,
    SafetyRequirementMissing,
    DiscoveryIsNotEnrollment,
    HiddenDeviceHandle,
    UnsupportedHost,
    SensitiveDisclosure,
    DescribeCausedEffect,
    CapabilityMissing,
}

impl RoboticsReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-RBT-001",
            Self::InvalidDescriptor => "CND-RBT-002",
            Self::ValueRoleMismatch => "CND-RBT-003",
            Self::QuantityMismatch => "CND-RBT-004",
            Self::ObservationStale => "CND-RBT-005",
            Self::CommandExpired => "CND-RBT-006",
            Self::AuthorityInvalid => "CND-RBT-007",
            Self::SafetyRequirementMissing => "CND-RBT-008",
            Self::DiscoveryIsNotEnrollment => "CND-RBT-009",
            Self::HiddenDeviceHandle => "CND-RBT-010",
            Self::UnsupportedHost => "CND-RBT-011",
            Self::SensitiveDisclosure => "CND-RBT-012",
            Self::DescribeCausedEffect => "CND-RBT-013",
            Self::CapabilityMissing => "CND-RBT-014",
        }
    }
}

pub fn validate_profile(profile: RoboticsProfile<'_>) -> Result<(), RoboticsReason> {
    if profile.schema_version != ROBOTICS_PROFILE_SCHEMA_VERSION {
        return Err(RoboticsReason::UnsupportedVersion);
    }
    if profile.id.as_str().is_empty()
        || profile.value_types.len() != 7
        || profile.value_types.len() > MAXIMUM_PROFILE_TYPES
        || profile.quantities.is_empty()
        || profile.quantities.len() > MAXIMUM_PROFILE_QUANTITIES
        || profile.logical_relationships.is_empty()
        || profile.logical_relationships.len() > MAXIMUM_LOGICAL_RELATIONSHIPS
        || profile.safety.command_ttl_ticks == 0
        || profile.safety.maximum_linear_velocity_mm_per_second == 0
        || profile
            .safety
            .maximum_angular_velocity_milliradians_per_second
            == 0
        || profile.safety.maximum_command_queue == 0
        || profile.quantities.iter().any(|quantity| {
            quantity.id.as_str().is_empty()
                || quantity.units.as_str().is_empty()
                || quantity.frame.as_str().is_empty()
                || quantity.clock.as_str().is_empty()
                || quantity.maximum_age_ticks == 0
        })
        || profile.logical_relationships.iter().any(|relationship| {
            relationship.id.as_str().is_empty()
                || relationship.allowed_carriers.is_empty()
                || relationship.allowed_carriers.len() > MAXIMUM_CARRIER_CANDIDATES
        })
        || profile
            .value_types
            .iter()
            .enumerate()
            .any(|(index, value)| {
                profile.value_types[index + 1..]
                    .iter()
                    .any(|other| value.role == other.role || value.descriptor == other.descriptor)
            })
        || profile
            .value_types
            .iter()
            .any(|value| !pin_valid(value.descriptor))
        || !pin_valid(profile.safety.possession)
        || !pin_valid(profile.safety.motion_authority)
        || !pin_valid(profile.safety.stop)
        || !pin_valid(profile.safety.emergency_stop)
        || !pin_valid(profile.safety.not_charging)
        || !pin_valid(profile.safety.charging_interlock)
        || !pin_valid(profile.safety.inhibit_clear)
        || !pin_valid(profile.safety.capability)
        || !pin_valid(profile.role_descriptor)
        || !pin_valid(profile.checkpoint_descriptor)
        || !pin_valid(profile.redaction_policy)
    {
        return Err(RoboticsReason::InvalidDescriptor);
    }
    for role in [
        RoboticsValueRole::Observation,
        RoboticsValueRole::Command,
        RoboticsValueRole::Acknowledgement,
        RoboticsValueRole::SafeOutcome,
        RoboticsValueRole::Possession,
        RoboticsValueRole::Terminal,
        RoboticsValueRole::Fault,
    ] {
        if !profile.value_types.iter().any(|value| value.role == role) {
            return Err(RoboticsReason::InvalidDescriptor);
        }
    }
    if profile
        .semantic_hash()
        .map_err(|_| RoboticsReason::InvalidDescriptor)?
        != profile.identity
    {
        return Err(RoboticsReason::InvalidDescriptor);
    }
    Ok(())
}

pub fn validate_observation(
    profile: RoboticsProfile<'_>,
    observation: RoboticsObservation<'_>,
    current_tick: u64,
) -> Result<(), RoboticsReason> {
    validate_profile(profile)?;
    if observation.value_type != OBSERVATION_TYPE || observation.profile != profile.identity {
        return Err(RoboticsReason::ValueRoleMismatch);
    }
    let quantity = profile
        .quantities
        .iter()
        .find(|quantity| quantity.id == observation.quantity)
        .ok_or(RoboticsReason::QuantityMismatch)?;
    if quantity.units != observation.units
        || quantity.frame != observation.frame
        || quantity.clock != observation.time_basis
        || observation.uncertainty > quantity.uncertainty_bound
    {
        return Err(RoboticsReason::QuantityMismatch);
    }
    if observation.observed_at_tick > current_tick
        || current_tick >= observation.valid_until_tick
        || observation
            .valid_until_tick
            .saturating_sub(observation.observed_at_tick)
            > quantity.maximum_age_ticks
    {
        return Err(RoboticsReason::ObservationStale);
    }
    Ok(())
}

pub fn validate_motion_admission(
    profile: RoboticsProfile<'_>,
    command: MotionAdmission<'_>,
    current_tick: u64,
) -> Result<(), RoboticsReason> {
    validate_profile(profile)?;
    if command.command_type != COMMAND_TYPE || command.profile != profile.identity {
        return Err(RoboticsReason::ValueRoleMismatch);
    }
    if command.issued_at_tick > current_tick
        || current_tick >= command.expires_at_tick
        || command
            .expires_at_tick
            .saturating_sub(command.issued_at_tick)
            > profile.safety.command_ttl_ticks
    {
        return Err(RoboticsReason::CommandExpired);
    }
    if command.linear_velocity_mm_per_second.unsigned_abs()
        > profile.safety.maximum_linear_velocity_mm_per_second
        || command
            .angular_velocity_milliradians_per_second
            .unsigned_abs()
            > profile
                .safety
                .maximum_angular_velocity_milliradians_per_second
    {
        return Err(RoboticsReason::QuantityMismatch);
    }
    if command.possession != Some(profile.safety.possession)
        || command.authority != Some(profile.safety.motion_authority)
    {
        return Err(RoboticsReason::AuthorityInvalid);
    }
    if command.capability != Some(profile.safety.capability) {
        return Err(RoboticsReason::CapabilityMissing);
    }
    if !command.stop_available
        || !command.emergency_stop_available
        || !command.inhibit_clear
        || (command.charging && !command.charging_interlock_active)
    {
        return Err(RoboticsReason::SafetyRequirementMissing);
    }
    Ok(())
}

pub fn validate_describe_only_report(report: RoboticsHostReport<'_>) -> Result<(), RoboticsReason> {
    validate_profile(report.profile)?;
    validate_host_conformance_profile(report.generic_host).map_err(map_host_reason)?;
    if report.generic_host.class != HostClass::DescribeOnly
        || report.generic_host.execution_mode != HostExecutionMode::DescribeOnly
        || !report
            .generic_host
            .optional_providers
            .iter()
            .any(|provider| {
                provider.contract == ROBOTICS_PROFILE_CONTRACT
                    && provider.state == ProviderInventoryState::Linked
            })
        || !report.generic_host.extensions.iter().any(|extension| {
            extension.kind == HostExtensionKind::Implementation
                && extension.descriptor == report.implementation
        })
        || !report.generic_host.extensions.iter().any(|extension| {
            extension.kind == HostExtensionKind::Adapter && extension.descriptor == report.adapter
        })
        || !report
            .generic_host
            .mandatory_facts
            .contains(&report.artifact)
        || !report.generic_host.mandatory_facts.contains(&report.entity)
        || !report.generic_host.mandatory_facts.contains(&report.role)
    {
        return Err(RoboticsReason::UnsupportedHost);
    }
    if report.boot.is_some() || report.possession.is_some() || report.authority.is_some() {
        return Err(RoboticsReason::DiscoveryIsNotEnrollment);
    }
    if !report.path_observations.is_empty()
        || report.path_observations.len() > MAXIMUM_PATH_OBSERVATIONS
    {
        return Err(RoboticsReason::DiscoveryIsNotEnrollment);
    }
    if report.compiled_providers.len() > MAXIMUM_PROVIDER_STATES
        || report.carrier_candidates.len() > MAXIMUM_CARRIER_CANDIDATES
        || report
            .compiled_providers
            .iter()
            .any(|provider| !pin_valid(provider.capability))
        || report.carrier_candidates.iter().any(|candidate| {
            candidate.admitted
                || !pin_valid(candidate.provider)
                || !report
                    .profile
                    .logical_relationships
                    .iter()
                    .any(|relationship| {
                        relationship.id == candidate.relationship
                            && relationship.allowed_carriers.contains(&candidate.carrier)
                    })
        })
    {
        return Err(RoboticsReason::DiscoveryIsNotEnrollment);
    }
    if report.hidden_device_handles != 0 {
        return Err(RoboticsReason::HiddenDeviceHandle);
    }
    if report.secret_or_topology_fields != 0 {
        return Err(RoboticsReason::SensitiveDisclosure);
    }
    if !report.effect_audit.is_effect_free() {
        return Err(RoboticsReason::DescribeCausedEffect);
    }
    Ok(())
}

fn map_host_reason(reason: HostConformanceReason) -> RoboticsReason {
    match reason {
        HostConformanceReason::UnsupportedVersion => RoboticsReason::UnsupportedVersion,
        _ => RoboticsReason::UnsupportedHost,
    }
}

fn pin_valid(pin: PinnedDescriptor<'_>) -> bool {
    !pin.id.as_str().is_empty()
        && pin.id.as_str().contains('/')
        && pin.schema_version == 0
        && pin.semantic_hash != ZERO_HASH
}

const LINUX_FACTS: [PinnedDescriptor<'static>; 5] = [
    LINUX_ARTIFACT,
    pin(
        "netherwick/host-fact/linux-rpi5",
        hash_bytes("dd1b0292a3bb5e0f4353637e960513289a9bacf1dd46907d1ecc41e6ac5cc107"),
    ),
    pin(
        "netherwick/entity/pete-brainstem",
        hash_bytes("9c03a91a6ee299caecc0728bcb4fc3672cf59c302e607f141ddc22ecc8024ef6"),
    ),
    ROLE_DESCRIPTOR,
    CHECKPOINT_DESCRIPTOR,
];
const PICO_FACTS: [PinnedDescriptor<'static>; 5] = [
    PICO_ARTIFACT,
    pin(
        "netherwick/host-fact/rp2040-pico-w",
        hash_bytes("eb3476db3465f5935409beef993825f276df4e40a0358d748218e282bf640247"),
    ),
    pin(
        "netherwick/entity/pete-brainstem",
        hash_bytes("9c03a91a6ee299caecc0728bcb4fc3672cf59c302e607f141ddc22ecc8024ef6"),
    ),
    ROLE_DESCRIPTOR,
    CHECKPOINT_DESCRIPTOR,
];
const LINUX_PROVIDERS: [ProviderInventory<'static>; 1] = [ProviderInventory {
    contract: ROBOTICS_PROFILE_CONTRACT,
    provider_bundle: LINUX_PROVIDER_BUNDLE,
    state: ProviderInventoryState::Linked,
}];
const PICO_PROVIDERS: [ProviderInventory<'static>; 1] = [ProviderInventory {
    contract: ROBOTICS_PROFILE_CONTRACT,
    provider_bundle: PICO_PROVIDER_BUNDLE,
    state: ProviderInventoryState::Linked,
}];
const LINUX_EXTENSIONS: [HostExtension<'static>; 2] = [
    HostExtension {
        kind: HostExtensionKind::Implementation,
        descriptor: LINUX_IMPLEMENTATION,
    },
    HostExtension {
        kind: HostExtensionKind::Adapter,
        descriptor: DESCRIBE_ADAPTER,
    },
];
const PICO_EXTENSIONS: [HostExtension<'static>; 2] = [
    HostExtension {
        kind: HostExtensionKind::Implementation,
        descriptor: PICO_IMPLEMENTATION,
    },
    HostExtension {
        kind: HostExtensionKind::Adapter,
        descriptor: DESCRIBE_ADAPTER,
    },
];

#[must_use]
pub fn linux_describe_only_host() -> HostConformanceProfile<'static> {
    describe_only_host(
        Id("netherwick/pete-linux-describe-only"),
        &LINUX_FACTS,
        &LINUX_PROVIDERS,
        &LINUX_EXTENSIONS,
    )
}

#[must_use]
pub fn pico_describe_only_host() -> HostConformanceProfile<'static> {
    describe_only_host(
        Id("netherwick/pete-pico-w-describe-only"),
        &PICO_FACTS,
        &PICO_PROVIDERS,
        &PICO_EXTENSIONS,
    )
}

fn describe_only_host(
    id: Id<'static>,
    mandatory_facts: &'static [PinnedDescriptor<'static>],
    optional_providers: &'static [ProviderInventory<'static>],
    extensions: &'static [HostExtension<'static>],
) -> HostConformanceProfile<'static> {
    let mut profile = HostConformanceProfile {
        schema_version: HOST_CONFORMANCE_PROFILE_SCHEMA_VERSION,
        identity: ZERO_HASH,
        id,
        class: HostClass::DescribeOnly,
        execution_mode: HostExecutionMode::DescribeOnly,
        mandatory_facts,
        optional_providers,
        extensions,
    };
    let mut scratch = [ZERO_HASH; 16];
    profile.identity = profile
        .computed_semantic_hash(&mut scratch)
        .expect("built-in host profile is canonical");
    profile
}

#[cfg(test)]
fn computed_profile_contract_hash() -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id("conduit/robotics-profile-contract"),
        schema_version: ROBOTICS_PROFILE_SCHEMA_VERSION,
        body: CanonicalValue::Map(&[
            semantic(
                "id",
                CanonicalValue::Identifier(ROBOTICS_PROFILE_CONTRACT.id),
            ),
            semantic(
                "maximum-profile-types",
                CanonicalValue::Integer(MAXIMUM_PROFILE_TYPES as i128),
            ),
            semantic(
                "maximum-profile-quantities",
                CanonicalValue::Integer(MAXIMUM_PROFILE_QUANTITIES as i128),
            ),
            semantic(
                "maximum-logical-relationships",
                CanonicalValue::Integer(MAXIMUM_LOGICAL_RELATIONSHIPS as i128),
            ),
            semantic(
                "maximum-provider-states",
                CanonicalValue::Integer(MAXIMUM_PROVIDER_STATES as i128),
            ),
            semantic(
                "maximum-carrier-candidates",
                CanonicalValue::Integer(MAXIMUM_CARRIER_CANDIDATES as i128),
            ),
            semantic(
                "maximum-path-observations",
                CanonicalValue::Integer(MAXIMUM_PATH_OBSERVATIONS as i128),
            ),
        ]),
    }
    .semantic_hash()
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTITY: PinnedDescriptor<'static> = pin(
        "netherwick/entity/pete-brainstem",
        hash_bytes("9c03a91a6ee299caecc0728bcb4fc3672cf59c302e607f141ddc22ecc8024ef6"),
    );
    const LINUX_COMPILED: [CompiledProvider<'static>; 4] = [
        CompiledProvider {
            capability: pin("netherwick/capability/usb", [0x91; 32]),
            state: CompiledProviderState::Compiled,
        },
        CompiledProvider {
            capability: pin("netherwick/capability/cyw43", [0x92; 32]),
            state: CompiledProviderState::Unsupported,
        },
        CompiledProvider {
            capability: pin("netherwick/capability/network", [0x93; 32]),
            state: CompiledProviderState::Compiled,
        },
        CompiledProvider {
            capability: MOTION_CAPABILITY,
            state: CompiledProviderState::Compiled,
        },
    ];
    const PICO_COMPILED: [CompiledProvider<'static>; 4] = [
        CompiledProvider {
            capability: pin("netherwick/capability/usb", [0x91; 32]),
            state: CompiledProviderState::Compiled,
        },
        CompiledProvider {
            capability: pin("netherwick/capability/cyw43", [0x92; 32]),
            state: CompiledProviderState::Compiled,
        },
        CompiledProvider {
            capability: pin("netherwick/capability/network", [0x93; 32]),
            state: CompiledProviderState::Compiled,
        },
        CompiledProvider {
            capability: MOTION_CAPABILITY,
            state: CompiledProviderState::Compiled,
        },
    ];
    const LINUX_CARRIERS: [CarrierCandidate<'static>; 2] = [
        CarrierCandidate {
            relationship: Id("motherbrain-to-brainstem"),
            carrier: Id("usb"),
            provider: pin("netherwick/provider/linux-usb", [0xa1; 32]),
            compiled: true,
            admitted: false,
        },
        CarrierCandidate {
            relationship: Id("motherbrain-to-brainstem"),
            carrier: Id("ethernet"),
            provider: pin("netherwick/provider/linux-ethernet", [0xa2; 32]),
            compiled: true,
            admitted: false,
        },
    ];
    const PICO_CARRIERS: [CarrierCandidate<'static>; 2] = [
        CarrierCandidate {
            relationship: Id("motherbrain-to-brainstem"),
            carrier: Id("usb"),
            provider: pin("netherwick/provider/pico-usb", [0xa3; 32]),
            compiled: true,
            admitted: false,
        },
        CarrierCandidate {
            relationship: Id("motherbrain-to-brainstem"),
            carrier: Id("wifi"),
            provider: pin("netherwick/provider/pico-wifi", [0xa4; 32]),
            compiled: true,
            admitted: false,
        },
    ];

    fn report(linux: bool) -> RoboticsHostReport<'static> {
        RoboticsHostReport {
            profile: audited_profile(),
            generic_host: if linux {
                linux_describe_only_host()
            } else {
                pico_describe_only_host()
            },
            implementation: if linux {
                LINUX_IMPLEMENTATION
            } else {
                PICO_IMPLEMENTATION
            },
            artifact: if linux { LINUX_ARTIFACT } else { PICO_ARTIFACT },
            adapter: DESCRIBE_ADAPTER,
            entity: ENTITY,
            boot: None,
            role: ROLE_DESCRIPTOR,
            possession: None,
            authority: None,
            compiled_providers: if linux {
                &LINUX_COMPILED
            } else {
                &PICO_COMPILED
            },
            carrier_candidates: if linux {
                &LINUX_CARRIERS
            } else {
                &PICO_CARRIERS
            },
            path_observations: &[],
            hidden_device_handles: 0,
            secret_or_topology_fields: 0,
            effect_audit: DescribeEffectAudit::NONE,
        }
    }

    fn observation() -> RoboticsObservation<'static> {
        RoboticsObservation {
            value_type: OBSERVATION_TYPE,
            profile: audited_profile().identity,
            quantity: Id("linear-velocity"),
            units: Id("mm-per-second"),
            frame: Id("create-body"),
            time_basis: Id("brainstem-monotonic-milliseconds"),
            observed_at_tick: 100,
            valid_until_tick: 200,
            uncertainty: 20,
        }
    }

    fn command() -> MotionAdmission<'static> {
        MotionAdmission {
            command_type: COMMAND_TYPE,
            profile: audited_profile().identity,
            issued_at_tick: 100,
            expires_at_tick: 200,
            linear_velocity_mm_per_second: 100,
            angular_velocity_milliradians_per_second: 100,
            possession: Some(POSSESSION_REQUIREMENT),
            authority: Some(MOTION_AUTHORITY),
            capability: Some(MOTION_CAPABILITY),
            stop_available: true,
            emergency_stop_available: true,
            charging: false,
            charging_interlock_active: true,
            inhibit_clear: true,
        }
    }

    #[test]
    fn audited_descriptor_has_exact_hash_and_distinct_value_roles() {
        assert_eq!(
            ROBOTICS_PROFILE_CONTRACT.semantic_hash,
            computed_profile_contract_hash().unwrap()
        );
        for descriptor in [
            LINUX_IMPLEMENTATION,
            PICO_IMPLEMENTATION,
            LINUX_ARTIFACT,
            PICO_ARTIFACT,
            LINUX_PROVIDER_BUNDLE,
            PICO_PROVIDER_BUNDLE,
            DESCRIBE_ADAPTER,
            OBSERVATION_TYPE,
            COMMAND_TYPE,
            ACKNOWLEDGEMENT_TYPE,
            SAFE_OUTCOME_TYPE,
            POSSESSION_TYPE,
            TERMINAL_TYPE,
            FAULT_TYPE,
            MOTION_AUTHORITY,
            POSSESSION_REQUIREMENT,
            STOP_REQUIREMENT,
            ESTOP_REQUIREMENT,
            CHARGING_REQUIREMENT,
            INTERLOCK_REQUIREMENT,
            INHIBIT_REQUIREMENT,
            MOTION_CAPABILITY,
            ROLE_DESCRIPTOR,
            CHECKPOINT_DESCRIPTOR,
            REDACTION_POLICY,
            LINUX_FACTS[1],
            LINUX_FACTS[2],
            PICO_FACTS[1],
        ] {
            assert_eq!(
                descriptor.semantic_hash,
                hash_fields(
                    "conduit/pinned-descriptor",
                    &[semantic("id", CanonicalValue::Identifier(descriptor.id))]
                )
                .unwrap(),
                "{} must be derived from its canonical descriptor",
                descriptor.id
            );
        }
        let profile = audited_profile();
        validate_profile(profile).unwrap();
        assert_eq!(
            profile.identity.to_string(),
            "sha256:f4ccc52b6c40ef5106a0752cf8e2d926bbcb1bcebe5b19287c32733315767029"
        );
        assert_ne!(profile.identity, ZERO_HASH);
        assert_eq!(profile.value_types.len(), 7);
        assert_ne!(OBSERVATION_TYPE, COMMAND_TYPE);
        assert_ne!(ACKNOWLEDGEMENT_TYPE, SAFE_OUTCOME_TYPE);
        assert_ne!(TERMINAL_TYPE, FAULT_TYPE);

        let mut changed_quantities = QUANTITIES;
        changed_quantities[0].frame = Id("world");
        assert_eq!(
            validate_profile(RoboticsProfile {
                quantities: &changed_quantities,
                ..profile
            }),
            Err(RoboticsReason::InvalidDescriptor)
        );
    }

    #[test]
    fn linux_and_pico_use_one_contract_with_distinct_generic_implementations() {
        let linux = report(true);
        let pico = report(false);
        validate_describe_only_report(linux).unwrap();
        validate_describe_only_report(pico).unwrap();
        assert_eq!(
            linux.generic_host.identity.to_string(),
            "sha256:09a016735e026bbc9fc15a0af4f8023fae2dd67a31f172749f7da46037cdfb52"
        );
        assert_eq!(
            pico.generic_host.identity.to_string(),
            "sha256:46bb4f92740d3389b3e2e0be7f2a042d4c7f317fd134d9ec33133e0ea6cd1fd9"
        );
        assert_eq!(
            linux.generic_host.optional_providers[0].contract,
            pico.generic_host.optional_providers[0].contract
        );
        assert_ne!(linux.implementation, pico.implementation);
        assert_ne!(linux.artifact, pico.artifact);
        assert_ne!(linux.generic_host.identity, pico.generic_host.identity);
    }

    #[test]
    fn observation_requires_exact_type_units_frame_clock_uncertainty_and_freshness() {
        let profile = audited_profile();
        validate_observation(profile, observation(), 150).unwrap();
        assert_eq!(
            validate_observation(
                profile,
                RoboticsObservation {
                    value_type: COMMAND_TYPE,
                    ..observation()
                },
                150
            ),
            Err(RoboticsReason::ValueRoleMismatch)
        );
        assert_eq!(
            validate_observation(
                profile,
                RoboticsObservation {
                    units: Id("metres-per-second"),
                    ..observation()
                },
                150
            ),
            Err(RoboticsReason::QuantityMismatch)
        );
        assert_eq!(
            validate_observation(
                profile,
                RoboticsObservation {
                    frame: Id("world"),
                    ..observation()
                },
                150
            ),
            Err(RoboticsReason::QuantityMismatch)
        );
        assert_eq!(
            validate_observation(profile, observation(), 200),
            Err(RoboticsReason::ObservationStale)
        );
    }

    #[test]
    fn motion_requires_finite_ttl_envelope_authority_capability_and_safety() {
        let profile = audited_profile();
        validate_motion_admission(profile, command(), 150).unwrap();
        assert_eq!(
            validate_motion_admission(
                profile,
                MotionAdmission {
                    expires_at_tick: 400,
                    ..command()
                },
                150
            ),
            Err(RoboticsReason::CommandExpired)
        );
        assert_eq!(
            validate_motion_admission(
                profile,
                MotionAdmission {
                    possession: Some(ROLE_DESCRIPTOR),
                    ..command()
                },
                150
            ),
            Err(RoboticsReason::AuthorityInvalid)
        );
        assert_eq!(
            validate_motion_admission(
                profile,
                MotionAdmission {
                    capability: None,
                    ..command()
                },
                150
            ),
            Err(RoboticsReason::CapabilityMissing)
        );
        assert_eq!(
            validate_motion_admission(
                profile,
                MotionAdmission {
                    inhibit_clear: false,
                    ..command()
                },
                150
            ),
            Err(RoboticsReason::SafetyRequirementMissing)
        );
        assert_eq!(
            validate_motion_admission(
                profile,
                MotionAdmission {
                    charging: true,
                    charging_interlock_active: false,
                    ..command()
                },
                150
            ),
            Err(RoboticsReason::SafetyRequirementMissing)
        );
    }

    #[test]
    fn describe_only_rejects_live_facts_handles_leakage_and_effects() {
        let clean = report(true);
        validate_describe_only_report(clean).unwrap();
        assert_eq!(
            validate_describe_only_report(RoboticsHostReport {
                possession: Some(POSSESSION_REQUIREMENT),
                ..clean
            }),
            Err(RoboticsReason::DiscoveryIsNotEnrollment)
        );
        assert_eq!(
            validate_describe_only_report(RoboticsHostReport {
                hidden_device_handles: 1,
                ..clean
            }),
            Err(RoboticsReason::HiddenDeviceHandle)
        );
        assert_eq!(
            validate_describe_only_report(RoboticsHostReport {
                secret_or_topology_fields: 1,
                ..clean
            }),
            Err(RoboticsReason::SensitiveDisclosure)
        );
        assert_eq!(
            validate_describe_only_report(RoboticsHostReport {
                effect_audit: DescribeEffectAudit {
                    device_open_count: 1,
                    ..DescribeEffectAudit::NONE
                },
                ..clean
            }),
            Err(RoboticsReason::DescribeCausedEffect)
        );

        let path = [PathObservation {
            relationship: Id("motherbrain-to-brainstem"),
            carrier: Id("usb"),
            provider: pin("netherwick/provider/linux-usb", [0xa1; 32]),
            generation: 1,
            time_basis: Id("brainstem-monotonic-milliseconds"),
            observed_at_tick: 10,
            valid_until_tick: 20,
            sensitivity: Id("restricted"),
        }];
        assert_eq!(
            validate_describe_only_report(RoboticsHostReport {
                path_observations: &path,
                ..clean
            }),
            Err(RoboticsReason::DiscoveryIsNotEnrollment)
        );
        assert_eq!(
            validate_describe_only_report(RoboticsHostReport {
                implementation: PICO_IMPLEMENTATION,
                ..clean
            }),
            Err(RoboticsReason::UnsupportedHost)
        );
    }

    #[test]
    fn conformance_fixture_names_every_required_negative() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/c5/netherwick-describe-only-profile.json"
        ))
        .unwrap();
        for id in [
            "observation-as-command",
            "unit-mismatch",
            "frame-mismatch",
            "stale-sensor",
            "expired-command-ttl",
            "role-as-motion-grant",
            "membership-as-motion-grant",
            "capability-as-motion-grant",
            "missing-inhibit",
            "network-discovery-as-enrollment",
            "hidden-device-handle",
            "unsupported-host",
            "secret-topology-leakage",
            "describe-causes-effects",
            "missing-capability",
            "descriptor-hash-mismatch",
        ] {
            assert!(
                value["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|case| case["id"] == id),
                "missing {id}"
            );
        }
    }
}
