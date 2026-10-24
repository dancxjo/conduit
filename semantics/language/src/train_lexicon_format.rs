//! Fixed CLXPOS1 resource decoding. No corpus, lexical inference or Source policy.
use super::{TrainLexiconRefusal as R, ENTRIES, MAXIMUM_SURFACE_BYTES, SOURCE_DIGEST};
use alloc::{boxed::Box, vec::Vec};
const HEADER: usize = 44;
pub(super) struct Row<'a> {
    pub surface: &'a str,
    pub codes: &'a [u8],
    pub end: usize,
}
pub(super) fn row(bytes: &[u8], start: usize) -> Result<Row<'_>, R> {
    let len_bytes: [u8; 2] = bytes
        .get(start..start.checked_add(2).ok_or(R::Bounds)?)
        .ok_or(R::Format)?
        .try_into()
        .map_err(|_| R::Format)?;
    let len = usize::from(u16::from_le_bytes(len_bytes));
    if len == 0 || len > MAXIMUM_SURFACE_BYTES {
        return Err(R::Bounds);
    }
    let begin = start.checked_add(2).ok_or(R::Bounds)?;
    let end = begin.checked_add(len).ok_or(R::Bounds)?;
    let surface =
        core::str::from_utf8(bytes.get(begin..end).ok_or(R::Format)?).map_err(|_| R::Utf8)?;
    let count = usize::from(*bytes.get(end).ok_or(R::Format)?);
    if !(1..=4).contains(&count) {
        return Err(R::Candidates);
    }
    let first = end.checked_add(1).ok_or(R::Bounds)?;
    let end = first.checked_add(count).ok_or(R::Bounds)?;
    let codes = bytes.get(first..end).ok_or(R::Format)?;
    if codes.iter().any(|code| *code > 16) || codes.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(R::Candidates);
    }
    Ok(Row {
        surface,
        codes,
        end,
    })
}
pub(super) fn validate(bytes: &[u8], expected: usize) -> Result<Box<[u32]>, R> {
    if bytes.get(..8) != Some(b"CLXPOS1\0") || bytes.get(8..40) != Some(SOURCE_DIGEST.as_slice()) {
        return Err(R::Format);
    }
    let count = u32::from_le_bytes(
        bytes
            .get(40..HEADER)
            .ok_or(R::Format)?
            .try_into()
            .map_err(|_| R::Format)?,
    ) as usize;
    if count != expected || count == 0 || count > ENTRIES {
        return Err(R::Bounds);
    }
    let mut offsets = Vec::with_capacity(count + 1);
    let mut cursor = HEADER;
    let mut previous: Option<&str> = None;
    for _ in 0..count {
        offsets.push(u32::try_from(cursor).map_err(|_| R::Bounds)?);
        let next = row(bytes, cursor)?;
        if previous.is_some_and(|prior| prior.as_bytes() >= next.surface.as_bytes()) {
            return Err(R::Ordering);
        }
        previous = Some(next.surface);
        cursor = next.end;
    }
    if cursor != bytes.len() {
        return Err(R::Format);
    }
    offsets.push(u32::try_from(cursor).map_err(|_| R::Bounds)?);
    Ok(offsets.into_boxed_slice())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn blob(rows: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bytes = b"CLXPOS1\0".to_vec();
        bytes.extend_from_slice(&SOURCE_DIGEST);
        bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
        for (surface, codes) in rows {
            bytes.extend_from_slice(&(surface.len() as u16).to_le_bytes());
            bytes.extend_from_slice(surface.as_bytes());
            bytes.push(codes.len() as u8);
            bytes.extend_from_slice(codes);
        }
        bytes
    }
    #[test]
    fn exact_rows_keep_all_candidates_and_utf8_byte_order() {
        let bytes = blob(&[("Alpha", &[7, 11]), ("alpha", &[7]), ("é", &[0, 7, 11, 16])]);
        let offsets = validate(&bytes, 3).unwrap();
        assert_eq!(offsets.len(), 4);
        assert_eq!(offsets[3] as usize, bytes.len());
        let first = row(&bytes, offsets[0] as usize).unwrap();
        assert_eq!(first.surface, "Alpha");
        assert_eq!(first.codes, [7, 11]);
        let last = row(&bytes, offsets[2] as usize).unwrap();
        assert_eq!(last.surface, "é");
        assert_eq!(last.codes, [0, 7, 11, 16]);
    }
    #[test]
    fn bounded_exact_lookup_preserves_whole_rows_and_case() {
        // Private decoder fixture only; it is never publicly admitted as the
        // pinned TRAIN artifact and supplies no resource/read authority proof.
        let bytes = blob(&[("Alpha", &[7, 11]), ("alpha", &[7]), ("é", &[0, 7, 11, 16])]);
        let offsets = validate(&bytes, 3).unwrap();
        let owner = super::super::PreparedTrainLexicon {
            reference: super::super::pinned_train_lexicon_reference(),
            access: conduit_core::AdmittedResourceAccess {
                handle: conduit_core::ResourceHandleId::from("private-unit-fixture"),
                authority_grant: conduit_core::AuthorityGrantId::from("private-unit-fixture"),
                maximum_bytes: bytes.len() as u64,
                maximum_items: Some(3),
            },
            bytes: alloc::sync::Arc::from(bytes),
            offsets,
        };
        for (surface, ordinal, expected) in [
            ("Alpha", 0, &[7, 11][..]),
            ("alpha", 1, &[7][..]),
            ("é", 2, &[0, 7, 11, 16][..]),
        ] {
            let found = owner.lookup(surface).unwrap().unwrap();
            assert_eq!(found.ordinal(), ordinal);
            assert_eq!(found.pos_codes(), expected);
            assert_eq!(found.surface(), surface);
            assert!(core::ptr::eq(found.owner(), &owner));
            assert!(found.comparisons() <= super::super::MAXIMUM_LOOKUP_COMPARISONS);
            assert!(!found.row_canonical().is_empty());
        }
        for absent in ["ALPHA", "0", "between", "終"] {
            assert!(owner.lookup(absent).unwrap().is_none());
        }
        assert!(matches!(owner.lookup(""), Err(R::QueryBound)));
    }
    #[test]
    fn noncanonical_and_incomplete_resources_refuse() {
        for bytes in [
            blob(&[("same", &[7]), ("same", &[11])]),
            blob(&[("z", &[7]), ("a", &[7])]),
        ] {
            assert_eq!(validate(&bytes, 2).unwrap_err(), R::Ordering);
        }
        for codes in [&[][..], &[7, 7], &[11, 7], &[17], &[0, 1, 2, 3, 4]] {
            assert_eq!(
                validate(&blob(&[("a", codes)]), 1).unwrap_err(),
                R::Candidates
            );
        }
        assert_eq!(validate(&blob(&[("", &[7])]), 1).unwrap_err(), R::Bounds);
        let mut bytes = blob(&[("a", &[7])]);
        bytes.push(0);
        assert_eq!(validate(&bytes, 1).unwrap_err(), R::Format);
        bytes.pop();
        bytes.pop();
        assert!(validate(&bytes, 1).is_err());
        let mut bytes = blob(&[("a", &[7])]);
        bytes[8] ^= 1;
        assert_eq!(validate(&bytes, 1).unwrap_err(), R::Format);
        assert_eq!(validate(&blob(&[("a", &[7])]), 2).unwrap_err(), R::Bounds);
    }
}
