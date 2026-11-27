//! Hosted preparation serializer with an actual fixed output allocation.
use serde::Serialize;
use std::io::{Error, Write};

struct Bounded {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(Error::other("owner Mask JSON extent"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn encode(value: &impl Serialize, maximum: usize) -> Result<Vec<u8>, String> {
    if maximum == 0 {
        return Err("owner Mask JSON extent".into());
    }
    let mut output = Bounded {
        bytes: Vec::with_capacity(maximum),
        maximum,
    };
    serde_json::to_writer(&mut output, value).map_err(|error| error.to_string())?;
    if output.bytes.is_empty() {
        return Err("owner Mask empty JSON".into());
    }
    Ok(output.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_limit_succeeds_and_escape_expansion_refuses_before_growth() {
        assert_eq!(encode(&"abc", 5).unwrap(), b"\"abc\"");
        assert!(encode(&"abc", 4).is_err());
        assert!(encode(&"\n".repeat(1024), 1024).is_err());
        let output = encode(&true, 7).unwrap();
        assert_eq!(output, b"true");
        assert_eq!(output.capacity(), 7);
    }
}
