//! Validate complete finite CBOR extents before any typed allocation hint runs.
//! Like the existing private sequence encoder, this format uses known lengths.
use super::Refusal;
use ciborium_ll::{Decoder, Header};

pub(super) fn validate(mut bytes: &[u8]) -> Result<(), Refusal> {
    // One synthetic root and at most 64 real container/tag frames. Each token
    // consumes a byte, so even hostile counts cannot create unbounded work.
    let mut pending = [0usize; 65];
    pending[0] = 1;
    let mut depth = 1;
    while depth != 0 {
        if pending[depth - 1] == 0 {
            depth -= 1;
            continue;
        }
        pending[depth - 1] -= 1;
        let header = Decoder::from(&mut bytes)
            .pull()
            .map_err(|_| Refusal::Malformed)?;
        if matches!(header, Header::Array(_) | Header::Map(_) | Header::Tag(_))
            && depth == pending.len()
        {
            return Err(Refusal::Malformed);
        }
        let children = match header {
            Header::Array(Some(n)) => n,
            Header::Map(Some(n)) => n.checked_mul(2).ok_or(Refusal::Malformed)?,
            Header::Tag(_) => 1,
            Header::Bytes(Some(n)) | Header::Text(Some(n)) => {
                let value = bytes.get(..n).ok_or(Refusal::Malformed)?;
                if matches!(header, Header::Text(_)) {
                    core::str::from_utf8(value).map_err(|_| Refusal::Malformed)?;
                }
                bytes = &bytes[n..];
                0
            }
            Header::Positive(_) | Header::Negative(_) | Header::Simple(20..=22) => 0,
            Header::Float(value) if value.is_finite() => 0,
            _ => return Err(Refusal::Malformed),
        };
        if children != 0 {
            if depth == pending.len() || children > bytes.len() {
                return Err(Refusal::Malformed);
            }
            pending[depth] = children;
            depth += 1;
        }
    }
    if bytes.is_empty() {
        Ok(())
    } else {
        Err(Refusal::Malformed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    #[test]
    fn hostile_lengths_trailing_utf8_and_nonfinite_values_refuse() {
        for bytes in [
            &[0x9b, 255, 255, 255, 255, 255, 255, 255, 255][..],
            &[0xa1, 0],
            &[0x00, 0x00],
            &[0x62, 0xff, 0xff],
            &[0x5a, 0xff, 0xff, 0xff, 0xff],
            &[0x9f, 0xff],
            &[0xf9, 0x7c, 0],
            &[0xfa, 0x7f, 0x80, 0, 0],
            &[0xfb, 0x7f, 0xf0, 0, 0, 0, 0, 0, 0],
        ] {
            assert_eq!(validate(bytes), Err(Refusal::Malformed));
        }
    }
    #[test]
    fn depth_and_numeric_extents_are_finite() {
        let mut nested = vec![0x81; 64];
        nested.push(0);
        assert_eq!(validate(&nested), Ok(()));
        nested.insert(0, 0x81);
        assert_eq!(validate(&nested), Err(Refusal::Malformed));
        for bytes in [
            &[0x1b, 255, 255, 255, 255, 255, 255, 255, 255][..],
            &[0x3b, 255, 255, 255, 255, 255, 255, 255, 255],
            &[0xc2, 0x41, 0xff],
            &[0xa1, 0x61, b'x', 0x82, 0xf4, 0xf6],
            &[0x40],
            &[0x80],
            &[0xa0],
        ] {
            assert_eq!(validate(bytes), Ok(()));
        }
    }

    #[test]
    fn empty_container_boundary_and_typed_recursion_remain_bounded() {
        for end in [0x80, 0xa0] {
            for containers in [64, 65] {
                let mut bytes = vec![0x81; containers - 1];
                bytes.push(end);
                assert_eq!(validate(&bytes).is_ok(), containers <= 64);
                let decoded: Result<ciborium::Value, _> =
                    ciborium::de::from_reader_with_recursion_limit(&bytes[..], 64);
                assert_eq!(decoded.is_ok(), containers <= 64);
            }
        }
        // Tags also consume our finite syntax depth, including ones a typed
        // visitor could ignore. They cannot become an unbounded alternate path.
        for tags in [64, 65] {
            let mut bytes = vec![0xc0; tags];
            bytes.push(0);
            assert_eq!(validate(&bytes).is_ok(), tags <= 64);
        }
    }
}
