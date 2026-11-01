//! Frozen recipe reads authenticated consumed blocks, never a copied-source DTO.
//! Unused old bytes are not reverified on every chunk. Physical writes still
//! require the caller's SectorStorage and the final complete output digest.
use super::*;
extern crate alloc;
use self::alloc::vec::Vec;

/// Affine raw-storage provenance only, not Body, signing or flash authority.
pub(crate) struct SourceBlocks {
    source: SourceIdentity,
    header_hash: [u8; 32],
    start: usize,
    end: usize,
    first: usize,
    hashes: Vec<[u8; 32]>,
}
fn header<S: Storage>(
    journal: &mut Checkpoint<S>,
    source: SourceIdentity,
) -> Result<PrivateHeader, Refusal> {
    if journal.retained != Some(source.retained) {
        return Err(Refusal::Generation);
    }
    let mut h = PrivateHeader([0; 160]);
    journal
        .storage
        .read(u32::from(source.retained.bank) * BANK, &mut h.0)?;
    if &h.0[..8] != MAGIC
        || &h.0[152..156] != COMMIT
        || h.0[20..24] != [255; 4]
        || h.0[88..120] != [255; 32]
        || h.0[156..160] != [255; 4]
        || h.0[56..88] == [0; 32]
        || u64::from_le_bytes(h.0[8..16].try_into().unwrap()) != source.retained.generation
        || u32::from_le_bytes(h.0[16..20].try_into().unwrap()) != source.retained.length
        || h.0[24..56] != source.retained.binding
        || h.0[120..152] != source.expected
    {
        return Err(Refusal::Corrupt);
    }
    Ok(h)
}
impl SourceBlocks {
    /// Reserve a caller-bounded hash index before reading or producing effects;
    /// then fully authenticate this exact header and source in one streaming pass.
    pub(crate) fn authenticate<S: Storage>(
        journal: &mut Checkpoint<S>,
        source: &SourceSnapshot,
        start: usize,
        end: usize,
        max_hash_bytes: usize,
    ) -> Result<Self, Refusal> {
        if start > end || end > source.payload_length() {
            return Err(Refusal::Geometry);
        }
        let first = start / 128;
        let count = if start == end {
            0
        } else {
            (end - 1) / 128 + 1 - first
        };
        let bytes = count.checked_mul(32).ok_or(Refusal::Capacity)?;
        if bytes > max_hash_bytes {
            return Err(Refusal::Capacity);
        }
        let mut hashes = Vec::new();
        hashes
            .try_reserve_exact(count)
            .map_err(|_| Refusal::Capacity)?;
        let h = header(journal, source.identity())?;
        let header_hash = Sha256::digest(&h.0).into();
        let mut whole = Sha256::new();
        whole.update(&h.0[..120]);
        let mut reader = PayloadReader {
            storage: &mut journal.storage,
            bank: source.retained.bank,
            length: source.retained.length,
            at: 0,
            hash: whole,
            expected: source.expected,
        };
        let mut chunk = PrivateChunk([0; 128]);
        let mut index = 0;
        loop {
            let n = reader.read(&mut chunk.0)?;
            if n == 0 {
                break;
            }
            if index >= first && index - first < count {
                hashes.push(Sha256::digest(&chunk.0[..n]).into());
            }
            index += 1;
        }
        reader.finish()?;
        if hashes.len() != count {
            return Err(Refusal::Corrupt);
        }
        let result = Self {
            source: source.identity(),
            header_hash,
            start,
            end,
            first,
            hashes,
        };
        result.check(journal)?;
        Ok(result)
    }
    fn check<S: Storage>(&self, journal: &mut Checkpoint<S>) -> Result<PrivateHeader, Refusal> {
        let h = header(journal, self.source)?;
        if <[u8; 32]>::from(Sha256::digest(&h.0)) != self.header_hash {
            return Err(Refusal::Corrupt);
        }
        Ok(h)
    }
    pub(crate) fn hash_bytes(&self) -> usize {
        self.hashes.len() * 32
    }
    pub(crate) fn retained_hash_bytes(&self) -> usize {
        self.hashes.capacity() * 32
    }

