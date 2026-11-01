//! Cooperative checkpoint rewrite. Caller retains exclusive outer StoreLease
//! throughout. Production Access carries the actual reviewed flash permit.
use super::*;
pub(crate) trait SectorStorage<S: Storage> {
    fn erase_sector(&mut self, storage: &mut S, bank: u8, sector: u8) -> Result<(), Refusal>;
    fn program(&mut self, storage: &mut S, at: u32, bytes: &[u8]) -> Result<(), Refusal>;
    fn read(&mut self, storage: &mut S, at: u32, out: &mut [u8]) -> Result<(), Refusal>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    EraseSector,
    ProgramChunk,
    ReadbackChunk,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Erase(u8),
    Header,
    Payload,
    Digest,
    HeaderRead,
    Readback(u32),
    Commit,
    FinalRead,
    Done,
    Failed,
}
pub(crate) struct IncrementalRewrite {
    previous: SourceIdentity,
    header: PrivateHeader,
    expected: [u8; 32],
    length: usize,
    bank: u8,
    phase: Phase,
    block: PrivateChunk,
    staged: bool,
    at: u32,
    whole: Sha256,
    readback: Sha256,
    retains_seed: bool,
    step: u32,
}
impl IncrementalRewrite {
    /// Authenticates the complete source and measures the complete output before
    /// any effect. A replay consumes at most BANK-HEADER source/output bytes.
    pub(crate) fn prepare<S: Storage>(
        journal: &mut Checkpoint<S>,
        source: &SourceSnapshot,
        binding: &[u8; 32],
        seed: &[u8; 32],
        mut emit: impl FnMut(
            &mut PayloadReader<'_, S>,
            &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
        ) -> Result<(), Refusal>,
    ) -> Result<Self, Refusal> {
        if seed == &[0; 32] || binding != source.binding() {
            return Err(Refusal::Corrupt);
        }
        let generation = source
            .generation()
            .checked_add(1)
            .ok_or(Refusal::Generation)?;
        let (mut reader, old_seed) = journal.snapshot_reader_with_seed(source)?;
        let retains_seed = old_seed.0 == *seed;
        let mut length = 0usize;
        let mut hash = Sha256::new();
        emit(&mut reader, &mut |bytes| {
            length = length
                .checked_add(bytes.len())
                .filter(|n| *n <= (BANK - HEADER) as usize)
                .ok_or(Refusal::Capacity)?;
            hash.update(bytes);
            Ok(())
        })?;
        reader.finish()?;
        if length == 0 {
            return Err(Refusal::Capacity);
        }
        let mut header = PrivateHeader([255; 160]);
        header.0[..8].copy_from_slice(MAGIC);
        header.0[8..16].copy_from_slice(&generation.to_le_bytes());
        header.0[16..20].copy_from_slice(&(length as u32).to_le_bytes());
        header.0[24..56].copy_from_slice(binding);
        header.0[56..88].copy_from_slice(seed);
        let mut whole = Sha256::new();
        whole.update(&header.0[..120]);
        Ok(Self {
            previous: source.identity(),
            header,
            expected: hash.finalize().into(),
            length,
            bank: 1 - source.retained.bank,
            phase: Phase::Erase(0),
            block: PrivateChunk([255; 128]),
            staged: false,
            at: 0,
            whole,
            readback: Sha256::new(),
            retains_seed,
            step: 0,
        })
    }
    pub(crate) fn output_bytes(&self) -> usize {
        self.length
    }
    fn check<S: Storage>(&self, journal: &Checkpoint<S>) -> Result<(), Refusal> {
        if journal.retained != Some(self.previous.retained) || matches!(self.phase, Phase::Failed) {
            Err(Refusal::Generation)
        } else {
            Ok(())
        }
    }
    pub(crate) fn step(&self) -> u32 {
        self.step
    }
    pub(crate) fn needs_chunk(&self) -> bool {
        self.phase == Phase::Payload && !self.staged
    }
    pub(crate) fn is_complete(&self) -> bool {
        self.phase == Phase::Done
    }
    pub(crate) fn operation(&self) -> Option<Operation> {
        match self.phase {
            Phase::Erase(_) => Some(Operation::EraseSector),
            Phase::Header | Phase::Digest | Phase::Commit => Some(Operation::ProgramChunk),
            Phase::Payload if self.staged => Some(Operation::ProgramChunk),
            Phase::HeaderRead | Phase::Readback(_) | Phase::FinalRead => {
                Some(Operation::ReadbackChunk)
            }
            _ => None,
        }
    }
    /// CPU-only full replay. Source authentication and full output digest checks
    /// precede staging the selected <=128-byte physical program operation.
    pub(crate) fn prepare_chunk<S: Storage>(
        &mut self,
        journal: &mut Checkpoint<S>,
        mut emit: impl FnMut(
            &mut PayloadReader<'_, S>,
            &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
        ) -> Result<(), Refusal>,
    ) -> Result<(), Refusal> {
        self.check(journal)?;
        if self.phase != Phase::Payload || self.staged {
            return Err(Refusal::Generation);
        }
        let source = SourceSnapshot {
            retained: self.previous.retained,
            expected: self.previous.expected,
        };
        self.block.0.fill(255);
        let result = (|| {
            let mut reader = journal.snapshot_reader(&source)?;
            let mut count = 0usize;
            let mut hash = Sha256::new();
            let start = self.at as usize;
            let end = (start + 128).min(self.length);
            emit(&mut reader, &mut |bytes| {
                let next = count
                    .checked_add(bytes.len())
                    .filter(|n| *n <= self.length)
                    .ok_or(Refusal::Capacity)?;
                let lo = count.max(start);
                let hi = next.min(end);
                if lo < hi {
                    self.block.0[lo - start..hi - start]
                        .copy_from_slice(&bytes[lo - count..hi - count]);
                }
                hash.update(bytes);
                count = next;
                Ok(())
            })?;
            reader.finish()?;
            if count != self.length || <[u8; 32]>::from(hash.finalize()) != self.expected {
                return Err(Refusal::Corrupt);
            }
            Ok(())
        })();
        if result.is_err() {
            self.phase = Phase::Failed;
            self.block.0.fill(0);
            self.header.0.fill(0);
        } else {
            self.staged = true;
        }
        result
    }
    /// One physical call only. Production access is scoped to actual permit.
    pub(crate) fn advance<S: Storage>(
        &mut self,
        journal: &mut Checkpoint<S>,
        access: &mut impl SectorStorage<S>,
    ) -> Result<bool, Refusal> {
        self.check(journal)?;
        let base = u32::from(self.bank) * BANK;
        let result = (|| {
            match self.phase {
                Phase::Erase(sector) => {
                    access.erase_sector(&mut journal.storage, self.bank, sector)?;
                    self.phase = if sector == 7 {
                        Phase::Header
                    } else {
                        Phase::Erase(sector + 1)
                    };
                }
                Phase::Header => {
                    access.program(&mut journal.storage, base, &self.header.0[..120])?;
                    self.phase = Phase::Payload;
                }
                Phase::Payload if self.staged => {
                    let n = (self.length - self.at as usize).min(128);
                    access.program(
                        &mut journal.storage,
                        base + HEADER + self.at,
                        &self.block.0[..(n + 3) & !3],
                    )?;
                    self.whole.update(&self.block.0[..n]);
                    // Reuse the otherwise idle readback hasher to bind the
                    // programmed payload to the immutable preparation result.
                    self.readback.update(&self.block.0[..n]);
                    self.at += n as u32;
                    self.block.0.fill(0);
                    self.staged = false;
                    if self.at as usize == self.length {
                        if <[u8; 32]>::from(self.readback.clone().finalize()) != self.expected {
                            return Err(Refusal::Corrupt);
                        }
                        self.readback = Sha256::new();
                        self.header.0[120..152].copy_from_slice(&self.whole.clone().finalize());
                        self.phase = Phase::Digest;
                    }
                }
                Phase::Digest => {
                    access.program(&mut journal.storage, base + 120, &self.header.0[120..152])?;
                    self.phase = Phase::HeaderRead;
                }
                Phase::HeaderRead => {
                    let mut out = PrivateHeader([0; 160]);
                    access.read(&mut journal.storage, base, &mut out.0)?;
                    if out.0 != self.header.0 {
                        return Err(Refusal::Corrupt);
                    }
                    self.readback.update(&out.0[..120]);
                    self.phase = Phase::Readback(0);
                }
                Phase::Readback(at) => {
                    let n = (self.length - at as usize).min(128);
                    access.read(
                        &mut journal.storage,
                        base + HEADER + at,
                        &mut self.block.0[..(n + 3) & !3],
                    )?;
                    self.readback.update(&self.block.0[..n]);
                    self.block.0.fill(0);
                    self.phase = if at as usize + n == self.length {
                        if <[u8; 32]>::from(self.readback.clone().finalize())
                            != self.header.0[120..152]
                        {
                            return Err(Refusal::Corrupt);
                        }
                        Phase::Commit
                    } else {
                        Phase::Readback(at + n as u32)
                    };
                }
                Phase::Commit => {
                    access.program(&mut journal.storage, base + 152, COMMIT)?;
                    self.phase = Phase::FinalRead;
                }
                Phase::FinalRead => {
                    let mut out = PrivateHeader([0; 160]);
                    access.read(&mut journal.storage, base, &mut out.0)?;
                    let mut expected = PrivateHeader(self.header.0);
                    expected.0[152..156].copy_from_slice(COMMIT);
                    if out.0 != expected.0 {
                        return Err(Refusal::Corrupt);
                    }
                    self.phase = Phase::Done;
                }
                _ => return Err(Refusal::Generation),
            }
            self.step = self.step.checked_add(1).ok_or(Refusal::Generation)?;
            Ok(self.phase == Phase::Done)
        })();
        if result.is_err() {
            self.phase = Phase::Failed;
            self.block.0.fill(0);
            self.header.0.fill(0);
        }
        result
    }
    pub(crate) fn finish<S: Storage>(
        self,
        journal: &mut Checkpoint<S>,
    ) -> Result<RewriteCompletion, Refusal> {
        self.check(journal)?;
        if self.phase != Phase::Done {
            return Err(Refusal::Generation);
        }
        let retained = Retained {
            bank: self.bank,
            generation: u64::from_le_bytes(self.header.0[8..16].try_into().unwrap()),
            length: self.length as u32,
            binding: self.previous.retained.binding,
        };
        journal.retained = Some(retained);
        Ok(RewriteCompletion {
            previous: self.previous,
            source: SourceSnapshot {
                retained,
                expected: self.header.0[120..152].try_into().unwrap(),
            },
            encoded_digest: self.expected,
            encoded_length: self.length,
            retains_seed: self.retains_seed,
        })
    }
}
#[cfg(test)]
#[path = "native_checkpoint_incremental_tests.rs"]
mod tests;
