//! Exact bounded network-control evidence and append-only commit-last custody.
//! This is integrity storage, never attachment, Body, provider or Play authority.
use conduit_kernel::{self as k, KernelEventKind as K, SignQuery};
pub const MAXIMUM_EVENTS: usize = 128;
const HEADER: usize = 256;
const EVENT: usize = 16;
const DATA: usize = HEADER + MAXIMUM_EVENTS * EVENT;
pub const MAXIMUM_BYTES: usize = DATA + 8;
pub const SLOT_BYTES: u32 = 4096;
pub const CAPACITY: u32 = 32768;
const COMMIT: &[u8; 4] = b"nce1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Geometry,
    Media,
    Corrupt,
    Capacity,
    Scope,
    Gap,
    Event,
}
pub trait Storage {
    fn capacity(&self) -> u32;
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal>;
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope<'a> {
    pub host: &'a str,
    pub boot: &'a str,
    pub plan: &'a str,
    pub active_play: &'a str,
    pub provider_generation: u64,
    pub play_sequence: u64,
    pub configuration_revision: u64,
}
fn crc(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
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
        _ => return Err(Refusal::Event),
    })
}
pub fn encode(
    scope: Scope<'_>,
    events: impl Iterator<Item = k::KernelEvent>,
    query: &impl SignQuery,
    out: &mut [u8; MAXIMUM_BYTES],
) -> Result<(), Refusal> {
    if query.retention_gap().is_some() {
        return Err(Refusal::Gap);
    }
    if scope.provider_generation == 0
        || scope.play_sequence == 0
        || scope.configuration_revision == 0
    {
        return Err(Refusal::Scope);
    }
    for (text, maximum) in [
        (scope.host, 32),
        (scope.boot, 40),
        (scope.plan, 64),
        (scope.active_play, 64),
    ] {
        if text.is_empty() || text.len() > maximum {
            return Err(Refusal::Scope);
        }
    }
    out.fill(0);
    out[..4].copy_from_slice(b"NCE1");
    out[4..6].copy_from_slice(&1u16.to_le_bytes());
    out[8..16].copy_from_slice(&scope.provider_generation.to_le_bytes());
    out[16..24].copy_from_slice(&scope.play_sequence.to_le_bytes());
    for (index, (text, start)) in [
        (scope.host, 32),
        (scope.boot, 64),
        (scope.plan, 104),
        (scope.active_play, 168),
    ]
    .into_iter()
    .enumerate()
    {
        out[24 + index] = text.len() as u8;
        out[start..start + text.len()].copy_from_slice(text.as_bytes());
    }
    out[232..240].copy_from_slice(&scope.configuration_revision.to_le_bytes());
    let mut count = 0;
    for event in events {
        if count >= MAXIMUM_EVENTS {
            return Err(Refusal::Capacity);
        }
        if event.sequence != count as u32
            || query.remote_identity(event.sequence).is_some()
            || matches!(
                event.kind,
                K::RemoteValueOffered
                    | K::RemoteValueAccepted
                    | K::RemoteValueDelivered
                    | K::RemoteOutputClosed
                    | K::RemoteInputAdmitted
                    | K::RemoteInputClosed
            )
        {
            return Err(Refusal::Event);
        }
        let row = &mut out[HEADER + count * EVENT..HEADER + (count + 1) * EVENT];
        row[..4].copy_from_slice(&event.sequence.to_le_bytes());
        row[4..6].copy_from_slice(&event.node.0.to_le_bytes());
        if let Some(port) = event.port {
            row[6] = 1;
            row[7..9].copy_from_slice(&port.0.to_le_bytes());
        }
        if let Some(request) = event.request {
            row[9] = 1;
            row[10..14].copy_from_slice(&request.0.to_le_bytes());
        }
        row[14] = tag(event.kind);
        count += 1;
    }
    out[6..8].copy_from_slice(&(count as u16).to_le_bytes());
    let checksum = crc(&out[..DATA]);
    out[DATA..DATA + 4].copy_from_slice(&checksum.to_le_bytes());
    out[DATA + 4..].copy_from_slice(COMMIT);
    Ok(())
}
pub struct Page<'a> {
    bytes: &'a [u8; MAXIMUM_BYTES],
    scope: Scope<'a>,
    count: usize,
}
fn text(bytes: &[u8], length: usize) -> Result<&str, Refusal> {
    if length == 0 || length > bytes.len() || bytes[length..].iter().any(|b| *b != 0) {
        return Err(Refusal::Scope);
    }
    core::str::from_utf8(&bytes[..length]).map_err(|_| Refusal::Scope)
}
impl<'a> Page<'a> {
    pub fn parse(bytes: &'a [u8; MAXIMUM_BYTES]) -> Result<Self, Refusal> {
        if &bytes[..4] != b"NCE1"
            || bytes[4..6] != 1u16.to_le_bytes()
            || &bytes[DATA + 4..] != COMMIT
            || crc(&bytes[..DATA]) != u32::from_le_bytes(bytes[DATA..DATA + 4].try_into().unwrap())
            || bytes[28..32]
                .iter()
                .chain(&bytes[240..256])
                .any(|b| *b != 0)
        {
            return Err(Refusal::Corrupt);
        }
        let count = u16::from_le_bytes(bytes[6..8].try_into().unwrap()) as usize;
        if count > MAXIMUM_EVENTS || bytes[HEADER + count * EVENT..DATA].iter().any(|b| *b != 0) {
            return Err(Refusal::Corrupt);
        }
        let scope = Scope {
            host: text(&bytes[32..64], bytes[24] as usize)?,
            boot: text(&bytes[64..104], bytes[25] as usize)?,
            plan: text(&bytes[104..168], bytes[26] as usize)?,
            active_play: text(&bytes[168..232], bytes[27] as usize)?,
            provider_generation: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            play_sequence: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            configuration_revision: u64::from_le_bytes(bytes[232..240].try_into().unwrap()),
        };
        if scope.provider_generation == 0
            || scope.play_sequence == 0
            || scope.configuration_revision == 0
        {
            return Err(Refusal::Scope);
        }
        let page = Self {
            bytes,
            scope,
            count,
        };
        for index in 0..count {
            page.event(index)?;
        }
        Ok(page)
    }
    pub fn scope(&self) -> Scope<'a> {
        self.scope
    }
    pub fn events(&self) -> impl Iterator<Item = k::KernelEvent> + '_ {
        (0..self.count).map(|index| self.event(index).expect("validated canonical event"))
    }
    fn event(&self, index: usize) -> Result<k::KernelEvent, Refusal> {
        let row = &self.bytes[HEADER + index * EVENT..HEADER + (index + 1) * EVENT];
        let sequence = u32::from_le_bytes(row[..4].try_into().unwrap());
        if sequence != index as u32
            || row[6] > 1
            || row[9] > 1
            || row[15] != 0
            || (row[6] == 0 && row[7..9] != [0, 0])
            || (row[9] == 0 && row[10..14] != [0, 0, 0, 0])
        {
            return Err(Refusal::Event);
        }
        let kind = kind(row[14])?;
        if matches!(
            kind,
            K::RemoteValueOffered
                | K::RemoteValueAccepted
                | K::RemoteValueDelivered
                | K::RemoteOutputClosed
                | K::RemoteInputAdmitted
                | K::RemoteInputClosed
        ) {
            return Err(Refusal::Event);
        }
        Ok(k::KernelEvent {
            sequence,
            node: k::NodeId(u16::from_le_bytes(row[4..6].try_into().unwrap())),
            port: (row[6] == 1)
                .then(|| k::PortId(u16::from_le_bytes(row[7..9].try_into().unwrap()))),
            request: (row[9] == 1)
                .then(|| k::RequestId(u32::from_le_bytes(row[10..14].try_into().unwrap()))),
            kind,
        })
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct Committed {
    slot: u8,
    checksum: u32,
}
impl Committed {
    pub fn slot(&self) -> u8 {
        self.slot
    }
    pub fn checksum(&self) -> u32 {
        self.checksum
    }
}
pub struct Journal<S> {
    storage: S,
    next: u8,
    count: u8,
}
impl<S: Storage> Journal<S> {
    pub fn open(mut storage: S, work: &mut [u8; MAXIMUM_BYTES]) -> Result<Self, (S, Refusal)> {
        if storage.capacity() != CAPACITY {
            return Err((storage, Refusal::Geometry));
        }
        let mut next = 0;
        let mut count = 0;
        for slot in 0..8 {
            if let Err(e) = storage.read(slot * SLOT_BYTES, work) {
                return Err((storage, e));
            }
            if work.iter().all(|b| *b == 255) {
                continue;
            }
            next = slot as u8 + 1;
            if &work[DATA + 4..] == COMMIT {
                if let Err(e) = Page::parse(work) {
                    return Err((storage, e));
                }
                count += 1;
            }
        }
        Ok(Self {
            storage,
            next,
            count,
        })
    }
    /// Unused physical slots, excluding torn writes. This is capacity telemetry,
    /// never permission to overwrite retained evidence.
    pub fn remaining_slots(&self) -> u8 {
        8u8.saturating_sub(self.next)
    }
    pub fn committed_count(&self) -> u8 {
        self.count
    }
    pub fn read_slot(&mut self, slot: u8, work: &mut [u8; MAXIMUM_BYTES]) -> Result<(), Refusal> {
        if slot >= 8 {
            return Err(Refusal::Geometry);
        }
        self.storage.read(u32::from(slot) * SLOT_BYTES, work)?;
        Page::parse(work).map(|_| ())
    }
    pub fn append(&mut self, wire: &[u8; MAXIMUM_BYTES]) -> Result<Committed, Refusal> {
        Page::parse(wire)?;
        // A commit may have reached flash even when its final readback failed.
        // Retry recognizes the exact complete record instead of spending a slot.
        for slot in 0..self.next {
            if matching_committed(&mut self.storage, u32::from(slot) * SLOT_BYTES, wire)? {
                return Ok(Committed {
                    slot,
                    checksum: u32::from_le_bytes(wire[DATA..DATA + 4].try_into().unwrap()),
                });
            }
        }
        if self.next >= 8 {
            return Err(Refusal::Capacity);
        }
        let slot = self.next;
        self.next += 1;
        let at = u32::from(slot) * SLOT_BYTES;
        self.storage.program(at, &wire[..DATA + 4])?;
        verify(&mut self.storage, at, &wire[..DATA + 4])?;
        self.storage.program(at + DATA as u32 + 4, COMMIT)?;
        verify(&mut self.storage, at, wire)?;
        self.count += 1;
        Ok(Committed {
            slot,
            checksum: u32::from_le_bytes(wire[DATA..DATA + 4].try_into().unwrap()),
        })
    }
    pub fn into_storage(self) -> S {
        self.storage
    }
}
fn verify(storage: &mut impl Storage, at: u32, wire: &[u8]) -> Result<(), Refusal> {
    let mut read = [0; 64];
    for (index, chunk) in wire.chunks(64).enumerate() {
        storage.read(at + (index * 64) as u32, &mut read[..chunk.len()])?;
        if &read[..chunk.len()] != chunk {
            return Err(Refusal::Media);
        }
    }
    Ok(())
}

fn matching_committed(
    storage: &mut impl Storage,
    at: u32,
    wire: &[u8; MAXIMUM_BYTES],
) -> Result<bool, Refusal> {
    let mut word = [0; 4];
    storage.read(at + DATA as u32 + 4, &mut word)?;
    if &word != COMMIT {
        return Ok(false);
    }
    let mut read = [0; 64];
    let mut checksum = !0u32;
    let mut matches = true;
    for (index, expected) in wire[..DATA].chunks(64).enumerate() {
        storage.read(at + (index * 64) as u32, &mut read[..expected.len()])?;
        matches &= &read[..expected.len()] == expected;
        for byte in &read[..expected.len()] {
            checksum ^= u32::from(*byte);
            for _ in 0..8 {
                checksum = (checksum >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(checksum & 1));
            }
        }
    }
    storage.read(at + DATA as u32, &mut word)?;
    if !checksum != u32::from_le_bytes(word) {
        return Err(Refusal::Corrupt);
    }
    Ok(matches && word == wire[DATA..DATA + 4])
}
