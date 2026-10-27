//! Finite transport representation of one complete kernel invocation's evidence.
//! This carries no observation, display policy, membership or effect authority.
use conduit_kernel::{self as k, KernelEventKind as K, SignQuery};
pub mod packed;
pub const MAXIMUM_EVENTS: usize = 64;
pub const HEADER_BYTES: usize = 16;
pub const RECORD_BYTES: usize = 32;
pub const MAXIMUM_BYTES: usize = HEADER_BYTES + MAXIMUM_EVENTS * RECORD_BYTES;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Capacity,
    Gap,
    Geometry,
    Canonical,
    Identity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub event: k::KernelEvent,
    pub remote: Option<k::RemoteLifecycleIdentity>,
}
fn tag(kind: K) -> u8 {
    match kind {
        K::StepFuelGranted => 0,
        K::StepYielded => 1,
        K::StepFuelExceeded => 2,
        K::ValueStored => 3,
        K::ValueRouted => 4,
        K::ValueConsumed => 5,
        K::RemoteValueOffered => 6,
        K::RemoteValueAccepted => 7,
        K::RemoteValueDelivered => 8,
        K::RemoteOutputClosed => 9,
        K::RemoteInputAdmitted => 10,
        K::RemoteInputClosed => 11,
        K::InputClosed => 12,
        K::InputAbnormal => 13,
        K::HostCallRequested => 14,
        K::HostCallCancellationRequested => 15,
        K::HostCallCompleted => 16,
        K::BackCompleted => 17,
        K::BackFailed => 18,
        K::SemanticAbnormal => 19,
        K::SemanticAbnormalRecovered => 20,
        K::CancellationRequested => 21,
        K::RunCancelled => 22,
    }
}
fn kind(tag: u8) -> Result<K, Refusal> {
    Ok(match tag {
        0 => K::StepFuelGranted,
        1 => K::StepYielded,
        2 => K::StepFuelExceeded,
        3 => K::ValueStored,
        4 => K::ValueRouted,
        5 => K::ValueConsumed,
        6 => K::RemoteValueOffered,
        7 => K::RemoteValueAccepted,
        8 => K::RemoteValueDelivered,
        9 => K::RemoteOutputClosed,
        10 => K::RemoteInputAdmitted,
        11 => K::RemoteInputClosed,
        12 => K::InputClosed,
        13 => K::InputAbnormal,
        14 => K::HostCallRequested,
        15 => K::HostCallCancellationRequested,
        16 => K::HostCallCompleted,
        17 => K::BackCompleted,
        18 => K::BackFailed,
        19 => K::SemanticAbnormal,
        20 => K::SemanticAbnormalRecovered,
        21 => K::CancellationRequested,
        22 => K::RunCancelled,
        _ => return Err(Refusal::Canonical),
    })
}
fn remote_kind(kind: K) -> bool {
    matches!(
        kind,
        K::RemoteValueOffered
            | K::RemoteValueAccepted
            | K::RemoteValueDelivered
            | K::RemoteOutputClosed
            | K::RemoteInputAdmitted
            | K::RemoteInputClosed
    )
}
pub fn encode(
    invocation: u64,
    events: impl Iterator<Item = k::KernelEvent>,
    query: &impl SignQuery,
    output: &mut [u8],
) -> Result<usize, Refusal> {
    if query.retention_gap().is_some() {
        return Err(Refusal::Gap);
    }
    if output.len() < MAXIMUM_BYTES {
        return Err(Refusal::Capacity);
    }
    output[..MAXIMUM_BYTES].fill(0);
    output[..4].copy_from_slice(b"CEV1");
    output[4..6].copy_from_slice(&1u16.to_le_bytes());
    output[8..16].copy_from_slice(&invocation.to_le_bytes());
    let mut count = 0;
    for event in events {
        if count == MAXIMUM_EVENTS {
            return Err(Refusal::Capacity);
        }
        if event.sequence != count as u32 {
            return Err(Refusal::Identity);
        }
        let record = &mut output
            [HEADER_BYTES + count * RECORD_BYTES..HEADER_BYTES + (count + 1) * RECORD_BYTES];
        record[..4].copy_from_slice(&event.sequence.to_le_bytes());
        record[4..6].copy_from_slice(&event.node.0.to_le_bytes());
        if let Some(port) = event.port {
            record[6] = 1;
            record[7..9].copy_from_slice(&port.0.to_le_bytes());
        }
        if let Some(request) = event.request {
            record[9] = 1;
            record[10..14].copy_from_slice(&request.0.to_le_bytes());
        }
        record[14] = tag(event.kind);
        let remote = query.remote_identity(event.sequence);
        if remote_kind(event.kind) != remote.is_some() {
            return Err(Refusal::Identity);
        }
        if let Some(remote) = remote {
            if event.port.is_none()
                || event.request.is_some()
                || !direction_matches(event.kind, remote.direction)
            {
                return Err(Refusal::Identity);
            }
            record[15] = 1;
            record[16..18].copy_from_slice(&remote.endpoint.0.to_le_bytes());
            record[18..20].copy_from_slice(&remote.cord.0.to_le_bytes());
            record[20] = match remote.direction {
                k::RemoteCordDirection::Ingress => 0,
                k::RemoteCordDirection::Egress => 1,
            };
            record[21..29].copy_from_slice(&remote.sequence.to_le_bytes());
        }
        count += 1;
    }
    if count == 0 {
        return Err(Refusal::Geometry);
    }
    output[6..8].copy_from_slice(&(count as u16).to_le_bytes());
    Ok(HEADER_BYTES + count * RECORD_BYTES)
}
pub struct Page<'a> {
    bytes: &'a [u8],
}
impl<'a> Page<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Refusal> {
        if bytes.len() < HEADER_BYTES || &bytes[..4] != b"CEV1" || bytes[4..6] != 1u16.to_le_bytes()
        {
            return Err(Refusal::Geometry);
        }
        let count = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        if count == 0
            || count > MAXIMUM_EVENTS
            || bytes.len() != HEADER_BYTES + count * RECORD_BYTES
        {
            return Err(Refusal::Geometry);
        }
        for (index, record) in bytes[HEADER_BYTES..].chunks_exact(RECORD_BYTES).enumerate() {
            let event = decode(record)?;
            if event.event.sequence != index as u32 {
                return Err(Refusal::Identity);
            }
        }
        Ok(Self { bytes })
    }
    pub fn invocation(&self) -> u64 {
        u64::from_le_bytes(self.bytes[8..16].try_into().expect("checked header"))
    }
    pub fn records(&self) -> impl Iterator<Item = Record> + '_ {
        self.bytes[HEADER_BYTES..]
            .chunks_exact(RECORD_BYTES)
            .map(|bytes| decode(bytes).expect("checked records"))
    }
}
fn decode(bytes: &[u8]) -> Result<Record, Refusal> {
    if bytes[6] > 1
        || bytes[9] > 1
        || bytes[15] > 1
        || bytes[29..].iter().any(|b| *b != 0)
        || (bytes[6] == 0 && bytes[7..9] != [0; 2])
        || (bytes[9] == 0 && bytes[10..14] != [0; 4])
        || (bytes[15] == 0 && bytes[16..29] != [0; 13])
    {
        return Err(Refusal::Canonical);
    }
    let kind = kind(bytes[14])?;
    if remote_kind(kind) != (bytes[15] == 1) {
        return Err(Refusal::Identity);
    }
    let remote = if bytes[15] == 1 {
        Some(k::RemoteLifecycleIdentity {
            endpoint: k::RemoteEndpointId(u16::from_le_bytes(bytes[16..18].try_into().unwrap())),
            cord: k::CordId(u16::from_le_bytes(bytes[18..20].try_into().unwrap())),
            direction: match bytes[20] {
                0 => k::RemoteCordDirection::Ingress,
                1 => k::RemoteCordDirection::Egress,
                _ => return Err(Refusal::Canonical),
            },
            sequence: u64::from_le_bytes(bytes[21..29].try_into().unwrap()),
        })
    } else {
        None
    };
    if remote
        .is_some_and(|r| bytes[6] != 1 || bytes[9] != 0 || !direction_matches(kind, r.direction))
    {
        return Err(Refusal::Identity);
    }
    Ok(Record {
        event: k::KernelEvent {
            sequence: u32::from_le_bytes(bytes[..4].try_into().unwrap()),
            node: k::NodeId(u16::from_le_bytes(bytes[4..6].try_into().unwrap())),
            port: (bytes[6] == 1)
                .then(|| k::PortId(u16::from_le_bytes(bytes[7..9].try_into().unwrap()))),
            request: (bytes[9] == 1)
                .then(|| k::RequestId(u32::from_le_bytes(bytes[10..14].try_into().unwrap()))),
            kind,
        },
        remote,
    })
}

fn direction_matches(kind: K, direction: k::RemoteCordDirection) -> bool {
    matches!(
        (kind, direction),
        (
            K::RemoteInputAdmitted | K::RemoteInputClosed,
            k::RemoteCordDirection::Ingress
        ) | (
            K::RemoteValueOffered
                | K::RemoteValueAccepted
                | K::RemoteValueDelivered
                | K::RemoteOutputClosed,
            k::RemoteCordDirection::Egress
        )
    )
}
