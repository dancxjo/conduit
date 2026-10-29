//! Compact the closed private schema's reserved byte sequences in place.
//! Read/write cursors only move forward and write never overtakes unread data.
use super::Refusal;

pub(super) fn compact(bytes: &mut [u8]) -> Result<usize, Refusal> {
    let mut pack = Pack {
        bytes,
        read: 0,
        written: 0,
    };
    pack.value(0, false)?;
    if pack.read != pack.bytes.len() {
        return Err(Refusal::Malformed);
    }
    Ok(pack.written)
}
struct Pack<'a> {
    bytes: &'a mut [u8],
    read: usize,
    written: usize,
}
impl Pack<'_> {
    fn header(&mut self) -> Result<(u8, u64), Refusal> {
        let byte = *self.bytes.get(self.read).ok_or(Refusal::Malformed)?;
        self.read += 1;
        let width = match byte & 31 {
            0..=23 => 0,
            24 => 1,
            25 => 2,
            26 => 4,
            27 => 8,
            _ => return Err(Refusal::Malformed),
        };
        let end = self.read.checked_add(width).ok_or(Refusal::Bounds)?;
        let tail = self.bytes.get(self.read..end).ok_or(Refusal::Malformed)?;
        let n = if width == 0 {
            u64::from(byte & 31)
        } else {
            tail.iter().fold(0, |n, b| (n << 8) | u64::from(*b))
        };
        self.read = end;
        Ok((byte >> 5, n))
    }
    fn copy(&mut self, start: usize) {
        let length = self.read - start;
        self.bytes.copy_within(start..self.read, self.written);
        self.written += length;
    }
    fn extent(&mut self, n: u64) -> Result<(), Refusal> {
        self.read = self
            .read
            .checked_add(usize::try_from(n).map_err(|_| Refusal::Bounds)?)
            .ok_or(Refusal::Bounds)?;
        if self.read > self.bytes.len() {
            return Err(Refusal::Malformed);
        }
        Ok(())
    }
    fn value(&mut self, depth: usize, packed: bool) -> Result<(), Refusal> {
        if depth > 64 {
            return Err(Refusal::Bounds);
        }
        let start = self.read;
        let (major, n) = self.header()?;
        if packed {
            if major != 4 || n > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u64 {
                return Err(Refusal::Malformed);
            }
            let output_header = self.written;
            self.copy(start);
            self.bytes[output_header] = (self.bytes[output_header] & 31) | 0x40;
            for _ in 0..n {
                let (major, value) = self.header()?;
                if major != 0 || value > 255 {
                    return Err(Refusal::Malformed);
                }
                self.bytes[self.written] = value as u8;
                self.written += 1;
            }
            return Ok(());
        }
        self.copy(start);
        match major {
            0 | 1 | 7 => {}
            2 | 3 => {
                let start = self.read;
                self.extent(n)?;
                self.copy(start);
            }
            4 | 5 => {
                if n > (self.bytes.len() - self.read) as u64 {
                    return Err(Refusal::Malformed);
                }
                for _ in 0..n {
                    let packed = if major == 5 {
                        let key_start = self.read;
                        let (key_major, key_n) = self.header()?;
                        let body = self.read;
                        let reserved = if key_major == 3 {
                            self.extent(key_n)?;
                            &self.bytes[body..self.read] == b"canonical_value"
                        } else {
                            false
                        };
                        self.read = key_start;
                        self.value(depth + 1, false)?;
                        reserved
                    } else {
                        false
                    };
                    self.value(depth + 1, packed)?;
                }
            }
            6 => self.value(depth + 1, false)?,
            _ => return Err(Refusal::Malformed),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};
    use serde::{Deserialize, Serialize};
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Record {
        canonical_value: Vec<u8>,
        other: Vec<u8>,
    }
    #[test]
    fn reserved_bytes_compact_and_round_trip() {
        let record = Record {
            canonical_value: vec![0, 23, 24, 255],
            other: vec![0, 23, 24, 255],
        };
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&record, &mut bytes).unwrap();
        let old_len = bytes.len();
        let n = compact(&mut bytes).unwrap();
        bytes.truncate(n);
        assert_eq!(old_len - n, 2);
        let value: ciborium::Value = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        let map = value.as_map().unwrap();
        assert_eq!(map[0].1.as_bytes().unwrap(), &[0, 23, 24, 255]);
        assert!(map[1].1.as_array().is_some());
        assert_eq!(
            super::super::decode_preflighted::<Record>(&bytes).unwrap(),
            record
        );
    }
    #[test]
    fn multibyte_counts_and_every_byte_preserve_nested_models() {
        for length in [0, 23, 24, 255, 256, 1024] {
            let record = Record {
                canonical_value: (0..length).map(|n| n as u8).collect(),
                other: (0..length).map(|n| n as u8).collect(),
            };
            let records = vec![record];
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&records, &mut bytes).unwrap();
            let length = compact(&mut bytes).unwrap();
            bytes.truncate(length);
            assert_eq!(
                super::super::decode_preflighted::<Vec<Record>>(&bytes).unwrap(),
                records
            );
        }
    }
    #[test]
    fn malformed_reserved_values_and_truncations_refuse() {
        for value in [
            ciborium::Value::Integer(256.into()),
            ciborium::Value::Integer((-1).into()),
            ciborium::Value::Text("bad".into()),
        ] {
            let value = ciborium::Value::Map(vec![(
                "canonical_value".into(),
                ciborium::Value::Array(vec![value]),
            )]);
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&value, &mut bytes).unwrap();
            assert!(compact(&mut bytes).is_err());
        }
        let record = Record {
            canonical_value: vec![0, 23, 24, 255],
            other: vec![1, 2],
        };
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&record, &mut bytes).unwrap();
        for end in 0..bytes.len() {
            assert!(compact(&mut bytes[..end].to_vec()).is_err());
        }
    }
}
