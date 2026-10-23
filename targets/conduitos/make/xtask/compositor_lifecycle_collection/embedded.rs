//! Structural binding of one compositor ELF actually present in loaded Root bytes.
//! This is not authenticated linker provenance or executed guest confinement.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Root,
    Missing,
    Ambiguous,
}
fn n(b: &[u8], at: usize, len: usize) -> Option<u64> {
    let bytes = b.get(at..at.checked_add(len)?)?;
    Some(
        bytes
            .iter()
            .enumerate()
            .fold(0, |v, (i, b)| v | (u64::from(*b) << (i * 8))),
    )
}
fn usize_at(b: &[u8], at: usize, len: usize) -> Option<usize> {
    usize::try_from(n(b, at, len)?).ok()
}
fn header(b: &[u8]) -> Option<()> {
    if b.get(..7)? != b"\x7fELF\x02\x01\x01"
        || n(b, 16, 2)? != 2
        || n(b, 18, 2)? != 62
        || n(b, 52, 2)? != 64
    {
        return None;
    }
    Some(())
}
fn loads(b: &[u8]) -> Option<Vec<(usize, usize, u64, u64, u64)>> {
    header(b)?;
    let off = usize_at(b, 32, 8)?;
    let count = usize_at(b, 56, 2)?;
    if n(b, 54, 2)? != 56 || count == 0 || count > 64 {
        return None;
    }
    b.get(off..off.checked_add(count.checked_mul(56)?)?)?;
    let mut out = Vec::new();
    for i in 0..count {
        let p = off + i * 56;
        if n(b, p, 4)? != 1 {
            continue;
        }
        let start = usize_at(b, p + 8, 8)?;
        let size = usize_at(b, p + 32, 8)?;
        let memory = n(b, p + 40, 8)?;
        if size as u64 > memory {
            return None;
        }
        b.get(start..start.checked_add(size)?)?;
        let address = n(b, p + 16, 8)?;
        address.checked_add(memory)?;
        out.push((start, size, address, memory, n(b, p + 4, 4)?));
    }
    (!out.is_empty()).then_some(out)
}
fn compositor_extent(b: &[u8]) -> Option<usize> {
    let loads = loads(b)?;
    let entry = n(b, 24, 8)?;
    if loads.iter().any(|x| !matches!(x.4, 4 | 5))
        || !loads
            .iter()
            .any(|x| x.4 == 5 && entry >= x.2 && entry < x.2.checked_add(x.3).unwrap())
    {
        return None;
    }
    let off = usize_at(b, 40, 8)?;
    let count = usize_at(b, 60, 2)?;
    if n(b, 58, 2)? != 64 || count == 0 || count > 4096 {
        return None;
    }
    let mut extent = off.checked_add(count.checked_mul(64)?)?;
    b.get(..extent)?;
    extent = extent
        .max(usize_at(b, 32, 8)?.checked_add(usize_at(b, 56, 2)?.checked_mul(56)?)?)
        .max(64);
    let mut matches = 0;
    for i in 0..count {
        let p = off + i * 64;
        let kind = n(b, p + 4, 4)?;
        let start = usize_at(b, p + 24, 8)?;
        let size = usize_at(b, p + 32, 8)?;
        if kind != 8 {
            extent = extent.max(start.checked_add(size)?);
            b.get(start..start.checked_add(size)?)?;
        }
        if kind != 2 {
            continue;
        }
        if n(b, p + 56, 8)? != 24 || size % 24 != 0 || size / 24 > 100_000 {
            return None;
        }
        let link = usize_at(b, p + 40, 4)?;
        if link >= count {
            return None;
        }
        let strings = off + link * 64;
        if n(b, strings + 4, 4)? != 3 {
            return None;
        }
        let a = usize_at(b, strings + 24, 8)?;
        let z = usize_at(b, strings + 32, 8)?;
        let text = b.get(a..a.checked_add(z)?)?;
        for symbol in (start..start.checked_add(size)?).step_by(24) {
            let name = usize_at(b, symbol, 4)?;
            let suffix = text.get(name..)?;
            let end = suffix.iter().position(|b| *b == 0)?;
            if &suffix[..end] == b"compositor_entry" {
                let info = *b.get(symbol + 4)?;
                if info >> 4 != 1
                    || info & 15 != 2
                    || n(b, symbol + 6, 2)? == 0
                    || n(b, symbol + 8, 8)? != entry
                {
                    return None;
                }
                matches += 1;
            }
        }
    }
    for (start, size, _, _, _) in loads {
        extent = extent.max(start.checked_add(size)?);
    }
    (matches == 1 && extent <= 64 * 1024 * 1024).then_some(extent)
}
pub(super) fn extract(root: &[u8]) -> Result<&[u8], Error> {
    let loaded = loads(root).ok_or(Error::Root)?;
    let mut found = None;
    for (offset, _) in root
        .windows(7)
        .enumerate()
        .filter(|(_, b)| *b == b"\x7fELF\x02\x01\x01")
    {
        if offset == 0 {
            continue;
        }
        let Some(length) = compositor_extent(&root[offset..]) else {
            continue;
        };
        let Some(end) = offset.checked_add(length) else {
            continue;
        };
        if !loaded.iter().any(|x| offset >= x.0 && end <= x.0 + x.1) {
            continue;
        }
        if found.is_some() {
            return Err(Error::Ambiguous);
        }
        found = root.get(offset..end);
    }
    found.ok_or(Error::Missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(b: &mut [u8], at: usize, n: u64, len: usize) {
        b[at..at + len].copy_from_slice(&n.to_le_bytes()[..len]);
    }
    fn elf() -> Vec<u8> {
        let mut b = vec![0; 512];
        b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        for (at, n, len) in [
            (16, 2, 2),
            (18, 62, 2),
            (24, 0x400000, 8),
            (32, 64, 8),
            (40, 320, 8),
            (52, 64, 2),
            (54, 56, 2),
            (56, 1, 2),
            (58, 64, 2),
            (60, 3, 2),
            (64, 1, 4),
            (68, 5, 4),
            (80, 0x400000, 8),
            (96, 512, 8),
            (104, 512, 8),
        ] {
            put(&mut b, at, n, len);
        }
        b[160..178].copy_from_slice(b"\0compositor_entry\0");
        put(&mut b, 192, 1, 4);
        b[196] = 0x12;
        put(&mut b, 198, 1, 2);
        put(&mut b, 200, 0x400000, 8);
        for (at, n, len) in [
            (388, 3, 4),
            (408, 160, 8),
            (416, 18, 8),
            (452, 2, 4),
            (472, 192, 8),
            (480, 24, 8),
            (488, 1, 4),
            (504, 24, 8),
        ] {
            put(&mut b, at, n, len);
        }
        b
    }
    fn root(guest: &[u8]) -> Vec<u8> {
        let mut b = elf();
        b.resize(1024 + guest.len(), 0);
        b[1024..].copy_from_slice(guest);
        let n = b.len() as u64;
        put(&mut b, 96, n, 8);
        put(&mut b, 104, n, 8);
        b
    }
    #[test]
    fn extracts_actual_loaded_symbol_bound_bytes() {
        let guest = elf();
        assert_eq!(extract(&root(&guest)).unwrap(), guest);
    }
    #[test]
    fn malformed_entry_permissions_extent_and_missing_symbol_refuse() {
        for (at, value, len) in [
            (200, 0x400001, 8),
            (68, 7, 4),
            (480, u64::MAX, 8),
            (198, 0, 2),
        ] {
            let mut guest = elf();
            put(&mut guest, at, value, len);
            assert_eq!(extract(&root(&guest)).unwrap_err(), Error::Missing);
        }
        let mut guest = elf();
        guest[161] = b'x';
        assert_eq!(extract(&root(&guest)).unwrap_err(), Error::Missing);
    }
    #[test]
    fn duplicate_valid_images_are_ambiguous() {
        let guest = elf();
        let mut b = root(&guest);
        b.extend_from_slice(&guest);
        let n = b.len() as u64;
        put(&mut b, 96, n, 8);
        put(&mut b, 104, n, 8);
        assert_eq!(extract(&b).unwrap_err(), Error::Ambiguous);
    }
}
