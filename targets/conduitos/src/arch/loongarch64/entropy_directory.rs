//! Exact read-only firmware input descriptor; no prefix or implicit size match.
use crate::cryptographic_entropy::EntropyRefusal;
pub(super) const INPUT_BYTES: usize = 4096;
const NAME: &[u8] = b"opt/conduitos/cryptographic-entropy@1";

pub(super) fn selector(entry: &[u8; 64]) -> Result<Option<u16>, EntropyRefusal> {
    let name = &entry[8..];
    let end = name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name.len());
    if &name[..end] != NAME {
        return Ok(None);
    }
    let size = u32::from_be_bytes(entry[..4].try_into().unwrap());
    let selector = u16::from_be_bytes(entry[4..6].try_into().unwrap());
    if size != INPUT_BYTES as u32
        || !(0x20..0x4000).contains(&selector)
        || entry[6..8] != [0, 0]
        || name[end..].iter().any(|byte| *byte != 0)
    {
        return Err(EntropyRefusal::SourceFailure);
    }
    Ok(Some(selector))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> [u8; 64] {
        let mut entry = [0; 64];
        entry[..4].copy_from_slice(&(INPUT_BYTES as u32).to_be_bytes());
        entry[4..6].copy_from_slice(&0x20_u16.to_be_bytes());
        entry[8..8 + NAME.len()].copy_from_slice(NAME);
        entry
    }
    #[test]
    fn exact_name_size_and_read_only_channel_are_required() {
        assert_eq!(selector(&record()), Ok(Some(0x20)));
        for (index, byte) in [(3, 1), (4, 0x40), (5, 0x19), (6, 1), (63, 1)] {
            let mut altered = record();
            altered[index] = byte;
            assert_eq!(selector(&altered), Err(EntropyRefusal::SourceFailure));
        }
        let mut other = record();
        other[8] = b'x';
        assert_eq!(selector(&other), Ok(None));
        let mut prefix = record();
        prefix[8 + NAME.len()] = b'x';
        assert_eq!(selector(&prefix), Ok(None));
    }
}
