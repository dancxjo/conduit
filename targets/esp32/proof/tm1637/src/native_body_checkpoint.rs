//! Root-private two-bank custody. Bytes must be the complete canonical mutable
//! Body evidence, validated by the lifecycle owner before use. This layer adds
//! crash consistency, not membership, recovery transitions, or archive ACKs.
#[path = "native_checkpoint_timing.rs"]
pub(crate) mod timing;
use core::cell::RefCell;
use sha2::{Digest, Sha256};
use timing::Stage;
const BANK: u32 = 0x8000;
const HEADER: u32 = 160;
const MAGIC: &[u8; 8] = b"BDYCST01";
const COMMIT: &[u8; 4] = b"done";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Storage,
    Geometry,
    Corrupt,
    Capacity,
    Generation,
}
pub trait Storage {
    fn capacity(&self) -> u32;
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal>;
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal>;
    /// Erase the leading sector (containing the commit marker) first, then
    /// remaining sectors in ascending order. A failed erase must not affect
    /// the other bank. This makes interrupted rotation uncommitted immediately.
    fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal>;
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Retained {
    bank: u8,
    generation: u64,
    length: u32,
    binding: [u8; 32],
}
pub struct Checkpoint<S> {
    storage: S,
    retained: Option<Retained>,
}
/// Affine custody of one fully consumed authenticated committed snapshot.
/// This is raw storage provenance, never Body membership or archive validation.
pub(crate) struct SourceSnapshot {
    retained: Retained,
    expected: [u8; 32],
}
/// Equality-only physical source identity; it grants no storage or Body authority.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceIdentity {
    retained: Retained,
    expected: [u8; 32],
}
impl SourceSnapshot {
    pub(crate) fn identity(&self) -> SourceIdentity {
        SourceIdentity {
            retained: self.retained,
            expected: self.expected,
        }
    }

