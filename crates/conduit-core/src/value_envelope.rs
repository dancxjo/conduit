//! Bounded per-value metadata, clock conversion, and feedback admission.
//!
//! These borrowed contracts are plan facts. They do not allocate storage,
//! obtain a clock, assign correlation, or create a feedback executor.

use core::fmt;

use crate::{AuthorityTime, Id, InstancePath, PinnedDescriptor, ResolvedPlanCord, Sensitivity};

pub const VALUE_ENVELOPE_POLICY_SCHEMA_VERSION: u32 = 1;
pub const CLOCK_CONVERSION_SCHEMA_VERSION: u32 = 1;
pub const FEEDBACK_BOUNDARY_SCHEMA_VERSION: u32 = 1;
pub const MAX_VALUE_CLOCK_DOMAINS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueEnvelopePolicy<'a> {
    pub cord: Id<'a>,
    pub representation: PinnedDescriptor<'a>,
    pub maximum_payload_bytes: u32,
    pub maximum_envelope_bytes: u32,
    pub maximum_fragments: u16,
    pub maximum_fragment_bytes: u32,
    pub maximum_timestamps: u8,
    pub clock_domains: &'a [Id<'a>],
    pub identity_allowed: bool,
    pub correlation_allowed: bool,
    pub causation_allowed: bool,
    pub provenance_allowed: bool,
    pub sensitivity_ceiling: Sensitivity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueTimestamp<'a> {
    pub domain: Id<'a>,
    pub tick: i64,
    pub uncertainty_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueEnvelope<'a> {
    pub representation: PinnedDescriptor<'a>,
    pub payload_bytes: u32,
    pub envelope_bytes: u32,
    pub fragment_count: u16,
    pub fragment_bytes: u32,
    pub identity: Option<Id<'a>>,
    pub correlation: Option<Id<'a>>,
    pub causation: Option<Id<'a>>,
    pub provenance: Option<Id<'a>>,
    pub timestamps: &'a [ValueTimestamp<'a>],
    pub sensitivity: Sensitivity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockRounding {
    Exact,
    Floor,
    Ceiling,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanClockConversion<'a> {
    pub id: Id<'a>,
    pub source: Id<'a>,
    pub destination: Id<'a>,
    pub numerator: u64,
    pub denominator: u64,
    pub offset_ticks: i64,
    pub rounding: ClockRounding,
    pub maximum_uncertainty_ticks: u64,
    pub observed_at: AuthorityTime<'a>,
    pub valid_until_tick: u64,
    pub authority: Id<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConvertedTime {
    pub domain_tick: i64,
    pub earliest_tick: i64,
    pub latest_tick: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedbackBoundaryKind {
    Delay,
    State,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedbackInitialization {
    Empty,
    InitialValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedbackReplayGapPolicy {
    Fail,
    Reset,
    Wait,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedbackTerminalPolicy {
    DropRetained,
    DrainRetained,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanFeedbackBoundary<'a> {
    pub id: Id<'a>,
    pub node: InstancePath<'a>,
    pub cord: Id<'a>,
    pub kind: FeedbackBoundaryKind,
    pub initialization: FeedbackInitialization,
    pub initial_items: u16,
    pub initial_bytes: u64,
    pub maximum_retained_items: u16,
    pub maximum_retained_bytes: u64,
    pub delay_ticks: u64,
    pub clock: Option<Id<'a>>,
    pub replay_gap: FeedbackReplayGapPolicy,
    pub cancellation: PinnedDescriptor<'a>,
    pub terminal: FeedbackTerminalPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueEnvelopeReason {
    InvalidPolicy,
    InvalidBound,
    UnauthorizedField,
    RepresentationMismatch,
    ClockNotAuthorized,
    SensitivityWidening,
    InvalidClockConversion,
    StaleClockConversion,
    ClockArithmeticOverflow,
    InvalidFeedbackBoundary,
    InvalidFeedbackCycle,
}

impl ValueEnvelopeReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "CND-VEF-001",
            Self::InvalidBound => "CND-VEF-002",
            Self::UnauthorizedField
            | Self::RepresentationMismatch
            | Self::ClockNotAuthorized
            | Self::SensitivityWidening => "CND-VEF-003",
            Self::InvalidClockConversion
            | Self::StaleClockConversion
            | Self::ClockArithmeticOverflow => "CND-CLK-001",
            Self::InvalidFeedbackBoundary => "CND-FBK-001",
            Self::InvalidFeedbackCycle => "CND-FBK-002",
        }
    }
}

impl fmt::Display for ValueEnvelopeReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

fn valid_id(value: Id<'_>) -> bool {
    Id::new(value.as_str()).is_ok()
}

fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    valid_id(value.id) && value.schema_version != 0 && value.semantic_hash.as_bytes() != &[0; 32]
}

pub fn validate_value_envelope_policy(
    policy: ValueEnvelopePolicy<'_>,
) -> Result<(), ValueEnvelopeReason> {
    if !valid_id(policy.cord)
        || !valid_pin(policy.representation)
        || policy.clock_domains.len() > MAX_VALUE_CLOCK_DOMAINS
        || policy
            .clock_domains
            .iter()
            .enumerate()
            .any(|(index, domain)| {
                !valid_id(*domain) || policy.clock_domains[..index].contains(domain)
            })
    {
        return Err(ValueEnvelopeReason::InvalidPolicy);
    }
    if policy.maximum_payload_bytes == 0
        || policy.maximum_envelope_bytes == 0
        || policy.maximum_fragments == 0
        || policy.maximum_fragment_bytes == 0
        || u64::from(policy.maximum_fragments)
            .checked_mul(u64::from(policy.maximum_fragment_bytes))
            .is_none_or(|ceiling| ceiling < u64::from(policy.maximum_payload_bytes))
        || (policy.clock_domains.is_empty() && policy.maximum_timestamps != 0)
        || (!policy.clock_domains.is_empty() && policy.maximum_timestamps == 0)
    {
        return Err(ValueEnvelopeReason::InvalidBound);
    }
    Ok(())
}

pub fn validate_value_envelope(
    policy: ValueEnvelopePolicy<'_>,
    envelope: ValueEnvelope<'_>,
) -> Result<(), ValueEnvelopeReason> {
    validate_value_envelope_policy(policy)?;
    if envelope.representation != policy.representation {
        return Err(ValueEnvelopeReason::RepresentationMismatch);
    }
    if envelope.payload_bytes == 0
        || envelope.payload_bytes > policy.maximum_payload_bytes
        || envelope.envelope_bytes == 0
        || envelope.envelope_bytes > policy.maximum_envelope_bytes
        || envelope.fragment_count == 0
        || envelope.fragment_count > policy.maximum_fragments
        || envelope.fragment_bytes < envelope.payload_bytes
        || envelope.fragment_bytes
            > u32::from(envelope.fragment_count)
                .checked_mul(policy.maximum_fragment_bytes)
                .ok_or(ValueEnvelopeReason::InvalidBound)?
        || envelope.timestamps.len() > usize::from(policy.maximum_timestamps)
    {
        return Err(ValueEnvelopeReason::InvalidBound);
    }
    if (envelope.identity.is_some() && !policy.identity_allowed)
        || (envelope.correlation.is_some() && !policy.correlation_allowed)
        || (envelope.causation.is_some() && !policy.causation_allowed)
        || (envelope.provenance.is_some() && !policy.provenance_allowed)
    {
        return Err(ValueEnvelopeReason::UnauthorizedField);
    }
    if envelope
        .timestamps
        .iter()
        .any(|timestamp| !policy.clock_domains.contains(&timestamp.domain))
    {
        return Err(ValueEnvelopeReason::ClockNotAuthorized);
    }
    if envelope.sensitivity > policy.sensitivity_ceiling {
        return Err(ValueEnvelopeReason::SensitivityWidening);
    }
    Ok(())
}

pub fn validate_clock_conversion(
    conversion: PlanClockConversion<'_>,
) -> Result<(), ValueEnvelopeReason> {
    if !valid_id(conversion.id)
        || !valid_id(conversion.source)
        || !valid_id(conversion.destination)
        || !valid_id(conversion.observed_at.basis)
        || !valid_id(conversion.authority)
        || conversion.source == conversion.destination
        || conversion.numerator == 0
        || conversion.denominator == 0
    {
        return Err(ValueEnvelopeReason::InvalidClockConversion);
    }
    if conversion.valid_until_tick < conversion.observed_at.tick {
        return Err(ValueEnvelopeReason::StaleClockConversion);
    }
    Ok(())
}

pub fn convert_clock(
    conversion: PlanClockConversion<'_>,
    source_tick: i64,
    now: AuthorityTime<'_>,
) -> Result<ConvertedTime, ValueEnvelopeReason> {
    validate_clock_conversion(conversion)?;
    if now.basis != conversion.observed_at.basis
        || now.tick < conversion.observed_at.tick
        || now.tick > conversion.valid_until_tick
    {
        return Err(ValueEnvelopeReason::StaleClockConversion);
    }

    let scaled = i128::from(source_tick)
        .checked_mul(i128::from(conversion.numerator))
        .ok_or(ValueEnvelopeReason::ClockArithmeticOverflow)?;
    let denominator = i128::from(conversion.denominator);
    let quotient = scaled.div_euclid(denominator);
    let remainder = scaled.rem_euclid(denominator);
    let rounded = match conversion.rounding {
        ClockRounding::Exact if remainder != 0 => {
            return Err(ValueEnvelopeReason::InvalidClockConversion);
        }
        ClockRounding::Exact | ClockRounding::Floor => quotient,
        ClockRounding::Ceiling => quotient
            .checked_add(i128::from(remainder != 0))
            .ok_or(ValueEnvelopeReason::ClockArithmeticOverflow)?,
    }
    .checked_add(i128::from(conversion.offset_ticks))
    .ok_or(ValueEnvelopeReason::ClockArithmeticOverflow)?;

    let uncertainty = i128::from(conversion.maximum_uncertainty_ticks);
    let earliest = rounded
        .checked_sub(uncertainty)
        .ok_or(ValueEnvelopeReason::ClockArithmeticOverflow)?;
    let latest = rounded
        .checked_add(uncertainty)
        .ok_or(ValueEnvelopeReason::ClockArithmeticOverflow)?;
    Ok(ConvertedTime {
        domain_tick: i64::try_from(rounded)
            .map_err(|_| ValueEnvelopeReason::ClockArithmeticOverflow)?,
        earliest_tick: i64::try_from(earliest)
            .map_err(|_| ValueEnvelopeReason::ClockArithmeticOverflow)?,
        latest_tick: i64::try_from(latest)
            .map_err(|_| ValueEnvelopeReason::ClockArithmeticOverflow)?,
    })
}

pub fn validate_feedback_boundary(
    boundary: PlanFeedbackBoundary<'_>,
) -> Result<(), ValueEnvelopeReason> {
    if !valid_id(boundary.id)
        || InstancePath::new(boundary.node.as_str()).is_err()
        || !valid_id(boundary.cord)
        || !valid_pin(boundary.cancellation)
        || boundary.maximum_retained_items == 0
        || boundary.maximum_retained_bytes == 0
        || boundary.initial_items > boundary.maximum_retained_items
        || boundary.initial_bytes > boundary.maximum_retained_bytes
        || matches!(boundary.initialization, FeedbackInitialization::Empty)
            && (boundary.initial_items != 0 || boundary.initial_bytes != 0)
        || matches!(
            boundary.initialization,
            FeedbackInitialization::InitialValue
        ) && (boundary.initial_items == 0 || boundary.initial_bytes == 0)
    {
        return Err(ValueEnvelopeReason::InvalidFeedbackBoundary);
    }

    match boundary.kind {
        FeedbackBoundaryKind::Delay => {
            if boundary.delay_ticks == 0 || boundary.clock.is_none_or(|clock| !valid_id(clock)) {
                return Err(ValueEnvelopeReason::InvalidFeedbackBoundary);
            }
        }
        FeedbackBoundaryKind::State => {
            if boundary.delay_ticks != 0 || boundary.clock.is_some() {
                return Err(ValueEnvelopeReason::InvalidFeedbackBoundary);
            }
        }
    }
    Ok(())
}

/// Prove that removing the exact feedback-boundary edges leaves an acyclic
/// dependency graph.
///
/// `removed` is caller-owned scratch with at least one slot per node. This
/// keeps the portable admission check allocator-free.
pub fn validate_feedback_graph(
    nodes: &[InstancePath<'_>],
    cords: &[ResolvedPlanCord<'_>],
    boundaries: &[PlanFeedbackBoundary<'_>],
    removed: &mut [bool],
) -> Result<(), ValueEnvelopeReason> {
    if removed.len() < nodes.len()
        || nodes.iter().enumerate().any(|(index, node)| {
            InstancePath::new(node.as_str()).is_err() || nodes[..index].contains(node)
        })
    {
        return Err(ValueEnvelopeReason::InvalidFeedbackCycle);
    }
    removed[..nodes.len()].fill(false);

    for (index, boundary) in boundaries.iter().enumerate() {
        validate_feedback_boundary(*boundary)?;
        let Some(cord) = cords.iter().find(|cord| cord.id == boundary.cord) else {
            return Err(ValueEnvelopeReason::InvalidFeedbackBoundary);
        };
        if boundaries[..index]
            .iter()
            .any(|prior| prior.cord == boundary.cord)
            || (cord.from.node != boundary.node && cord.to.node != boundary.node)
        {
            return Err(ValueEnvelopeReason::InvalidFeedbackBoundary);
        }
    }

    for cord in cords {
        if !nodes.contains(&cord.from.node) || !nodes.contains(&cord.to.node) {
            return Err(ValueEnvelopeReason::InvalidFeedbackCycle);
        }
    }

    let mut remaining = nodes.len();
    while remaining != 0 {
        let Some(candidate) = nodes.iter().enumerate().find_map(|(index, node)| {
            if removed[index] {
                return None;
            }
            let has_incoming = cords.iter().any(|cord| {
                cord.to.node == *node
                    && !boundaries.iter().any(|boundary| boundary.cord == cord.id)
                    && nodes
                        .iter()
                        .position(|candidate| *candidate == cord.from.node)
                        .is_some_and(|source| !removed[source])
            });
            (!has_incoming).then_some(index)
        }) else {
            return Err(ValueEnvelopeReason::InvalidFeedbackCycle);
        };
        removed[candidate] = true;
        remaining -= 1;
    }
    Ok(())
}
