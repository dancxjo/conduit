//! Finite physical-memory admission for the Nano boot handoff.
//! Firmware RAM descriptions are candidates, never permission to overwrite a
//! reservation. The caller must include the image, stack, DTB and live firmware
//! ranges among exclusions before obtaining a writable placement.

pub const MAX_REGIONS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Empty,
    Overflow,
    Capacity,
    Overlap,
    OutsideRam,
    Reserved,
    Alignment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Region {
    start: u64,
    end: u64,
}

impl Region {
    pub fn new(start: u64, bytes: u64) -> Result<Self, Refusal> {
        if bytes == 0 {
            return Err(Refusal::Empty);
        }
        Ok(Self {
            start,
            end: start.checked_add(bytes).ok_or(Refusal::Overflow)?,
        })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn bytes(self) -> u64 {
        self.end - self.start
    }

    fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

#[derive(Debug)]
pub struct Regions {
    entries: [Option<Region>; MAX_REGIONS],
    len: usize,
}

impl Regions {
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_REGIONS],
            len: 0,
        }
    }

    /// RAM banks must be disjoint. Reservation lists may overlap: firmware
    /// commonly repeats a carveout in both the DTB map and reserved-memory.
    pub fn push(&mut self, region: Region, disjoint: bool) -> Result<(), Refusal> {
        if disjoint && self.iter().any(|entry| entry.overlaps(region)) {
            return Err(Refusal::Overlap);
        }
        let slot = self.entries.get_mut(self.len).ok_or(Refusal::Capacity)?;
        *slot = Some(region);
        self.len += 1;
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = Region> + '_ {
        self.entries[..self.len].iter().flatten().copied()
    }
}

/// Only a fully covered, aligned, unreserved region is admitted. This does not
/// establish a mapping, device authority, DMA confinement or usable boot state.
pub fn admit(
    ram: &Regions,
    reserved: &Regions,
    placement: Region,
    alignment: u64,
) -> Result<Region, Refusal> {
    if !alignment.is_power_of_two() || placement.start & (alignment - 1) != 0 {
        return Err(Refusal::Alignment);
    }
    if !ram.iter().any(|bank| bank.contains(placement)) {
        return Err(Refusal::OutsideRam);
    }
    if reserved.iter().any(|range| range.overlaps(placement)) {
        return Err(Refusal::Reserved);
    }
    Ok(placement)
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
