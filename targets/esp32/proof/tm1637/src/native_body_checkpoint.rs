//! Root-private two-bank custody. Bytes must be the complete canonical mutable
//! Body evidence, validated by the lifecycle owner before use. This layer adds
//! crash consistency, not membership, recovery transitions, or archive ACKs.
use sha2::{Digest, Sha256};
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
#[derive(Clone, Copy)]
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
        let mut buffer = [0; 128];
        for (i, output) in evidence[..length].chunks_mut(128).enumerate() {
            self.storage.read(
                u32::from(retained.bank) * BANK + HEADER + i as u32 * 128,
                &mut buffer[..(output.len() + 3) & !3],
            )?;
            output.copy_from_slice(&buffer[..output.len()]);
        }
        self.storage
            .read(u32::from(retained.bank) * BANK + 56, seed)?;
        Ok(Some(length))
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
            storage: &mut self.storage,
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
        emit(&mut |bytes| program.push(bytes))?;
        if program.count != length
            || <[u8; 32]>::from(program.payload.clone().finalize()) != expected
        {
            return Err(Refusal::Corrupt);
        }
        program.flush()?;
        header[120..152].copy_from_slice(&program.whole.finalize());
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
struct Program<'a, S> {
    storage: &'a mut S,
    base: u32,
    block: [u8; 128],
    used: usize,
    written: u32,
    count: usize,
    length: usize,
    payload: Sha256,
    whole: Sha256,
}
impl<S: Storage> Program<'_, S> {
    fn push(&mut self, mut bytes: &[u8]) -> Result<(), Refusal> {
        self.count = self
            .count
            .checked_add(bytes.len())
            .filter(|n| *n <= self.length)
            .ok_or(Refusal::Capacity)?;
        self.payload.update(bytes);
        self.whole.update(bytes);
        while !bytes.is_empty() {
            let n = (self.block.len() - self.used).min(bytes.len());
            self.block[self.used..self.used + n].copy_from_slice(&bytes[..n]);
            self.used += n;
            bytes = &bytes[n..];
            if self.used == self.block.len() {
                self.flush()?;
            }
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Refusal> {
        if self.used == 0 {
            return Ok(());
        }
        let n = (self.used + 3) & !3;
        self.storage
            .program(self.base + HEADER + self.written, &self.block[..n])?;
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
    let mut buffer = [0; 128];
    let mut at = 0;
    while at < length {
        let n = (length - at).min(128) as usize;
        // Physical adapter requires aligned reads; hash only actual payload bytes.
        storage.read(
            u32::from(bank) * BANK + HEADER + at,
            &mut buffer[..(n + 3) & !3],
        )?;
        hash.update(&buffer[..n]);
        at += n as u32;
    }
    Ok(hash.finalize().into())
}
fn inspect<S: Storage>(storage: &mut S, bank: u8) -> Result<Option<Retained>, Refusal> {
    let mut header = [0; 160];
    storage.read(u32::from(bank) * BANK, &mut header)?;
    if &header[152..156] != COMMIT {
        return Ok(None);
    }
    if &header[..8] != MAGIC
        || header[20..24] != [255; 4]
        || header[88..120] != [255; 32]
        || header[156..160] != [255; 4]
        || digest(storage, bank, &header)? != header[120..152]
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
