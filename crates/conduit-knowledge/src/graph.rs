//! Bounded graph-shaped knowledge built from exact citation values.
//!
//! A graph edge is not source support. Every traversed claim retains and
//! validates its own citation, disposition, validity, sensitivity, schema,
//! snapshot, and provider facts.

use conduit_core::{
    ConfigContract, ConfigFieldContract, Direction, Id, NodeContract, SemanticHash, TypeContractRef,
};
use conduit_panel::Node;
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};

use super::{
    CITATION_TYPE, RetrievalResult, TEXT_TYPE, decode_result, exact_u64, field, no_config, port,
    type_ref,
};

pub const MAXIMUM_CLAIMS: usize = 8;
pub const MAXIMUM_QUERY_DEPTH: usize = 2;
pub const MAXIMUM_QUERY_BREADTH: usize = 4;
pub const MAXIMUM_QUERY_PATHS: usize = 4;
pub const MAXIMUM_QUERY_RESULTS: usize = 4;
pub const MAXIMUM_GRAPH_RETAINED_BYTES: usize = 4096;
pub const MAXIMUM_GRAPH_WORK: usize = 128;
pub const MAXIMUM_GRAPH_EVIDENCE_EVENTS: usize = 64;

pub const ENTITY_SCHEMA_IDENTITY: [u8; 32] = [0x81; 32];
pub const RELATION_SCHEMA_IDENTITY: [u8; 32] = [0x82; 32];
pub const GRAPH_SCHEMA_IDENTITY: [u8; 32] = [0x83; 32];
pub const GRAPH_SNAPSHOT_IDENTITY: [u8; 32] = [0x84; 32];
pub const GRAPH_PROVIDER_IDENTITY: [u8; 32] = [0x85; 32];
pub const SUBJECT_ENTITY_IDENTITY: [u8; 32] = [0x86; 32];
pub const OBJECT_ENTITY_IDENTITY: [u8; 32] = [0x87; 32];
pub const RELATION_IDENTITY: [u8; 32] = [0x88; 32];
pub const CLAIM_IDENTITY: [u8; 32] = [0x89; 32];
pub const CONFIDENCE_DESCRIPTOR_IDENTITY: [u8; 32] = [0x8a; 32];

pub const ENTITY_SCHEMA_IDENTITY_TEXT: &str =
    "sha256:8181818181818181818181818181818181818181818181818181818181818181";
pub const RELATION_SCHEMA_IDENTITY_TEXT: &str =
    "sha256:8282828282828282828282828282828282828282828282828282828282828282";
pub const GRAPH_SCHEMA_IDENTITY_TEXT: &str =
    "sha256:8383838383838383838383838383838383838383838383838383838383838383";
pub const GRAPH_SNAPSHOT_IDENTITY_TEXT: &str =
    "sha256:8484848484848484848484848484848484848484848484848484848484848484";
pub const GRAPH_PROVIDER_IDENTITY_TEXT: &str =
    "sha256:8585858585858585858585858585858585858585858585858585858585858585";
pub const SUBJECT_ENTITY_IDENTITY_TEXT: &str =
    "sha256:8686868686868686868686868686868686868686868686868686868686868686";
pub const OBJECT_ENTITY_IDENTITY_TEXT: &str =
    "sha256:8787878787878787878787878787878787878787878787878787878787878787";
pub const RELATION_IDENTITY_TEXT: &str =
    "sha256:8888888888888888888888888888888888888888888888888888888888888888";
pub const CLAIM_IDENTITY_TEXT: &str =
    "sha256:8989898989898989898989898989898989898989898989898989898989898989";
pub const CONFIDENCE_DESCRIPTOR_IDENTITY_TEXT: &str =
    "sha256:8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a";

pub const ENTITY_TYPE: TypeContractRef<'static> = type_ref("knowledge/entity", 0x91);
pub const RELATION_TYPE: TypeContractRef<'static> = type_ref("knowledge/relation", 0x92);
pub const CLAIM_TYPE: TypeContractRef<'static> = type_ref("knowledge/claim", 0x93);
pub const GRAPH_SNAPSHOT_TYPE: TypeContractRef<'static> =
    type_ref("knowledge/graph-snapshot", 0x94);
pub const GRAPH_QUERY_TYPE: TypeContractRef<'static> = type_ref("knowledge/graph-query", 0x95);
pub const GRAPH_RESULTS_TYPE: TypeContractRef<'static> = type_ref("knowledge/graph-results", 0x96);

