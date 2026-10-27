//! Address arithmetic only; these values construct no flash owner or erase grant.
pub(super) const TOTAL: u32 = 0x20000;
pub(super) const DATA: u32 = 0x1e000;
pub(super) const SECTOR: u32 = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Geometry {
    pub(super) origin: u32,
    pub(super) offset: u32,
}
impl Geometry {
    pub(super) fn from_view(partition: u32, start: u32, length: u32) -> Option<Self> {
        if !start.is_multiple_of(4) || !length.is_multiple_of(4) || length == 0 {
            return None;
        }
        let relative = start.checked_sub(partition)?;
        let index = relative / TOTAL;
        if index >= 5 {
            return None;
        }
        let origin = partition.checked_add(index.checked_mul(TOTAL)?)?;
        let offset = start.checked_sub(origin)?;
        if offset >= DATA || start.checked_add(length)? != origin.checked_add(TOTAL)? {
            return None;
        }
        Some(Self { origin, offset })
    }
    pub(super) fn data(&self, at: u32, length: usize) -> Option<u32> {
        let length = u32::try_from(length).ok()?;
        if !at.is_multiple_of(4) || !length.is_multiple_of(4) {
            return None;
        }
        let relative = self.offset.checked_add(at)?;
        if relative.checked_add(length)? > DATA {
            return None;
        }
        Some(relative)
    }
    pub(super) fn whole(at: usize, length: usize) -> Option<u32> {
        let at = u32::try_from(at).ok()?;
        let length = u32::try_from(length).ok()?;
        if !at.is_multiple_of(4) || !length.is_multiple_of(4) || at.checked_add(length)? > TOTAL {
            return None;
        }
        Some(at)
    }
    pub(super) fn marker(bank: u8, at: usize, length: usize) -> Option<u32> {
        if bank > 1 {
            return None;
        }
        let at = u32::try_from(at).ok()?;
        let length = u32::try_from(length).ok()?;
        if !at.is_multiple_of(4) || !length.is_multiple_of(4) || at.checked_add(length)? > SECTOR {
            return None;
        }
        DATA.checked_add(u32::from(bank) * SECTOR)?.checked_add(at)
    }
    pub(super) fn data_sector(sector: u8) -> Option<u32> {
        if u32::from(sector) >= DATA / SECTOR {
            return None;
        }
        Some(u32::from(sector) * SECTOR)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_exact_five_epochs_and_their_sealed_suffixes_convert() {
        let base = 0x348000;
        for i in 0..5 {
            let origin = base + i * TOTAL;
            for offset in [0, 4, 536, DATA - 4] {
                let g = Geometry::from_view(base, origin + offset, TOTAL - offset).unwrap();
                assert_eq!(g, Geometry { origin, offset });
            }
            assert!(Geometry::from_view(base, origin + DATA, TOTAL - DATA).is_none());
            assert!(Geometry::from_view(base, origin, TOTAL - 4).is_none());
        }
        for (start, len) in [
            (base - 4096, 4096),
            (base + 5 * TOTAL, TOTAL),
            (base + 1, TOTAL - 1),
            (base, 0),
            (u32::MAX - 3, 8),
        ] {
            assert!(Geometry::from_view(base, start, len).is_none());
        }
    }
    #[test]
    fn all_operations_are_confined_and_prefixes_cannot_program_tail() {
        let g = Geometry {
            origin: 0x348000,
            offset: 536,
        };
        assert_eq!(g.data(0, 4), Some(536));
        assert_eq!(g.data(DATA - 536 - 4, 4), Some(DATA - 4));
        assert!(g.data(DATA - 536, 4).is_none());
        assert!(g.data(u32::MAX - 3, 8).is_none());
        assert!(g.data(1, 4).is_none());
        for bank in 0..2 {
            assert_eq!(
                Geometry::marker(bank, 0, SECTOR as usize),
                Some(DATA + u32::from(bank) * SECTOR)
            );
            assert!(Geometry::marker(bank, SECTOR as usize, 4).is_none());
        }
        assert!(Geometry::marker(2, 0, 4).is_none());
        for sector in 0..30 {
            assert_eq!(
                Geometry::data_sector(sector),
                Some(u32::from(sector) * SECTOR)
            );
        }
        assert!(Geometry::data_sector(30).is_none());
        assert_eq!(Geometry::whole(TOTAL as usize - 4, 4), Some(TOTAL - 4));
        assert!(Geometry::whole(TOTAL as usize, 4).is_none());
        assert!(Geometry::whole(0, usize::MAX).is_none());
    }
}
