//! Linux-only requested-capacity serialization; no grow or partial readiness.
extern crate std;
use super::*;
use std::io::{self, Write};
struct BoundedJson {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let remaining = self.maximum.saturating_sub(self.bytes.len());
        if bytes.len() > remaining {
            return Err(io::ErrorKind::WriteZero.into());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn encode(
    value: &impl serde::Serialize,
    maximum: usize,
) -> Result<Vec<u8>, NativeMaskPlayError> {
    let mut output = BoundedJson {
        bytes: Vec::with_capacity(maximum),
        maximum,
    };
    serde_json::to_writer(&mut output, value).map_err(|_| NativeMaskPlayError::Value)?;
    Ok(output.bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_json_preserves_exact_wire_and_refuses_escape_expansion() {
        let value = "\u{1}\u{2} native";
        let expected = serde_json::to_vec(value).unwrap();
        assert_eq!(encode(&value, expected.len()).unwrap(), expected);
        assert!(encode(&value, expected.len() - 1).is_err());
    }
}