const U64_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/u64"),
    schema_version: 0,
    semantic_hash: conduit_core::SemanticHash::from_bytes([
        0xf9, 0xba, 0xd3, 0xea, 0x53, 0xd3, 0xca, 0x01, 0xa0, 0xa4, 0xd6, 0x9f, 0x86, 0xc8, 0x25,
        0x65, 0x17, 0x07, 0x16, 0x45, 0xea, 0x7d, 0x68, 0xef, 0x63, 0x6b, 0x6d, 0x94, 0x87, 0x70,
        0xf0, 0xec,
    ]),
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityIdentity {
    pub schema: [u8; 32],
    pub schema_version: u16,
    pub id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationIdentity {
    pub schema: [u8; 32],
    pub schema_version: u16,
    pub id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimDisposition {
    Supported,
    Contradicted,
    Superseded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ClaimSensitivity {
    Public,
    Protected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claim {
    pub schema: [u8; 32],
    pub schema_version: u16,
    pub identity: [u8; 32],
    pub subject: EntityIdentity,
    pub relation: RelationIdentity,
    pub object: EntityIdentity,
    pub source_identity: [u8; 32],
    pub source_revision_identity: [u8; 32],
    pub source_support: Option<RetrievalResult>,
    pub disposition: ClaimDisposition,
    pub confidence_descriptor: [u8; 32],
    pub valid_from_tick: u64,
    pub valid_until_tick: u64,
    pub sensitivity: ClaimSensitivity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphSnapshot {
    pub schema: [u8; 32],
    pub identity: [u8; 32],
    pub provider: [u8; 32],
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub coverage_complete: bool,
    pub claims: Vec<Claim>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphQuery {
    pub start: EntityIdentity,
    pub relation: RelationIdentity,
    pub maximum_depth: usize,
    pub maximum_breadth: usize,
    pub maximum_paths: usize,
    pub maximum_results: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_work: usize,
    pub maximum_evidence_events: usize,
    pub now_tick: u64,
    pub access_granted: bool,
    pub maximum_sensitivity: ClaimSensitivity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPath {
    pub claims: Vec<Claim>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphResults {
    pub snapshot: [u8; 32],
    pub provider: [u8; 32],
    pub paths: Vec<GraphPath>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphReason {
    WrongType,
    EntityCollision,
    SchemaMismatch,
    SnapshotMismatch,
    ProviderMismatch,
    CitationRevisionMismatch,
    UnsupportedClaim,
    Contradicted,
    Superseded,
    MissingFact,
    TraversalBound,
    Cycle,
    StaleSnapshot,
    PartialSnapshot,
    Unauthorized,
    SensitivityLeak,
    ProviderLost,
    Cancellation,
}

impl GraphReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::WrongType => "CND-KGRAPH-001",
            Self::EntityCollision => "CND-KGRAPH-002",
            Self::SchemaMismatch => "CND-KGRAPH-003",
            Self::SnapshotMismatch => "CND-KGRAPH-004",
            Self::ProviderMismatch => "CND-KGRAPH-005",
            Self::CitationRevisionMismatch => "CND-KGRAPH-006",
            Self::UnsupportedClaim => "CND-KGRAPH-007",
            Self::Contradicted => "CND-KGRAPH-008",
            Self::Superseded => "CND-KGRAPH-009",
            Self::MissingFact => "CND-KGRAPH-010",
            Self::TraversalBound => "CND-KGRAPH-011",
            Self::Cycle => "CND-KGRAPH-012",
            Self::StaleSnapshot => "CND-KGRAPH-013",
            Self::PartialSnapshot => "CND-KGRAPH-014",
            Self::Unauthorized => "CND-KGRAPH-015",
            Self::SensitivityLeak => "CND-KGRAPH-016",
            Self::ProviderLost => "CND-KGRAPH-017",
            Self::Cancellation => "CND-KGRAPH-018",
        }
    }
}

fn validate_query_bounds(query: GraphQuery) -> Result<(), GraphReason> {
    for (actual, maximum) in [
        (query.maximum_depth, MAXIMUM_QUERY_DEPTH),
        (query.maximum_breadth, MAXIMUM_QUERY_BREADTH),
        (query.maximum_paths, MAXIMUM_QUERY_PATHS),
        (query.maximum_results, MAXIMUM_QUERY_RESULTS),
        (query.maximum_retained_bytes, MAXIMUM_GRAPH_RETAINED_BYTES),
        (query.maximum_work, MAXIMUM_GRAPH_WORK),
        (query.maximum_evidence_events, MAXIMUM_GRAPH_EVIDENCE_EVENTS),
    ] {
        if actual == 0 || actual > maximum {
            return Err(GraphReason::TraversalBound);
        }
    }
    Ok(())
}

fn validate_claim(claim: &Claim, query: GraphQuery) -> Result<(), GraphReason> {
    if claim.schema != GRAPH_SCHEMA_IDENTITY
        || claim.schema_version != 0
        || claim.subject.schema != ENTITY_SCHEMA_IDENTITY
        || claim.subject.schema_version != 0
        || claim.object.schema != ENTITY_SCHEMA_IDENTITY
        || claim.object.schema_version != 0
        || claim.relation.schema != RELATION_SCHEMA_IDENTITY
        || claim.relation.schema_version != 0
    {
        return Err(GraphReason::SchemaMismatch);
    }
    if claim.valid_from_tick > query.now_tick || claim.valid_until_tick < query.now_tick {
        return Err(GraphReason::StaleSnapshot);
    }
    if !query.access_granted {
        return Err(GraphReason::Unauthorized);
    }
    if claim.sensitivity > query.maximum_sensitivity {
        return Err(GraphReason::SensitivityLeak);
    }
    match claim.disposition {
        ClaimDisposition::Supported => {}
        ClaimDisposition::Contradicted => return Err(GraphReason::Contradicted),
        ClaimDisposition::Superseded => return Err(GraphReason::Superseded),
    }
    let support = claim
        .source_support
        .as_ref()
        .ok_or(GraphReason::UnsupportedClaim)?;
    if support.start >= support.end || support.excerpt.is_empty() {
        return Err(GraphReason::UnsupportedClaim);
    }
    if support.source != claim.source_identity || support.revision != claim.source_revision_identity
    {
        return Err(GraphReason::CitationRevisionMismatch);
    }
    Ok(())
}

pub fn traverse(
    snapshot: &GraphSnapshot,
    query: GraphQuery,
    provider_available: bool,
    cancelled: bool,
) -> Result<GraphResults, GraphReason> {
    validate_query_bounds(query)?;
    if cancelled {
        return Err(GraphReason::Cancellation);
    }
    if !provider_available {
        return Err(GraphReason::ProviderLost);
    }
    if snapshot.schema != GRAPH_SCHEMA_IDENTITY {
        return Err(GraphReason::SchemaMismatch);
    }
    if query.start.schema != ENTITY_SCHEMA_IDENTITY
        || query.start.schema_version != 0
        || query.relation.schema != RELATION_SCHEMA_IDENTITY
        || query.relation.schema_version != 0
    {
        return Err(GraphReason::SchemaMismatch);
    }
    if snapshot.identity != GRAPH_SNAPSHOT_IDENTITY {
        return Err(GraphReason::SnapshotMismatch);
    }
    if snapshot.provider != GRAPH_PROVIDER_IDENTITY {
        return Err(GraphReason::ProviderMismatch);
    }
    if query.now_tick < snapshot.observed_at_tick || query.now_tick > snapshot.valid_until_tick {
        return Err(GraphReason::StaleSnapshot);
    }
    if !snapshot.coverage_complete {
        return Err(GraphReason::PartialSnapshot);
    }
    if snapshot.claims.is_empty() {
        return Err(GraphReason::MissingFact);
    }
    if snapshot.claims.len() > MAXIMUM_CLAIMS {
        return Err(GraphReason::TraversalBound);
    }
    let mut entities = Vec::with_capacity(snapshot.claims.len() * 2);
    for claim in &snapshot.claims {
        for entity in [claim.subject, claim.object] {
            if entities.iter().any(|earlier: &EntityIdentity| {
                earlier.id == entity.id
                    && (earlier.schema != entity.schema
                        || earlier.schema_version != entity.schema_version)
            }) {
                return Err(GraphReason::EntityCollision);
            }
            entities.push(entity);
        }
    }

    let mut frontier = vec![(query.start, Vec::<Claim>::new())];
    let mut paths = Vec::new();
    let mut work = 0usize;
    let mut retained = 0usize;
    let mut evidence = 3usize;

    for _ in 0..query.maximum_depth {
        let mut next = Vec::new();
        for (entity, prefix) in frontier {
            let mut breadth = 0usize;
            for claim in &snapshot.claims {
                work = work.checked_add(1).ok_or(GraphReason::TraversalBound)?;
                if work > query.maximum_work {
                    return Err(GraphReason::TraversalBound);
                }
                if claim.subject != entity || claim.relation != query.relation {
                    continue;
                }
                breadth += 1;
                if breadth > query.maximum_breadth {
                    return Err(GraphReason::TraversalBound);
                }
                validate_claim(claim, query)?;
                if claim.object == query.start
                    || prefix
                        .iter()
                        .any(|edge| edge.subject == claim.object || edge.object == claim.object)
                {
                    return Err(GraphReason::Cycle);
                }
                let mut path = prefix.clone();
                path.push(claim.clone());
                retained = retained
                    .checked_add(path.iter().map(encoded_claim_len).sum::<usize>())
                    .ok_or(GraphReason::TraversalBound)?;
                if retained > query.maximum_retained_bytes {
                    return Err(GraphReason::TraversalBound);
                }
                evidence = evidence.checked_add(1).ok_or(GraphReason::TraversalBound)?;
                if evidence > query.maximum_evidence_events {
                    return Err(GraphReason::TraversalBound);
                }
                if paths.len() >= query.maximum_paths || paths.len() >= query.maximum_results {
                    return Err(GraphReason::TraversalBound);
                }
                paths.push(GraphPath {
                    claims: path.clone(),
                });
                next.push((claim.object, path));
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    if paths.is_empty() {
        return Err(GraphReason::MissingFact);
    }
    Ok(GraphResults {
        snapshot: snapshot.identity,
        provider: snapshot.provider,
        paths,
    })
}

fn encoded_claim_len(claim: &Claim) -> usize {
    384 + claim
        .source_support
        .as_ref()
        .map_or(0, |support| 74 + support.excerpt.len())
}

fn put_u16(bytes: &mut Vec<u8>, value: usize) -> Result<(), GraphReason> {
    bytes.extend_from_slice(
        &u16::try_from(value)
            .map_err(|_| GraphReason::TraversalBound)?
            .to_le_bytes(),
    );
    Ok(())
}

fn take_array<const N: usize>(bytes: &[u8], offset: &mut usize) -> Result<[u8; N], GraphReason> {
    let end = offset.checked_add(N).ok_or(GraphReason::WrongType)?;
    let slice = bytes.get(*offset..end).ok_or(GraphReason::WrongType)?;
    let mut value = [0; N];
    value.copy_from_slice(slice);
    *offset = end;
    Ok(value)
}

fn take_u8(bytes: &[u8], offset: &mut usize) -> Result<u8, GraphReason> {
    Ok(take_array::<1>(bytes, offset)?[0])
}

fn take_u16(bytes: &[u8], offset: &mut usize) -> Result<usize, GraphReason> {
    Ok(usize::from(u16::from_le_bytes(take_array(bytes, offset)?)))
}

fn take_u64(bytes: &[u8], offset: &mut usize) -> Result<u64, GraphReason> {
    Ok(u64::from_le_bytes(take_array(bytes, offset)?))
}

fn encode_claim(claim: &Claim) -> Result<Vec<u8>, GraphReason> {
    let mut bytes = Vec::with_capacity(encoded_claim_len(claim));
    bytes.extend_from_slice(b"KCL0");
    for value in [
        claim.schema,
        claim.identity,
        claim.subject.schema,
        claim.relation.schema,
        claim.object.schema,
        claim.confidence_descriptor,
    ] {
        bytes.extend_from_slice(&value);
    }
    bytes.extend_from_slice(&claim.schema_version.to_le_bytes());
    bytes.extend_from_slice(&claim.subject.schema_version.to_le_bytes());
    bytes.extend_from_slice(&claim.subject.id);
    bytes.extend_from_slice(&claim.relation.schema_version.to_le_bytes());
    bytes.extend_from_slice(&claim.relation.id);
    bytes.extend_from_slice(&claim.object.schema_version.to_le_bytes());
    bytes.extend_from_slice(&claim.object.id);
    bytes.extend_from_slice(&claim.source_identity);
    bytes.extend_from_slice(&claim.source_revision_identity);
    bytes.push(match claim.disposition {
        ClaimDisposition::Supported => 0,
        ClaimDisposition::Contradicted => 1,
        ClaimDisposition::Superseded => 2,
    });
    bytes.extend_from_slice(&claim.valid_from_tick.to_le_bytes());
    bytes.extend_from_slice(&claim.valid_until_tick.to_le_bytes());
    bytes.push(match claim.sensitivity {
        ClaimSensitivity::Public => 0,
        ClaimSensitivity::Protected => 1,
    });
    if let Some(support) = &claim.source_support {
        let encoded = super::encode_result(b"KCT0", support);
        put_u16(&mut bytes, encoded.len())?;
        bytes.extend_from_slice(&encoded);
    } else {
        put_u16(&mut bytes, 0)?;
    }
    Ok(bytes)
}

fn decode_claim(bytes: &[u8]) -> Result<Claim, GraphReason> {
    if bytes.get(..4) != Some(b"KCL0") {
        return Err(GraphReason::WrongType);
    }
    let mut offset = 4;
    let schema = take_array(bytes, &mut offset)?;
    let identity = take_array(bytes, &mut offset)?;
    let subject = EntityIdentity {
        schema: take_array(bytes, &mut offset)?,
        schema_version: 0,
        id: [0; 32],
    };
    let relation_schema = take_array(bytes, &mut offset)?;
    let object_schema = take_array(bytes, &mut offset)?;
    let confidence_descriptor = take_array(bytes, &mut offset)?;
    let schema_version = u16::from_le_bytes(take_array(bytes, &mut offset)?);
    let subject = EntityIdentity {
        schema: subject.schema,
        schema_version: u16::from_le_bytes(take_array(bytes, &mut offset)?),
        id: take_array(bytes, &mut offset)?,
    };
    let relation = RelationIdentity {
        schema: relation_schema,
        schema_version: u16::from_le_bytes(take_array(bytes, &mut offset)?),
        id: take_array(bytes, &mut offset)?,
    };
    let object = EntityIdentity {
        schema: object_schema,
        schema_version: u16::from_le_bytes(take_array(bytes, &mut offset)?),
        id: take_array(bytes, &mut offset)?,
    };
    let source_identity = take_array(bytes, &mut offset)?;
    let source_revision_identity = take_array(bytes, &mut offset)?;
    let disposition = match take_u8(bytes, &mut offset)? {
        0 => ClaimDisposition::Supported,
        1 => ClaimDisposition::Contradicted,
        2 => ClaimDisposition::Superseded,
        _ => return Err(GraphReason::WrongType),
    };
    let valid_from_tick = take_u64(bytes, &mut offset)?;
    let valid_until_tick = take_u64(bytes, &mut offset)?;
    let sensitivity = match take_u8(bytes, &mut offset)? {
        0 => ClaimSensitivity::Public,
        1 => ClaimSensitivity::Protected,
        _ => return Err(GraphReason::WrongType),
    };
    let support_len = take_u16(bytes, &mut offset)?;
    let support_end = offset
        .checked_add(support_len)
        .ok_or(GraphReason::WrongType)?;
    let source_support = if support_len == 0 {
        None
    } else {
        Some(
            decode_result(
                bytes
                    .get(offset..support_end)
                    .ok_or(GraphReason::WrongType)?,
                b"KCT0",
            )
            .map_err(|_| GraphReason::WrongType)?,
        )
    };
    if support_end != bytes.len() {
        return Err(GraphReason::WrongType);
    }
    Ok(Claim {
        schema,
        schema_version,
        identity,
        subject,
        relation,
        object,
        source_identity,
        source_revision_identity,
        source_support,
        disposition,
        confidence_descriptor,
        valid_from_tick,
        valid_until_tick,
        sensitivity,
    })
}

fn encode_snapshot(snapshot: &GraphSnapshot) -> Result<Vec<u8>, GraphReason> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"KGS0");
    bytes.extend_from_slice(&snapshot.schema);
    bytes.extend_from_slice(&snapshot.identity);
    bytes.extend_from_slice(&snapshot.provider);
    bytes.extend_from_slice(&snapshot.observed_at_tick.to_le_bytes());
    bytes.extend_from_slice(&snapshot.valid_until_tick.to_le_bytes());
    bytes.push(u8::from(snapshot.coverage_complete));
    put_u16(&mut bytes, snapshot.claims.len())?;
    for claim in &snapshot.claims {
        let encoded = encode_claim(claim)?;
        put_u16(&mut bytes, encoded.len())?;
        bytes.extend_from_slice(&encoded);
    }
    Ok(bytes)
}

fn decode_snapshot(bytes: &[u8]) -> Result<GraphSnapshot, GraphReason> {
    if bytes.get(..4) != Some(b"KGS0") {
        return Err(GraphReason::WrongType);
    }
    let mut offset = 4;
    let schema = take_array(bytes, &mut offset)?;
    let identity = take_array(bytes, &mut offset)?;
    let provider = take_array(bytes, &mut offset)?;
    let observed_at_tick = take_u64(bytes, &mut offset)?;
    let valid_until_tick = take_u64(bytes, &mut offset)?;
    let coverage_complete = take_u8(bytes, &mut offset)? != 0;
    let count = take_u16(bytes, &mut offset)?;
    if count > MAXIMUM_CLAIMS {
        return Err(GraphReason::TraversalBound);
    }
    let mut claims = Vec::with_capacity(count);
    for _ in 0..count {
        let len = take_u16(bytes, &mut offset)?;
        let end = offset.checked_add(len).ok_or(GraphReason::WrongType)?;
        claims.push(decode_claim(
            bytes.get(offset..end).ok_or(GraphReason::WrongType)?,
        )?);
        offset = end;
    }
    if offset != bytes.len() {
        return Err(GraphReason::WrongType);
    }
    Ok(GraphSnapshot {
        schema,
        identity,
        provider,
        observed_at_tick,
        valid_until_tick,
        coverage_complete,
        claims,
    })
}

fn encode_query(query: GraphQuery) -> Result<Vec<u8>, GraphReason> {
    validate_query_bounds(query)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"KGQ0");
    bytes.extend_from_slice(&query.start.schema);
    bytes.extend_from_slice(&query.start.schema_version.to_le_bytes());
    bytes.extend_from_slice(&query.start.id);
    bytes.extend_from_slice(&query.relation.schema);
    bytes.extend_from_slice(&query.relation.schema_version.to_le_bytes());
    bytes.extend_from_slice(&query.relation.id);
    for value in [
        query.maximum_depth,
        query.maximum_breadth,
        query.maximum_paths,
        query.maximum_results,
        query.maximum_retained_bytes,
        query.maximum_work,
        query.maximum_evidence_events,
    ] {
        put_u16(&mut bytes, value)?;
    }
    bytes.extend_from_slice(&query.now_tick.to_le_bytes());
    bytes.push(u8::from(query.access_granted));
    bytes.push(match query.maximum_sensitivity {
        ClaimSensitivity::Public => 0,
        ClaimSensitivity::Protected => 1,
    });
    Ok(bytes)
}

fn decode_query(bytes: &[u8]) -> Result<GraphQuery, GraphReason> {
    if bytes.get(..4) != Some(b"KGQ0") {
        return Err(GraphReason::WrongType);
    }
    let mut offset = 4;
    let query = GraphQuery {
        start: EntityIdentity {
            schema: take_array(bytes, &mut offset)?,
            schema_version: u16::from_le_bytes(take_array(bytes, &mut offset)?),
            id: take_array(bytes, &mut offset)?,
        },
        relation: RelationIdentity {
            schema: take_array(bytes, &mut offset)?,
            schema_version: u16::from_le_bytes(take_array(bytes, &mut offset)?),
            id: take_array(bytes, &mut offset)?,
        },
        maximum_depth: take_u16(bytes, &mut offset)?,
        maximum_breadth: take_u16(bytes, &mut offset)?,
        maximum_paths: take_u16(bytes, &mut offset)?,
        maximum_results: take_u16(bytes, &mut offset)?,
        maximum_retained_bytes: take_u16(bytes, &mut offset)?,
        maximum_work: take_u16(bytes, &mut offset)?,
        maximum_evidence_events: take_u16(bytes, &mut offset)?,
        now_tick: take_u64(bytes, &mut offset)?,
        access_granted: take_u8(bytes, &mut offset)? != 0,
        maximum_sensitivity: match take_u8(bytes, &mut offset)? {
            0 => ClaimSensitivity::Public,
            1 => ClaimSensitivity::Protected,
            _ => return Err(GraphReason::WrongType),
        },
    };
    if offset != bytes.len() {
        return Err(GraphReason::WrongType);
    }
    validate_query_bounds(query)?;
    Ok(query)
}

fn encode_results(results: &GraphResults) -> Result<Vec<u8>, GraphReason> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"KGR0");
    bytes.extend_from_slice(&results.snapshot);
    bytes.extend_from_slice(&results.provider);
    put_u16(&mut bytes, results.paths.len())?;
    for path in &results.paths {
        put_u16(&mut bytes, path.claims.len())?;
        for claim in &path.claims {
            let encoded = encode_claim(claim)?;
            put_u16(&mut bytes, encoded.len())?;
            bytes.extend_from_slice(&encoded);
        }
    }
    Ok(bytes)
}

fn decode_results(bytes: &[u8]) -> Result<GraphResults, GraphReason> {
    if bytes.get(..4) != Some(b"KGR0") {
        return Err(GraphReason::WrongType);
    }
    let mut offset = 4;
    let snapshot = take_array(bytes, &mut offset)?;
    let provider = take_array(bytes, &mut offset)?;
    let path_count = take_u16(bytes, &mut offset)?;
    if path_count > MAXIMUM_QUERY_RESULTS {
        return Err(GraphReason::TraversalBound);
    }
    let mut paths = Vec::with_capacity(path_count);
    for _ in 0..path_count {
        let claim_count = take_u16(bytes, &mut offset)?;
        if claim_count == 0 || claim_count > MAXIMUM_QUERY_DEPTH {
            return Err(GraphReason::TraversalBound);
        }
        let mut claims = Vec::with_capacity(claim_count);
        for _ in 0..claim_count {
            let len = take_u16(bytes, &mut offset)?;
            let end = offset.checked_add(len).ok_or(GraphReason::WrongType)?;
            claims.push(decode_claim(
                bytes.get(offset..end).ok_or(GraphReason::WrongType)?,
            )?);
            offset = end;
        }
        paths.push(GraphPath { claims });
    }
    if offset != bytes.len() {
        return Err(GraphReason::WrongType);
    }
    Ok(GraphResults {
        snapshot,
        provider,
        paths,
    })
}

const CLAIM_FIELDS: [ConfigFieldContract<'static>; 17] = [
    field("graph_schema_identity", super::TEXT_TYPE),
    field("entity_schema_identity", super::TEXT_TYPE),
    field("entity_schema_version", U64_TYPE),
    field("relation_schema_identity", super::TEXT_TYPE),
    field("relation_schema_version", U64_TYPE),
    field("claim_schema_version", U64_TYPE),
    field("claim_identity", super::TEXT_TYPE),
    field("subject_identity", super::TEXT_TYPE),
    field("relation_identity", super::TEXT_TYPE),
    field("object_identity", super::TEXT_TYPE),
    field("source_identity", super::TEXT_TYPE),
    field("source_revision_identity", super::TEXT_TYPE),
    field("confidence_descriptor_identity", super::TEXT_TYPE),
    field("disposition", super::TEXT_TYPE),
    field("valid_from_tick", U64_TYPE),
    field("valid_until_tick", U64_TYPE),
    field("sensitivity", super::TEXT_TYPE),
];
const SNAPSHOT_FIELDS: [ConfigFieldContract<'static>; 9] = [
    field("graph_schema_identity", super::TEXT_TYPE),
    field("snapshot_identity", super::TEXT_TYPE),
    field("provider_identity", super::TEXT_TYPE),
    field("coverage", super::TEXT_TYPE),
    field("observed_at_tick", U64_TYPE),
    field("valid_until_tick", U64_TYPE),
    field("maximum_claims", U64_TYPE),
    field("maximum_retained_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const QUERY_FIELDS: [ConfigFieldContract<'static>; 15] = [
    field("entity_schema_identity", super::TEXT_TYPE),
    field("entity_schema_version", U64_TYPE),
    field("relation_schema_identity", super::TEXT_TYPE),
    field("relation_schema_version", U64_TYPE),
    field("start_entity_identity", super::TEXT_TYPE),
    field("relation_identity", super::TEXT_TYPE),
    field("maximum_depth", U64_TYPE),
    field("maximum_breadth", U64_TYPE),
    field("maximum_paths", U64_TYPE),
    field("maximum_results", U64_TYPE),
    field("maximum_retained_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
    field("now_tick", U64_TYPE),
    field("access_scope", super::TEXT_TYPE),
];
const TRAVERSE_FIELDS: [ConfigFieldContract<'static>; 10] = [
    field("graph_schema_identity", super::TEXT_TYPE),
    field("snapshot_identity", super::TEXT_TYPE),
    field("provider_identity", super::TEXT_TYPE),
    field("maximum_depth", U64_TYPE),
    field("maximum_breadth", U64_TYPE),
    field("maximum_paths", U64_TYPE),
    field("maximum_results", U64_TYPE),
    field("maximum_retained_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
];

const CLAIM_INPUTS: [conduit_core::PortContract<'static>; 1] =
    [port("citation", Direction::Input, CITATION_TYPE)];
const CLAIM_OUTPUTS: [conduit_core::PortContract<'static>; 1] =
    [port("claim", Direction::Output, CLAIM_TYPE)];
const SNAPSHOT_INPUTS: [conduit_core::PortContract<'static>; 1] =
    [port("claim", Direction::Input, CLAIM_TYPE)];
const SNAPSHOT_OUTPUTS: [conduit_core::PortContract<'static>; 1] =
    [port("snapshot", Direction::Output, GRAPH_SNAPSHOT_TYPE)];
const QUERY_OUTPUTS: [conduit_core::PortContract<'static>; 1] =
    [port("query", Direction::Output, GRAPH_QUERY_TYPE)];
const TRAVERSE_INPUTS: [conduit_core::PortContract<'static>; 2] = [
    port("snapshot", Direction::Input, GRAPH_SNAPSHOT_TYPE),
    port("query", Direction::Input, GRAPH_QUERY_TYPE),
];
const RESULTS_OUTPUTS: [conduit_core::PortContract<'static>; 1] =
    [port("results", Direction::Output, GRAPH_RESULTS_TYPE)];
const RESULTS_INPUTS: [conduit_core::PortContract<'static>; 1] =
    [port("results", Direction::Input, GRAPH_RESULTS_TYPE)];
const SUMMARY_OUTPUTS: [conduit_core::PortContract<'static>; 1] = super::TEXT_OUTPUTS;

pub const CLAIM_FROM_CITATION_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/claim/from-citation"),
    config: ConfigContract {
        fields: &CLAIM_FIELDS,
    },
    inputs: &CLAIM_INPUTS,
    outputs: &CLAIM_OUTPUTS,
};
pub const GRAPH_FIXTURE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/graph/fixture"),
    config: ConfigContract {
        fields: &SNAPSHOT_FIELDS,
    },
    inputs: &SNAPSHOT_INPUTS,
    outputs: &SNAPSHOT_OUTPUTS,
};
pub const GRAPH_QUERY_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/graph/query/literal"),
    config: ConfigContract {
        fields: &QUERY_FIELDS,
    },
    inputs: &[],
    outputs: &QUERY_OUTPUTS,
};
pub const GRAPH_TRAVERSE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/graph/traverse"),
    config: ConfigContract {
        fields: &TRAVERSE_FIELDS,
    },
    inputs: &TRAVERSE_INPUTS,
    outputs: &RESULTS_OUTPUTS,
};
pub const GRAPH_RESULTS_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/graph/results/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &RESULTS_INPUTS,
    outputs: &SUMMARY_OUTPUTS,
};

pub const GRAPH_CONTRACTS: [&NodeContract<'static>; 5] = [
    &CLAIM_FROM_CITATION_CONTRACT,
    &GRAPH_FIXTURE_CONTRACT,
    &GRAPH_QUERY_LITERAL_CONTRACT,
    &GRAPH_TRAVERSE_CONTRACT,
    &GRAPH_RESULTS_INSPECT_CONTRACT,
];

pub fn register_graph_contracts(registry: &mut Registry) {
    for contract in GRAPH_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

fn exact_text(node: &Node, key: &str, value: &str) -> bool {
    node.config(key) == Some(value)
}

fn validate_claim_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == CLAIM_FIELDS.len()
        && exact_text(node, "graph_schema_identity", GRAPH_SCHEMA_IDENTITY_TEXT)
        && exact_text(node, "entity_schema_identity", ENTITY_SCHEMA_IDENTITY_TEXT)
        && exact_u64(node, "entity_schema_version") == Some(0)
        && exact_text(
            node,
            "relation_schema_identity",
            RELATION_SCHEMA_IDENTITY_TEXT,
        )
        && exact_u64(node, "relation_schema_version") == Some(0)
        && exact_u64(node, "claim_schema_version") == Some(0)
        && exact_text(node, "claim_identity", CLAIM_IDENTITY_TEXT)
        && exact_text(node, "subject_identity", SUBJECT_ENTITY_IDENTITY_TEXT)
        && exact_text(node, "relation_identity", RELATION_IDENTITY_TEXT)
        && exact_text(node, "object_identity", OBJECT_ENTITY_IDENTITY_TEXT)
        && exact_text(node, "source_identity", super::SOURCE_IDENTITY_TEXT)
        && exact_text(
            node,
            "source_revision_identity",
            super::REVISION_IDENTITY_TEXT,
        )
        && exact_text(
            node,
            "confidence_descriptor_identity",
            CONFIDENCE_DESCRIPTOR_IDENTITY_TEXT,
        )
        && exact_text(node, "disposition", "supported")
        && exact_u64(node, "valid_from_tick") == Some(10)
        && exact_u64(node, "valid_until_tick") == Some(20)
        && exact_text(node, "sensitivity", "public"))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KGRAPH-003",
            "claim requires exact entity, relation, claim, confidence, validity, and sensitivity identities",
        )
    })
}

fn validate_snapshot_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == SNAPSHOT_FIELDS.len()
        && exact_text(node, "graph_schema_identity", GRAPH_SCHEMA_IDENTITY_TEXT)
        && exact_text(node, "snapshot_identity", GRAPH_SNAPSHOT_IDENTITY_TEXT)
        && exact_text(node, "provider_identity", GRAPH_PROVIDER_IDENTITY_TEXT)
        && exact_text(node, "coverage", "complete")
        && exact_u64(node, "observed_at_tick") == Some(10)
        && exact_u64(node, "valid_until_tick") == Some(20)
        && exact_u64(node, "maximum_claims") == Some(MAXIMUM_CLAIMS as u64)
        && exact_u64(node, "maximum_retained_bytes")
            == Some(MAXIMUM_GRAPH_RETAINED_BYTES as u64)
        && exact_u64(node, "maximum_work") == Some(MAXIMUM_GRAPH_WORK as u64))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KGRAPH-004",
            "graph fixture requires the exact bounded schema, snapshot, provider, coverage, and limits",
        )
    })
}

fn query_from_config(node: &Node) -> Option<GraphQuery> {
    Some(GraphQuery {
        start: EntityIdentity {
            schema: ENTITY_SCHEMA_IDENTITY,
            schema_version: 0,
            id: SUBJECT_ENTITY_IDENTITY,
        },
        relation: RelationIdentity {
            schema: RELATION_SCHEMA_IDENTITY,
            schema_version: 0,
            id: RELATION_IDENTITY,
        },
        maximum_depth: usize::try_from(exact_u64(node, "maximum_depth")?).ok()?,
        maximum_breadth: usize::try_from(exact_u64(node, "maximum_breadth")?).ok()?,
        maximum_paths: usize::try_from(exact_u64(node, "maximum_paths")?).ok()?,
        maximum_results: usize::try_from(exact_u64(node, "maximum_results")?).ok()?,
        maximum_retained_bytes: usize::try_from(exact_u64(node, "maximum_retained_bytes")?).ok()?,
        maximum_work: usize::try_from(exact_u64(node, "maximum_work")?).ok()?,
        maximum_evidence_events: usize::try_from(exact_u64(node, "maximum_evidence_events")?)
            .ok()?,
        now_tick: exact_u64(node, "now_tick")?,
        access_granted: exact_text(node, "access_scope", "public"),
        maximum_sensitivity: ClaimSensitivity::Public,
    })
}

fn validate_query_config(node: &Node) -> Result<(), ResolutionError> {
    let query = query_from_config(node).ok_or_else(|| {
        ResolutionError::new("CND-KGRAPH-011", "graph query bounds are malformed")
    })?;
    (node.config.len() == QUERY_FIELDS.len()
        && exact_text(node, "entity_schema_identity", ENTITY_SCHEMA_IDENTITY_TEXT)
        && exact_u64(node, "entity_schema_version") == Some(0)
        && exact_text(
            node,
            "relation_schema_identity",
            RELATION_SCHEMA_IDENTITY_TEXT,
        )
        && exact_u64(node, "relation_schema_version") == Some(0)
        && exact_text(node, "start_entity_identity", SUBJECT_ENTITY_IDENTITY_TEXT)
        && exact_text(node, "relation_identity", RELATION_IDENTITY_TEXT)
        && query.maximum_depth == 1
        && query.maximum_breadth == 1
        && query.maximum_paths == 1
        && query.maximum_results == 1
        && query.maximum_retained_bytes == MAXIMUM_GRAPH_RETAINED_BYTES
        && query.maximum_work == MAXIMUM_GRAPH_WORK
        && query.maximum_evidence_events == MAXIMUM_GRAPH_EVIDENCE_EVENTS
        && query.now_tick == 12
        && query.access_granted
        && validate_query_bounds(query).is_ok())
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KGRAPH-011",
            "graph query requires exact identities, access, time, and finite traversal bounds",
        )
    })
}

