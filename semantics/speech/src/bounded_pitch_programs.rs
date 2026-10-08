use alloc::{rc::Rc, vec::Vec};
use conduit_plot::PortableExpressionProgram;
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub retained: usize,
    pub preparation: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct Storage {
    pub retained_bound: usize,
    pub preparation_bound: usize,
    pub actual_retained: usize,
}
#[derive(Debug)]
pub enum Refusal {
    Capacity,
    Program,
    Evaluation(conduit_plot::PortableExpressionEvaluationRefusal),
}
struct Entry {
    original: &'static str,
    parsed: PortableExpressionProgram,
}
pub struct PitchPrograms {
    entries: [Entry; 3],
    storage: Storage,
}
impl PitchPrograms {
    pub fn requirements(programs: [(&'static str, &'static [u8]); 3]) -> Result<Storage, Refusal> {
        // Static canonical bytes are generator output. Verify their exact hex identity
        // without allocating before computing all structural decode quotas.
        let root = core::mem::size_of::<Self>()
            .checked_add(2 * core::mem::size_of::<usize>())
            .ok_or(Refusal::Capacity)?;
        let mut total = root;
        for (hex, bytes) in programs {
            if hex.len() != bytes.len().checked_mul(2).ok_or(Refusal::Capacity)? {
                return Err(Refusal::Program);
            }
            for (pair, byte) in hex.as_bytes().as_chunks::<2>().0.iter().zip(bytes) {
                fn nibble(v: u8) -> Option<u8> {
                    match v {
                        b'0'..=b'9' => Some(v - b'0'),
                        b'a'..=b'f' => Some(v - b'a' + 10),
                        _ => None,
                    }
                }
                if nibble(pair[0])
                    .zip(nibble(pair[1]))
                    .map(|(a, b)| (a << 4) | b)
                    != Some(*byte)
                {
                    return Err(Refusal::Program);
                }
            }
            total = total
                .checked_add(
                    PortableExpressionProgram::canonical_decode_storage_bound(bytes)
                        .map_err(|_| Refusal::Program)?,
                )
                .ok_or(Refusal::Capacity)?;
        }
        // Sum of all decode allocation bounds conservatively covers retained prior
        // owners plus the next sequential decoder and its canonical re-encoding.
        Ok(Storage {
            retained_bound: total,
            preparation_bound: total,
            actual_retained: 0,
        })
    }
    pub fn prepare(
        programs: [(&'static str, &'static [u8]); 3],
        limits: Limits,
    ) -> Result<Rc<Self>, Refusal> {
        let mut storage = Self::requirements(programs)?;
        if storage.retained_bound > limits.retained
            || storage.preparation_bound > limits.preparation
        {
            return Err(Refusal::Capacity);
        }
        let mut parse = |index: usize| -> Result<Entry, Refusal> {
            let (original, bytes) = programs[index];
            let bound = PortableExpressionProgram::canonical_decode_storage_bound(bytes)
                .map_err(|_| Refusal::Program)?;
            let parsed =
                PortableExpressionProgram::from_canonical_bytes_with_storage_limit(bytes, bound)
                    .map_err(|_| Refusal::Program)?;
            storage.actual_retained = storage
                .actual_retained
                .checked_add(parsed.owned_heap_bytes())
                .ok_or(Refusal::Capacity)?;
            Ok(Entry { original, parsed })
        };
        let entries = [parse(0)?, parse(1)?, parse(2)?];
        storage.actual_retained = storage
            .actual_retained
            .checked_add(core::mem::size_of::<Self>() + 2 * core::mem::size_of::<usize>())
            .ok_or(Refusal::Capacity)?;
        if storage.actual_retained > storage.retained_bound {
            return Err(Refusal::Capacity);
        }
        Ok(Rc::new(Self { entries, storage }))
    }
    pub fn matches_programs(&self, originals: [&str; 3]) -> bool {
        self.entries
            .iter()
            .zip(originals)
            .all(|(entry, original)| entry.original == original)
    }
    pub fn storage(&self) -> Storage {
        self.storage
    }
    pub fn execute(&self, index: usize, input: &[u8]) -> Result<(&'static str, Vec<u8>), Refusal> {
        let entry = self.entries.get(index).ok_or(Refusal::Program)?;
        Ok((
            entry.original,
            entry.parsed.evaluate(input).map_err(Refusal::Evaluation)?,
        ))
    }
}

pub const PROGRAMS: [(&str, &[u8]); 3] = [
    (
        crate::greeting_programs::PITCH_GRID,
        crate::greeting_programs::PITCH_GRID_BYTES,
    ),
    (
        crate::greeting_programs::PITCH_FRACTION,
        crate::greeting_programs::PITCH_FRACTION_BYTES,
    ),
    (
        crate::greeting_programs::PITCH_Q8,
        crate::greeting_programs::PITCH_Q8_BYTES,
    ),
];