    pub(crate) fn succeeds(&self, previous: SourceIdentity) -> bool {
        previous.retained.generation.checked_add(1) == Some(self.retained.generation)
            && self.retained.bank == 1 - previous.retained.bank
            && self.retained.binding == previous.retained.binding
    }
    pub(crate) fn generation(&self) -> u64 {
        self.retained.generation
    }
    pub(crate) fn payload_length(&self) -> usize {
        self.retained.length as usize
    }
    pub(crate) fn binding(&self) -> &[u8; 32] {
        &self.retained.binding
    }
    /// Digest of the complete authenticated header and encoded source. This
    /// identifies custody; it supplies neither signing authority nor membership.
    pub(crate) fn digest(&self) -> &[u8; 32] {
        &self.expected
    }
}
/// Completion of the actual two-pass writer and full physical readback.
/// Private construction binds its encoded transcript to this committed source.
pub(crate) struct RewriteCompletion {
    previous: SourceIdentity,
    source: SourceSnapshot,
    encoded_digest: [u8; 32],
    encoded_length: usize,
    retains_seed: bool,
}
impl RewriteCompletion {
    pub(crate) fn source(&self) -> &SourceSnapshot {
        &self.source
    }
    pub(crate) fn encoded_matches(&self, length: usize, digest: &[u8; 32]) -> bool {
        self.encoded_length == length && &self.encoded_digest == digest
    }
    pub(crate) fn continues_source(&self, previous: SourceIdentity) -> bool {
        self.previous == previous && self.source.succeeds(previous)
    }
    pub(crate) fn retains_seed(&self) -> bool {
        self.retains_seed
    }
    pub(crate) fn into_source(self) -> SourceSnapshot {
        self.source
    }
}
/// One serialized storage owner interleaves reads of the old committed bank
/// and writes to the other bank. No aliased mutable storage reference escapes.
pub(crate) struct RewriteIo<'a, S> {
    state: RefCell<RewriteState<'a, S>>,
}
struct RewriteState<'a, S> {
    source: PayloadReader<'a, S>,
    program: Program,
    writing: bool,
}
impl<S: Storage> RewriteIo<'_, S> {
    pub(crate) fn read_source(&self, out: &mut [u8; 128]) -> Result<usize, Refusal> {
        self.state
            .try_borrow_mut()
            .map_err(|_| Refusal::Storage)?
            .source
            .read(out)
    }
    pub(crate) fn write(&self, bytes: &[u8]) -> Result<(), Refusal> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Refusal::Storage)?;
        let RewriteState {
            source,
            program,
            writing,
        } = &mut *state;
        if *writing {
            program.push(source.storage, bytes)
        } else {
            program.accept(bytes)
        }
    }
}
/// Exclusive bounded view of one already authenticated committed payload.
/// A second streaming digest detects a changed read before decoded data escapes.
pub(crate) struct PayloadReader<'a, S> {
    storage: &'a mut S,
    bank: u8,
    length: u32,
    at: u32,
    hash: Sha256,
    expected: [u8; 32],
}
impl<S: Storage> PayloadReader<'_, S> {
    pub(crate) fn read(&mut self, out: &mut [u8; 128]) -> Result<usize, Refusal> {
        let n = (self.length - self.at).min(128) as usize;
        if n == 0 {
            return Ok(0);
        }
        self.storage.read(
            u32::from(self.bank) * BANK + HEADER + self.at,
            &mut out[..(n + 3) & !3],
        )?;
        self.hash.update(&out[..n]);
        self.at += n as u32;
        Ok(n)
    }
    fn finish(self) -> Result<(), Refusal> {
        if self.at != self.length || <[u8; 32]>::from(self.hash.finalize()) != self.expected {
            return Err(Refusal::Corrupt);
        }
        Ok(())
    }
}
struct PrivateSeed([u8; 32]);
impl Drop for PrivateSeed {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
struct PrivateChunk([u8; 128]);
impl Drop for PrivateChunk {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
struct PrivateHeader([u8; 160]);
impl Drop for PrivateHeader {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
impl<S: Storage> Checkpoint<S> {
    pub fn open(mut storage: S) -> Result<Self, Refusal> {
        if storage.capacity() != 2 * BANK {
            return Err(Refusal::Geometry);
        }
        let first = inspect(&mut storage, 0)?;
        let second = inspect(&mut storage, 1)?;
        let retained = match (first, second) {
            (None, other) | (other, None) => other,
            (Some(a), Some(b)) if a.generation == b.generation => return Err(Refusal::Generation),
            (Some(a), Some(b)) => Some(if a.generation > b.generation { a } else { b }),
        };
        if retained.is_none() {
            let mut bytes = [0; 128];
            for at in (0..2 * BANK).step_by(bytes.len()) {
                storage.read(at, &mut bytes)?;
                if bytes.iter().any(|byte| *byte != 255) {
                    return Err(Refusal::Corrupt);
                }
            }
        }
        Ok(Self { storage, retained })
    }
    pub fn payload_length(&self, binding: &[u8; 32]) -> Result<Option<usize>, Refusal> {
        match self.retained {
            Some(retained) if &retained.binding == binding => Ok(Some(retained.length as usize)),
            Some(_) => Err(Refusal::Corrupt),
            None => Ok(None),
        }
    }
    pub fn generation(&self) -> Option<u64> {
        self.retained.map(|r| r.generation)
    }
    /// Caller provides independently expected Body binding. The seed is private
    /// output; never log or publish it. No key is returned from torn media.
    pub fn load(
        &mut self,
        binding: &[u8; 32],
        seed: &mut [u8; 32],
        evidence: &mut [u8],
    ) -> Result<Option<usize>, Refusal> {
        let Some(retained) = self.retained else {
            return Ok(None);
        };
        if &retained.binding != binding {
            return Err(Refusal::Corrupt);
        }
        let length = retained.length as usize;
        if evidence.len() < length {
            return Err(Refusal::Capacity);
        }
        // Revalidate before exposing any secret, even if storage changed after open.
        let valid = inspect(&mut self.storage, retained.bank)?.ok_or(Refusal::Corrupt)?;
        if valid.generation != retained.generation
            || valid.binding != retained.binding
            || valid.length != retained.length
        {
            return Err(Refusal::Corrupt);
        }
        let mut buffer = PrivateChunk([0; 128]);
        for (i, output) in evidence[..length].chunks_mut(128).enumerate() {
            self.storage.read(
                u32::from(retained.bank) * BANK + HEADER + i as u32 * 128,
                &mut buffer.0[..(output.len() + 3) & !3],
            )?;
            output.copy_from_slice(&buffer.0[..output.len()]);
        }
        self.storage
            .read(u32::from(retained.bank) * BANK + 56, seed)?;
        Ok(Some(length))
    }
    /// Authenticate the complete committed bank before decoding. No whole raw
    /// payload allocation is needed, no secret is returned on any failure, and
    /// a second digest plus complete reader consumption fences changing media.
    pub(crate) fn load_streamed<T>(
        &mut self,
        binding: &[u8; 32],
        seed: &mut [u8; 32],
        decode: impl FnOnce(&mut PayloadReader<'_, S>) -> Result<T, Refusal>,
    ) -> Result<Option<T>, Refusal> {
        self.load_streamed_with_snapshot(binding, seed, decode)
            .map(|loaded| loaded.map(|(value, _)| value))
    }
    /// Mint storage provenance only after full preauthentication, the caller's
    /// typed validation and the second-read digest plus exact EOF have passed.
    pub(crate) fn load_streamed_with_snapshot<T>(
        &mut self,
        binding: &[u8; 32],
        seed: &mut [u8; 32],
        decode: impl FnOnce(&mut PayloadReader<'_, S>) -> Result<T, Refusal>,
    ) -> Result<Option<(T, SourceSnapshot)>, Refusal> {
        seed.fill(0);
        let Some(retained) = self.retained else {
            return Ok(None);
        };
        if &retained.binding != binding {
            return Err(Refusal::Corrupt);
        }
        let mut header = PrivateHeader([0; 160]);
        self.storage
            .read(u32::from(retained.bank) * BANK, &mut header.0)?;
        let valid =
            inspect_header(&mut self.storage, retained.bank, &header.0)?.ok_or(Refusal::Corrupt)?;
        if valid.generation != retained.generation
            || valid.binding != retained.binding
            || valid.length != retained.length
        {
            return Err(Refusal::Corrupt);
        }
        let mut hash = Sha256::new();
        hash.update(&header.0[..120]);
        let mut reader = PayloadReader {
            storage: &mut self.storage,
            bank: retained.bank,
            length: retained.length,
            at: 0,
            hash,
            expected: header.0[120..152].try_into().unwrap(),
        };
        let decoded = decode(&mut reader)?;
        reader.finish()?;
        seed.copy_from_slice(&header.0[56..88]);
        Ok(Some((
            decoded,
            SourceSnapshot {
                retained,
                expected: header.0[120..152].try_into().unwrap(),
            },
        )))
    }
    fn snapshot_reader<'a>(
        &'a mut self,
        snapshot: &SourceSnapshot,
    ) -> Result<PayloadReader<'a, S>, Refusal> {
        self.snapshot_reader_with_seed(snapshot)
            .map(|(reader, _seed)| reader)
    }
    fn snapshot_reader_with_seed<'a>(
        &'a mut self,
        snapshot: &SourceSnapshot,
    ) -> Result<(PayloadReader<'a, S>, PrivateSeed), Refusal> {
        if self.retained != Some(snapshot.retained) {
            return Err(Refusal::Generation);
        }
        let mut header = PrivateHeader([0; 160]);
        self.storage
            .read(u32::from(snapshot.retained.bank) * BANK, &mut header.0)?;
        let valid = inspect_header(&mut self.storage, snapshot.retained.bank, &header.0)?
            .ok_or(Refusal::Corrupt)?;
        if valid != snapshot.retained || header.0[120..152] != snapshot.expected {
            return Err(Refusal::Corrupt);
        }
        let mut hash = Sha256::new();
        hash.update(&header.0[..120]);
        Ok((
            PayloadReader {
                storage: &mut self.storage,
                bank: valid.bank,
                length: valid.length,
                at: 0,
                hash,
                expected: snapshot.expected,
            },
            PrivateSeed(header.0[56..88].try_into().unwrap()),
        ))
    }
    /// Reauthenticate this exact still-current source on every use. A stale
    /// witness cannot read an overwritten bank or select another generation.
    pub(crate) fn read_snapshot<T>(
        &mut self,
        snapshot: &SourceSnapshot,
        decode: impl FnOnce(&mut PayloadReader<'_, S>) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        let mut reader = self.snapshot_reader(snapshot)?;
        let value = decode(&mut reader)?;
        reader.finish()?;
        Ok(value)
    }
    /// Copy forward archive bytes while replacing active evidence, using one
    /// exclusive physical storage owner. BOTH passes consume and authenticate
    /// the entire old source. No target erase occurs until preflight fits.
    pub(crate) fn rewrite_streamed(
        &mut self,
        snapshot: &SourceSnapshot,
        binding: &[u8; 32],
        seed: &[u8; 32],
        mut emit: impl FnMut(&RewriteIo<'_, S>) -> Result<(), Refusal>,
    ) -> Result<SourceSnapshot, Refusal> {
        self.rewrite_streamed_outlined(snapshot, binding, seed, &mut emit, &mut |_| {})
            .map(RewriteCompletion::into_source)
    }
    pub(crate) fn rewrite_streamed_attested(
        &mut self,
        snapshot: &SourceSnapshot,
        binding: &[u8; 32],
        seed: &[u8; 32],
        mut emit: impl FnMut(&RewriteIo<'_, S>) -> Result<(), Refusal>,
    ) -> Result<RewriteCompletion, Refusal> {
        self.rewrite_streamed_outlined(snapshot, binding, seed, &mut emit, &mut |_| {})
    }
    pub(crate) fn rewrite_streamed_observed(
        &mut self,
        snapshot: &SourceSnapshot,
        binding: &[u8; 32],
        seed: &[u8; 32],
        mut emit: impl FnMut(&RewriteIo<'_, S>) -> Result<(), Refusal>,
        observe: &mut dyn FnMut(Stage),
    ) -> Result<RewriteCompletion, Refusal> {
        self.rewrite_streamed_outlined(snapshot, binding, seed, &mut emit, observe)
    }
    // Keep one physical transaction implementation for ordinary checkpoints and
    // cold biography migration; callback identity cannot change its guarantees.
    #[inline(never)]
    fn rewrite_streamed_outlined(
        &mut self,
        snapshot: &SourceSnapshot,
        binding: &[u8; 32],
        seed: &[u8; 32],
        emit: &mut dyn FnMut(&RewriteIo<'_, S>) -> Result<(), Refusal>,
        observe: &mut dyn FnMut(Stage),
    ) -> Result<RewriteCompletion, Refusal> {
        if *seed == [0; 32] || binding != snapshot.binding() {
            return Err(Refusal::Corrupt);
        }
        observe(Stage::SourceOne);
        let (length, expected, retains_seed) = {
            let (source, old_seed) = self.snapshot_reader_with_seed(snapshot)?;
            let retains_seed = old_seed.0 == *seed;
            drop(old_seed);
            let io = RewriteIo {
                state: RefCell::new(RewriteState {
                    source,
                    program: Program {
                        base: 0,
                        block: [255; 128],
                        used: 0,
                        written: 0,
                        count: 0,
                        length: (BANK - HEADER) as usize,
                        payload: Sha256::new(),
                        whole: Sha256::new(),
                    },
                    writing: false,
                }),
            };
            observe(Stage::Measure);
            emit(&io)?;
            observe(Stage::FinishOne);
            let state = io.state.into_inner();
            state.source.finish()?;
            (
                state.program.count,
                <[u8; 32]>::from(state.program.payload.clone().finalize()),
                retains_seed,
            )
        };
        if length == 0 {
            return Err(Refusal::Capacity);
        }
        let generation = snapshot
            .generation()
            .checked_add(1)
            .ok_or(Refusal::Generation)?;
        let bank = 1 - snapshot.retained.bank;
        let base = u32::from(bank) * BANK;
        let mut header = PrivateHeader([255; 160]);
        header.0[..8].copy_from_slice(MAGIC);
        header.0[8..16].copy_from_slice(&generation.to_le_bytes());
        header.0[16..20].copy_from_slice(&(length as u32).to_le_bytes());
        header.0[24..56].copy_from_slice(binding);
        header.0[56..88].copy_from_slice(seed);
        observe(Stage::Erase);
        self.storage.erase_bank(bank)?;
        observe(Stage::Header);
        self.storage.program(base, &header.0[..120])?;
        observe(Stage::SourceTwo);
        let mut program = {
            let source = self.snapshot_reader(snapshot)?;
            let mut program = Program {
                base,
                block: [255; 128],
                used: 0,
                written: 0,
                count: 0,
                length,
                payload: Sha256::new(),
                whole: Sha256::new(),
            };
            program.whole.update(&header.0[..120]);
            let io = RewriteIo {
                state: RefCell::new(RewriteState {
                    source,
                    program,
                    writing: true,
                }),
            };
            observe(Stage::Write);
            emit(&io)?;
            observe(Stage::FinishTwo);
            let state = io.state.into_inner();
            state.source.finish()?;
            state.program
        };
        if program.count != length
            || <[u8; 32]>::from(program.payload.clone().finalize()) != expected
        {
            return Err(Refusal::Corrupt);
        }
        observe(Stage::Flush);
        program.flush(&mut self.storage)?;
        header.0[120..152].copy_from_slice(&program.whole.clone().finalize());
        self.storage.program(base + 120, &header.0[120..152])?;
        observe(Stage::Readback);
        let mut readback = PrivateHeader([0; 160]);
        self.storage.read(base, &mut readback.0)?;
        if readback.0 != header.0
            || digest(&mut self.storage, bank, &readback.0)? != header.0[120..152]
        {
            return Err(Refusal::Corrupt);
        }
        observe(Stage::Commit);
        self.storage.program(base + 152, COMMIT)?;
        observe(Stage::Inspect);
        let retained = inspect(&mut self.storage, bank)?.ok_or(Refusal::Corrupt)?;
        self.retained = Some(retained);
        Ok(RewriteCompletion {
            previous: snapshot.identity(),
            source: SourceSnapshot {
                retained,
                expected: header.0[120..152].try_into().unwrap(),
            },
            encoded_digest: expected,
            encoded_length: length,
            retains_seed,
        })
    }
    /// The prior committed bank remains intact until a complete successor has
    /// been programmed, hashed from readback, and committed last. No truncation.
    pub fn save(
        &mut self,
        binding: &[u8; 32],
        seed: &[u8; 32],
        evidence: &[u8],
    ) -> Result<(), Refusal> {
        self.save_streamed(binding, seed, |sink| sink(evidence))
    }
    /// Two deterministic passes over immutable complete evidence: preflight
    /// extent/digest before touching flash; then program through 128 bytes.
    /// A differing second pass or any failed write cannot publish a commit.
    pub fn save_streamed(
        &mut self,
        binding: &[u8; 32],
        seed: &[u8; 32],
        mut emit: impl FnMut(&mut dyn FnMut(&[u8]) -> Result<(), Refusal>) -> Result<(), Refusal>,
    ) -> Result<(), Refusal> {
        if *seed == [0; 32] || self.retained.is_some_and(|r| &r.binding != binding) {
            return Err(Refusal::Corrupt);
        }
        let mut length = 0usize;
        let mut expected = Sha256::new();
        emit(&mut |bytes| {
            length = length
                .checked_add(bytes.len())
                .filter(|n| *n <= (BANK - HEADER) as usize)
                .ok_or(Refusal::Capacity)?;
            expected.update(bytes);
            Ok(())
        })?;
        if length == 0 {
            return Err(Refusal::Capacity);
        }
        let expected: [u8; 32] = expected.finalize().into();
        let generation = match self.retained {
            Some(r) => r.generation.checked_add(1).ok_or(Refusal::Generation)?,
            None => 1,
        };
        let bank = self.retained.map_or(0, |r| 1 - r.bank);
        let mut header = [255; HEADER as usize];
        header[..8].copy_from_slice(MAGIC);
        header[8..16].copy_from_slice(&generation.to_le_bytes());
        header[16..20].copy_from_slice(&(length as u32).to_le_bytes());
        header[24..56].copy_from_slice(binding);
        header[56..88].copy_from_slice(seed);
        let base = u32::from(bank) * BANK;
        self.storage.erase_bank(bank)?;
        self.storage.program(base, &header[..120])?;
        let mut program = Program {
            base,
            block: [255; 128],
            used: 0,
            written: 0,
            count: 0,
            length,
            payload: Sha256::new(),
            whole: Sha256::new(),
        };
        program.whole.update(&header[..120]);
        emit(&mut |bytes| program.push(&mut self.storage, bytes))?;
        if program.count != length
            || <[u8; 32]>::from(program.payload.clone().finalize()) != expected
        {
            return Err(Refusal::Corrupt);
        }
        program.flush(&mut self.storage)?;
        header[120..152].copy_from_slice(&program.whole.clone().finalize());
        self.storage.program(base + 120, &header[120..152])?;
        let mut readback = [0; 160];
        self.storage.read(base, &mut readback)?;
        if readback != header || digest(&mut self.storage, bank, &readback)? != header[120..152] {
            return Err(Refusal::Corrupt);
        }
        self.storage.program(base + 152, COMMIT)?;
        self.retained = Some(inspect(&mut self.storage, bank)?.ok_or(Refusal::Corrupt)?);
        header[56..88].fill(0);
        Ok(())
    }
}
struct Program {
    base: u32,
    block: [u8; 128],
    used: usize,
    written: u32,
    count: usize,
    length: usize,
    payload: Sha256,
    whole: Sha256,
}
impl Drop for Program {
    fn drop(&mut self) {
        for byte in &mut self.block {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
    }
}
impl Program {
    fn accept(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        self.count = self
            .count
            .checked_add(bytes.len())
            .filter(|n| *n <= self.length)
            .ok_or(Refusal::Capacity)?;
        self.payload.update(bytes);
        self.whole.update(bytes);
        Ok(())
    }
    fn push<S: Storage>(&mut self, storage: &mut S, mut bytes: &[u8]) -> Result<(), Refusal> {
        self.accept(bytes)?;
        while !bytes.is_empty() {
            let n = (self.block.len() - self.used).min(bytes.len());
            self.block[self.used..self.used + n].copy_from_slice(&bytes[..n]);
            self.used += n;
            bytes = &bytes[n..];
            if self.used == self.block.len() {
                self.flush(storage)?;
            }
        }
        Ok(())
    }
    fn flush<S: Storage>(&mut self, storage: &mut S) -> Result<(), Refusal> {
        if self.used == 0 {
            return Ok(());
        }
        let n = (self.used + 3) & !3;
        storage.program(self.base + HEADER + self.written, &self.block[..n])?;
        self.written += n as u32;
        self.used = 0;
        self.block.fill(255);
        Ok(())
    }
}

fn digest<S: Storage>(storage: &mut S, bank: u8, header: &[u8; 160]) -> Result<[u8; 32], Refusal> {
    let length = u32::from_le_bytes(header[16..20].try_into().unwrap());
    if length == 0 || length > BANK - HEADER {
        return Err(Refusal::Capacity);
    }
    let mut hash = Sha256::new();
    hash.update(&header[..120]);
    let mut buffer = PrivateChunk([0; 128]);
    let mut at = 0;
    while at < length {
        let n = (length - at).min(128) as usize;
        // Physical adapter requires aligned reads; hash only actual payload bytes.
        storage.read(
            u32::from(bank) * BANK + HEADER + at,
            &mut buffer.0[..(n + 3) & !3],
        )?;
        hash.update(&buffer.0[..n]);
        at += n as u32;
    }
    Ok(hash.finalize().into())
}
fn inspect<S: Storage>(storage: &mut S, bank: u8) -> Result<Option<Retained>, Refusal> {
    let mut header = PrivateHeader([0; 160]);
    storage.read(u32::from(bank) * BANK, &mut header.0)?;
    inspect_header(storage, bank, &header.0)
}
fn inspect_header<S: Storage>(
    storage: &mut S,
    bank: u8,
    header: &[u8; 160],
) -> Result<Option<Retained>, Refusal> {
    if &header[152..156] != COMMIT {
        return Ok(None);
    }
    if &header[..8] != MAGIC
        || header[20..24] != [255; 4]
        || header[88..120] != [255; 32]
        || header[156..160] != [255; 4]
        || digest(storage, bank, header)? != header[120..152]
    {
        return Err(Refusal::Corrupt);
    }
    let generation = u64::from_le_bytes(header[8..16].try_into().unwrap());
    if generation == 0 || header[56..88] == [0; 32] {
        return Err(Refusal::Corrupt);
    }
    Ok(Some(Retained {
        bank,
        generation,
        length: u32::from_le_bytes(header[16..20].try_into().unwrap()),
        binding: header[24..56].try_into().unwrap(),
    }))
}

#[cfg(target_os = "none")]
impl Storage for crate::native_evidence_region::BodyRegion {
    fn capacity(&self) -> u32 {
        Self::capacity(self)
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        Self::read(self, at, out).map_err(|_| Refusal::Storage)
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        Self::program(self, at, bytes).map_err(|_| Refusal::Storage)
    }
    fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
        Self::erase_bank(self, bank).map_err(|_| Refusal::Storage)
    }
}
