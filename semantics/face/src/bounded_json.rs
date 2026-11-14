//! Hosted identity encoding: count before allocation, then keep exact capacity.
//! Concrete callers own codec workspace independently; counting a compressed
//! PlannedMaskPlot is deliberately not an allocation-free preflight.
use alloc::vec::Vec;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{self, Write};

#[derive(Default)]
struct Count {
    bytes: Option<usize>,
}
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.and_then(|count| count.checked_add(bytes.len()));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Allocation-free only when T's own Serialize implementation is likewise so.
/// In particular callers must pass raw Plan, never PlannedMaskPlot, here.
pub(crate) fn count<T: Serialize + ?Sized>(value: &T) -> Option<usize> {
    let mut writer = Count { bytes: Some(0) };
    serde_json::to_writer(&mut writer, value).ok()?;
    writer.bytes
}

pub(crate) enum EncodingError {
    Invalid,
    Capacity { bytes: usize, maximum: usize },
}
struct Exact {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for Exact {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|next| next > self.maximum)
        {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(crate) fn encode<T: Serialize + ?Sized>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, EncodingError> {
    let bytes = count(value).ok_or(EncodingError::Invalid)?;
    if bytes > maximum {
        return Err(EncodingError::Capacity { bytes, maximum });
    }
    let mut writer = Exact {
        bytes: Vec::with_capacity(bytes),
        maximum: bytes,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| EncodingError::Invalid)?;
    if writer.bytes.len() != bytes {
        return Err(EncodingError::Invalid);
    }
    Ok(writer.bytes)
}

/// Same canonical JSON bytes as to_vec, streamed directly into SHA256. Neither
/// the advertisement nor an encoded copy is retained by this documentary pin.
pub(crate) fn digest<T: Serialize + ?Sized>(
    value: &T,
    maximum: usize,
) -> Result<[u8; 32], EncodingError> {
    struct Hash {
        bytes: Option<usize>,
        hash: Sha256,
    }
    impl Write for Hash {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes = self.bytes.and_then(|count| count.checked_add(bytes.len()));
            self.hash.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Hash {
        bytes: Some(0),
        hash: Sha256::new(),
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| EncodingError::Invalid)?;
    let bytes = writer.bytes.ok_or(EncodingError::Invalid)?;
    if bytes > maximum {
        return Err(EncodingError::Capacity { bytes, maximum });
    }
    Ok(writer.hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_identity_encoding_and_digest_match_existing_json() {
        let value = ("identity", "quotes\"/newline\n/unicodeé", [1u64, 20, 300]);
        let original = serde_json::to_vec(&value).unwrap();
        let encoded = encode(&value, original.len()).unwrap_or_else(|_| panic!("exact limit"));
        assert_eq!(encoded, original);
        let expected: [u8; 32] = Sha256::digest(&original).into();
        assert_eq!(digest(&value, original.len()).ok(), Some(expected));
        assert!(
            matches!(encode(&value, original.len() - 1), Err(EncodingError::Capacity { bytes, maximum })
            if bytes == original.len() && maximum + 1 == bytes)
        );
        assert!(matches!(
            digest(&value, original.len() - 1),
            Err(EncodingError::Capacity { .. })
        ));
    }
}