fn validate_traverse_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == TRAVERSE_FIELDS.len()
        && exact_text(node, "graph_schema_identity", GRAPH_SCHEMA_IDENTITY_TEXT)
        && exact_text(node, "snapshot_identity", GRAPH_SNAPSHOT_IDENTITY_TEXT)
        && exact_text(node, "provider_identity", GRAPH_PROVIDER_IDENTITY_TEXT)
        && exact_u64(node, "maximum_depth") == Some(1)
        && exact_u64(node, "maximum_breadth") == Some(1)
        && exact_u64(node, "maximum_paths") == Some(1)
        && exact_u64(node, "maximum_results") == Some(1)
        && exact_u64(node, "maximum_retained_bytes") == Some(MAXIMUM_GRAPH_RETAINED_BYTES as u64)
        && exact_u64(node, "maximum_work") == Some(MAXIMUM_GRAPH_WORK as u64)
        && exact_u64(node, "maximum_evidence_events") == Some(MAXIMUM_GRAPH_EVIDENCE_EVENTS as u64))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KGRAPH-011",
            "traverse requires the exact snapshot, provider, and finite bounds",
        )
    })
}

fn runtime(reason: GraphReason) -> RuntimeError {
    RuntimeError::new(
        reason.code(),
        format!("bounded graph operation failed: {reason:?}"),
    )
}