    /// Only bytes in the retained reused range can escape; each intersecting
    /// complete physical source block is checked before any byte is copied.
    pub(crate) fn read_slice<S: Storage>(
        &self,
        journal: &mut Checkpoint<S>,
        at: usize,
        out: &mut [u8],
    ) -> Result<(), Refusal> {
        if out.len() > 128 {
            return Err(Refusal::Geometry);
        }
        self.check(journal)?;
        let end = at.checked_add(out.len()).ok_or(Refusal::Geometry)?;
        if at < self.start || end > self.end {
            return Err(Refusal::Geometry);
        }
        let mut scratch = PrivateChunk([0; 128]);
        let mut copied = 0;
        while copied < out.len() {
            let pos = at + copied;
            let block = pos / 128;
            let offset = block * 128;
            let n = (self.source.retained.length as usize - offset).min(128);
            journal.storage.read(
                u32::from(self.source.retained.bank) * BANK + HEADER + offset as u32,
                &mut scratch.0[..(n + 3) & !3],
            )?;
            let expected = self
                .hashes
                .get(block - self.first)
                .ok_or(Refusal::Geometry)?;
            if &<[u8; 32]>::from(Sha256::digest(&scratch.0[..n])) != expected {
                return Err(Refusal::Corrupt);
            }
            let lo = pos - offset;
            let take = (n - lo).min(out.len() - copied);
            out[copied..copied + take].copy_from_slice(&scratch.0[lo..lo + take]);
            copied += take;
        }
        Ok(())
    }
}
impl IncrementalRewrite {
    /// `expected` must come from the caller's frozen canonical recipe. It is
    /// checked against all actually programmed bytes before commit, not trusted
    /// as authority to alter the source or release an external result.
    pub(crate) fn prepare_frozen<S: Storage>(
        journal: &mut Checkpoint<S>,
        source: &SourceBlocks,
        binding: &[u8; 32],
        seed: &[u8; 32],
        length: usize,
        expected: [u8; 32],
    ) -> Result<Self, Refusal> {
        let old = source.check(journal)?;
        if seed == &[0; 32] || binding != &source.source.retained.binding {
            return Err(Refusal::Corrupt);
        }
        if length == 0 || length > (BANK - HEADER) as usize {
            return Err(Refusal::Capacity);
        }
        let generation = source
            .source
            .retained
            .generation
            .checked_add(1)
            .ok_or(Refusal::Generation)?;
        let mut h = PrivateHeader([255; 160]);
        h.0[..8].copy_from_slice(MAGIC);
        h.0[8..16].copy_from_slice(&generation.to_le_bytes());
        h.0[16..20].copy_from_slice(&(length as u32).to_le_bytes());
        h.0[24..56].copy_from_slice(binding);
        h.0[56..88].copy_from_slice(seed);
        let mut whole = Sha256::new();
        whole.update(&h.0[..120]);
        Ok(Self {
            previous: source.source,
            header: h,
            expected,
            length,
            bank: 1 - source.source.retained.bank,
            phase: Phase::Erase(0),
            block: PrivateChunk([255; 128]),
            staged: false,
            at: 0,
            whole,
            readback: Sha256::new(),
            retains_seed: old.0[56..88] == *seed,
            step: 0,
        })
    }
    /// Assemble only this output chunk from changed pieces and checked source
    /// blocks. No allocation, JSON encoding or complete-source replay occurs.
    pub(crate) fn prepare_frozen_chunk<S: Storage>(
        &mut self,
        journal: &mut Checkpoint<S>,
        source: &SourceBlocks,
        prefix: &[u8],
        append: &[u8],
        suffix: &[u8],
    ) -> Result<(), Refusal> {
        if self.phase != Phase::Payload || self.staged {
            return Err(Refusal::Generation);
        }
        let result = (|| {
            self.check(journal)?;
            source.check(journal)?;
            if source.source != self.previous {
                return Err(Refusal::Generation);
            }
            let reused = source.end - source.start;
            let length = prefix
                .len()
                .checked_add(reused)
                .and_then(|n| n.checked_add(append.len()))
                .and_then(|n| n.checked_add(suffix.len()))
                .ok_or(Refusal::Capacity)?;
            if length != self.length {
                return Err(Refusal::Corrupt);
            }
            self.block.0.fill(255);
            let start = self.at as usize;
            let end = (start + 128).min(length);
            let mut offset = 0;
            for (part, bytes) in [Some(prefix), None, Some(append), Some(suffix)]
                .into_iter()
                .enumerate()
            {
                let n = bytes.map_or(reused, |b| b.len());
                let lo = start.max(offset);
                let hi = end.min(offset + n);
                if lo < hi {
                    if part == 1 {
                        source.read_slice(
                            journal,
                            source.start + lo - offset,
                            &mut self.block.0[lo - start..hi - start],
                        )?;
                    } else {
                        self.block.0[lo - start..hi - start]
                            .copy_from_slice(&bytes.unwrap()[lo - offset..hi - offset]);
                    }
                }
                offset += n;
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
}
#[cfg(test)]
#[path = "native_checkpoint_frozen_tests.rs"]
mod tests;