struct ClaimFromCitation;
impl Handler for ClaimFromCitation {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [citation] = inputs else {
            return Err(runtime(GraphReason::WrongType));
        };
        if citation.value_type != CITATION_TYPE {
            return Err(runtime(GraphReason::WrongType));
        }
        let support =
            decode_result(&citation.bytes, b"KCT0").map_err(|_| runtime(GraphReason::WrongType))?;
        let claim = Claim {
            schema: GRAPH_SCHEMA_IDENTITY,
            schema_version: 0,
            identity: CLAIM_IDENTITY,
            subject: EntityIdentity {
                schema: ENTITY_SCHEMA_IDENTITY,
                schema_version: 0,
                id: SUBJECT_ENTITY_IDENTITY,
            },
            relation: RelationIdentity {
                schema: RELATION_SCHEMA_IDENTITY,
                schema_version: 0,
                id: RELATION_IDENTITY,
            },
            object: EntityIdentity {
                schema: ENTITY_SCHEMA_IDENTITY,
                schema_version: 0,
                id: OBJECT_ENTITY_IDENTITY,
            },
            source_identity: super::SOURCE_IDENTITY,
            source_revision_identity: super::REVISION_IDENTITY,
            source_support: Some(support),
            disposition: ClaimDisposition::Supported,
            confidence_descriptor: CONFIDENCE_DESCRIPTOR_IDENTITY,
            valid_from_tick: 10,
            valid_until_tick: 20,
            sensitivity: ClaimSensitivity::Public,
        };
        Ok(vec![Value {
            value_type: CLAIM_TYPE,
            bytes: encode_claim(&claim).map_err(runtime)?,
        }])
    }
}

struct GraphFixture;
impl Handler for GraphFixture {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [claim] = inputs else {
            return Err(runtime(GraphReason::WrongType));
        };
        if claim.value_type != CLAIM_TYPE {
            return Err(runtime(GraphReason::WrongType));
        }
        let snapshot = GraphSnapshot {
            schema: GRAPH_SCHEMA_IDENTITY,
            identity: GRAPH_SNAPSHOT_IDENTITY,
            provider: GRAPH_PROVIDER_IDENTITY,
            observed_at_tick: 10,
            valid_until_tick: 20,
            coverage_complete: true,
            claims: vec![decode_claim(&claim.bytes).map_err(runtime)?],
        };
        Ok(vec![Value {
            value_type: GRAPH_SNAPSHOT_TYPE,
            bytes: encode_snapshot(&snapshot).map_err(runtime)?,
        }])
    }
}

struct GraphQueryLiteral;
impl Handler for GraphQueryLiteral {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(GraphReason::WrongType));
        }
        let query = query_from_config(node).ok_or_else(|| runtime(GraphReason::TraversalBound))?;
        Ok(vec![Value {
            value_type: GRAPH_QUERY_TYPE,
            bytes: encode_query(query).map_err(runtime)?,
        }])
    }
}

struct GraphTraverse;
impl Handler for GraphTraverse {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [snapshot, query] = inputs else {
            return Err(runtime(GraphReason::WrongType));
        };
        if snapshot.value_type != GRAPH_SNAPSHOT_TYPE || query.value_type != GRAPH_QUERY_TYPE {
            return Err(runtime(GraphReason::WrongType));
        }
        let results = traverse(
            &decode_snapshot(&snapshot.bytes).map_err(runtime)?,
            decode_query(&query.bytes).map_err(runtime)?,
            true,
            false,
        )
        .map_err(runtime)?;
        Ok(vec![Value {
            value_type: GRAPH_RESULTS_TYPE,
            bytes: encode_results(&results).map_err(runtime)?,
        }])
    }
}

struct GraphResultsInspect;
impl Handler for GraphResultsInspect {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [results] = inputs else {
            return Err(runtime(GraphReason::WrongType));
        };
        if results.value_type != GRAPH_RESULTS_TYPE {
            return Err(runtime(GraphReason::WrongType));
        }
        let results = decode_results(&results.bytes).map_err(runtime)?;
        let claim = results
            .paths
            .first()
            .and_then(|path| path.claims.first())
            .ok_or_else(|| runtime(GraphReason::MissingFact))?;
        let support = claim
            .source_support
            .as_ref()
            .ok_or_else(|| runtime(GraphReason::UnsupportedClaim))?;
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: format!(
                "knowledge:graph:Conduit--keeps-distinct-->exact-plans[source:{}..{}]",
                support.start, support.end
            )
            .into_bytes(),
        }])
    }
}

pub fn register_deterministic_graph_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    register_graph_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validator) in [
        (
            &CLAIM_FROM_CITATION_CONTRACT,
            "conduit.knowledge/claim-from-citation",
            "conduit.knowledge/claim-from-citation-artifact",
            "knowledge-claim-from-citation",
            (|| Box::new(ClaimFromCitation) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_claim_config as conduit_runtime::ConfigValidator,
        ),
        (
            &GRAPH_FIXTURE_CONTRACT,
            "conduit.knowledge/graph-fixture",
            "conduit.knowledge/graph-fixture-artifact",
            "knowledge-graph-fixture",
            (|| Box::new(GraphFixture) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_snapshot_config as conduit_runtime::ConfigValidator,
        ),
        (
            &GRAPH_QUERY_LITERAL_CONTRACT,
            "conduit.knowledge/graph-query-literal",
            "conduit.knowledge/graph-query-literal-artifact",
            "knowledge-graph-query-literal",
            (|| Box::new(GraphQueryLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_query_config as conduit_runtime::ConfigValidator,
        ),
        (
            &GRAPH_TRAVERSE_CONTRACT,
            "conduit.knowledge/graph-traverse-reference",
            "conduit.knowledge/graph-traverse-reference-artifact",
            "knowledge-graph-traverse",
            (|| Box::new(GraphTraverse) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_traverse_config as conduit_runtime::ConfigValidator,
        ),
        (
            &GRAPH_RESULTS_INSPECT_CONTRACT,
            "conduit.knowledge/graph-results-inspect",
            "conduit.knowledge/graph-results-inspect-artifact",
            "knowledge-graph-results-inspect",
            (|| Box::new(GraphResultsInspect) as Box<dyn Handler>)
                as conduit_runtime::HandlerFactory,
            no_config as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("graph.rs"),
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

    fn support() -> RetrievalResult {
        RetrievalResult {
            source: super::super::SOURCE_IDENTITY,
            revision: super::super::REVISION_IDENTITY,
            start: 31,
            end: 42,
            excerpt: b"exact plans".to_vec(),
            score_milli: 1000,
        }
    }

    fn claim() -> Claim {
        Claim {
            schema: GRAPH_SCHEMA_IDENTITY,
            schema_version: 0,
            identity: CLAIM_IDENTITY,
            subject: EntityIdentity {
                schema: ENTITY_SCHEMA_IDENTITY,
                schema_version: 0,
                id: SUBJECT_ENTITY_IDENTITY,
            },
            relation: RelationIdentity {
                schema: RELATION_SCHEMA_IDENTITY,
                schema_version: 0,
                id: RELATION_IDENTITY,
            },
            object: EntityIdentity {
                schema: ENTITY_SCHEMA_IDENTITY,
                schema_version: 0,
                id: OBJECT_ENTITY_IDENTITY,
            },
            source_identity: super::super::SOURCE_IDENTITY,
            source_revision_identity: super::super::REVISION_IDENTITY,
            source_support: Some(support()),
            disposition: ClaimDisposition::Supported,
            confidence_descriptor: CONFIDENCE_DESCRIPTOR_IDENTITY,
            valid_from_tick: 10,
            valid_until_tick: 20,
            sensitivity: ClaimSensitivity::Public,
        }
    }

    fn snapshot() -> GraphSnapshot {
        GraphSnapshot {
            schema: GRAPH_SCHEMA_IDENTITY,
            identity: GRAPH_SNAPSHOT_IDENTITY,
            provider: GRAPH_PROVIDER_IDENTITY,
            observed_at_tick: 10,
            valid_until_tick: 20,
            coverage_complete: true,
            claims: vec![claim()],
        }
    }

    fn query() -> GraphQuery {
        GraphQuery {
            start: claim().subject,
            relation: claim().relation,
            maximum_depth: 1,
            maximum_breadth: 1,
            maximum_paths: 1,
            maximum_results: 1,
            maximum_retained_bytes: MAXIMUM_GRAPH_RETAINED_BYTES,
            maximum_work: MAXIMUM_GRAPH_WORK,
            maximum_evidence_events: MAXIMUM_GRAPH_EVIDENCE_EVENTS,
            now_tick: 12,
            access_granted: true,
            maximum_sensitivity: ClaimSensitivity::Public,
        }
    }

    #[test]
    fn cited_claim_traverses_with_exact_schema_snapshot_provider_and_support() {
        let results = traverse(&snapshot(), query(), true, false).unwrap();
        assert_eq!(results.snapshot, GRAPH_SNAPSHOT_IDENTITY);
        assert_eq!(results.provider, GRAPH_PROVIDER_IDENTITY);
        assert_eq!(results.paths.len(), 1);
        assert_eq!(results.paths[0].claims[0].source_support, Some(support()));

        let encoded = encode_results(&results).unwrap();
        assert_eq!(decode_results(&encoded).unwrap(), results);
    }

    #[test]
    fn entity_links_never_propagate_source_support() {
        let first = claim();
        let mut second = claim();
        second.identity = [0x90; 32];
        second.subject = first.object;
        second.object.id = [0x91; 32];
        second.source_support = None;
        let mut graph = snapshot();
        graph.claims = vec![first, second];
        let mut request = query();
        request.maximum_depth = 2;
        request.maximum_paths = 2;
        request.maximum_results = 2;
        assert_eq!(
            traverse(&graph, request, true, false),
            Err(GraphReason::UnsupportedClaim)
        );
    }

    #[test]
    fn missing_contradicted_superseded_stale_and_unauthorized_are_distinct() {
        let mut graph = snapshot();
        let mut request = query();
        request.relation.id = [0; 32];
        assert_eq!(
            traverse(&graph, request, true, false),
            Err(GraphReason::MissingFact)
        );

        graph.claims[0].disposition = ClaimDisposition::Contradicted;
        assert_eq!(
            traverse(&graph, query(), true, false),
            Err(GraphReason::Contradicted)
        );
        graph.claims[0].disposition = ClaimDisposition::Superseded;
        assert_eq!(
            traverse(&graph, query(), true, false),
            Err(GraphReason::Superseded)
        );
        graph.claims[0].disposition = ClaimDisposition::Supported;
        graph.valid_until_tick = 11;
        assert_eq!(
            traverse(&graph, query(), true, false),
            Err(GraphReason::StaleSnapshot)
        );
        graph.valid_until_tick = 20;
        let mut denied = query();
        denied.access_granted = false;
        assert_eq!(
            traverse(&graph, denied, true, false),
            Err(GraphReason::Unauthorized)
        );

        let codes = [
            GraphReason::MissingFact.code(),
            GraphReason::Contradicted.code(),
            GraphReason::Superseded.code(),
            GraphReason::StaleSnapshot.code(),
            GraphReason::Unauthorized.code(),
        ];
        for (index, code) in codes.iter().enumerate() {
            assert!(!codes[..index].contains(code));
        }
    }

    #[test]
    fn every_bound_cycle_identity_freshness_and_provider_state_fail_closed() {
        let base = snapshot();
        let mut bad_schema = base.clone();
        bad_schema.schema = [0; 32];
        let mut bad_snapshot = base.clone();
        bad_snapshot.identity = [0; 32];
        let mut bad_provider = base.clone();
        bad_provider.provider = [0; 32];
        let mut partial = base.clone();
        partial.coverage_complete = false;
        for (graph, available, cancelled, expected) in [
            (bad_schema, true, false, GraphReason::SchemaMismatch),
            (bad_snapshot, true, false, GraphReason::SnapshotMismatch),
            (bad_provider, true, false, GraphReason::ProviderMismatch),
            (partial, true, false, GraphReason::PartialSnapshot),
            (base.clone(), false, false, GraphReason::ProviderLost),
            (base, true, true, GraphReason::Cancellation),
        ] {
            assert_eq!(
                traverse(&graph, query(), available, cancelled),
                Err(expected)
            );
        }

        let mut excessive = query();
        excessive.maximum_depth = MAXIMUM_QUERY_DEPTH + 1;
        assert_eq!(
            traverse(&snapshot(), excessive, true, false),
            Err(GraphReason::TraversalBound)
        );

        let mut cyclic = claim();
        cyclic.identity = [0x90; 32];
        cyclic.subject = claim().object;
        cyclic.object = claim().subject;
        let mut graph = snapshot();
        graph.claims.push(cyclic);
        let mut request = query();
        request.maximum_depth = 2;
        request.maximum_paths = 2;
        request.maximum_results = 2;
        assert_eq!(
            traverse(&graph, request, true, false),
            Err(GraphReason::Cycle)
        );
    }

    #[test]
    fn collision_revision_sensitivity_and_each_finite_dimension_fail_closed() {
        let mut collision = snapshot();
        let mut colliding = claim();
        colliding.identity = [0x90; 32];
        colliding.subject.id = claim().object.id;
        colliding.subject.schema = [0; 32];
        collision.claims.push(colliding);
        assert_eq!(
            traverse(&collision, query(), true, false),
            Err(GraphReason::EntityCollision)
        );

        let mut wrong_revision = snapshot();
        wrong_revision.claims[0]
            .source_support
            .as_mut()
            .unwrap()
            .revision = [0; 32];
        assert_eq!(
            traverse(&wrong_revision, query(), true, false),
            Err(GraphReason::CitationRevisionMismatch)
        );

        let mut sensitive = snapshot();
        sensitive.claims[0].sensitivity = ClaimSensitivity::Protected;
        assert_eq!(
            traverse(&sensitive, query(), true, false),
            Err(GraphReason::SensitivityLeak)
        );

        let mut bad_query_schema = query();
        bad_query_schema.start.schema_version = u16::MAX;
        assert_eq!(
            traverse(&snapshot(), bad_query_schema, true, false),
            Err(GraphReason::SchemaMismatch)
        );

        for request in [
            GraphQuery {
                maximum_retained_bytes: 1,
                ..query()
            },
            GraphQuery {
                maximum_evidence_events: 1,
                ..query()
            },
        ] {
            assert_eq!(
                traverse(&snapshot(), request, true, false),
                Err(GraphReason::TraversalBound)
            );
        }

        let mut branched = snapshot();
        let mut second = claim();
        second.identity = [0x90; 32];
        second.object.id = [0x91; 32];
        branched.claims.push(second);
        assert_eq!(
            traverse(&branched, query(), true, false),
            Err(GraphReason::TraversalBound)
        );

        let mut work_limited = query();
        work_limited.maximum_work = 1;
        let mut with_nonmatch = snapshot();
        let mut nonmatch = claim();
        nonmatch.identity = [0x90; 32];
        nonmatch.subject.id = [0x91; 32];
        nonmatch.object.id = [0x92; 32];
        with_nonmatch.claims.insert(0, nonmatch);
        assert_eq!(
            traverse(&with_nonmatch, work_limited, true, false),
            Err(GraphReason::TraversalBound)
        );
    }

    #[test]
    fn graph_values_round_trip_without_losing_claim_support() {
        let claim = claim();
        assert_eq!(decode_claim(&encode_claim(&claim).unwrap()).unwrap(), claim);
        let snapshot = snapshot();
        assert_eq!(
            decode_snapshot(&encode_snapshot(&snapshot).unwrap()).unwrap(),
            snapshot
        );
        let query = query();
        assert_eq!(decode_query(&encode_query(query).unwrap()).unwrap(), query);
    }

    #[test]
    fn conformance_fixture_owns_the_complete_graph_matrix() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/c4/knowledge-graph.json"))
                .unwrap();
        assert_eq!(fixture["schema"], "conduit.knowledge-graph-conformance");
        assert_eq!(fixture["positive"].as_array().unwrap().len(), 6);
        assert_eq!(fixture["negative"].as_array().unwrap().len(), 15);
        for case in fixture["negative"].as_array().unwrap() {
            let reason = match case.as_str().unwrap() {
                "entity-collision" => GraphReason::EntityCollision,
                "schema-mismatch" => GraphReason::SchemaMismatch,
                "citation-to-wrong-revision" => GraphReason::CitationRevisionMismatch,
                "unsupported-claim" => GraphReason::UnsupportedClaim,
                "contradicted-claim" => GraphReason::Contradicted,
                "superseded-claim" => GraphReason::Superseded,
                "missing-fact" => GraphReason::MissingFact,
                "traversal-bound" => GraphReason::TraversalBound,
                "cycle" => GraphReason::Cycle,
                "stale-snapshot" => GraphReason::StaleSnapshot,
                "partial-snapshot" => GraphReason::PartialSnapshot,
                "sensitivity-leak" => GraphReason::SensitivityLeak,
                "unauthorized-query" => GraphReason::Unauthorized,
                "provider-loss" => GraphReason::ProviderLost,
                "cancellation" => GraphReason::Cancellation,
                unknown => panic!("unowned graph conformance case {unknown}"),
            };
            assert!(reason.code().starts_with("CND-KGRAPH-"));
        }
    }
}
