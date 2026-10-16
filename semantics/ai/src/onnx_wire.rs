//! Preparation-only, bounded protobuf field traversal for imported ONNX metadata.
//! This decodes wire fields, never operators, paths, or external resources.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireRefusal {
    Truncated,
    Overflow,
    InvalidField,
    UnsupportedWireType,
    FieldBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value<'a> {
    Integer(u64),
    Bytes(&'a [u8]),
    Fixed32,
    Fixed64,
}

pub(crate) struct Fields<'a> {
    bytes: &'a [u8],
    remaining: usize,
}

impl<'a> Fields<'a> {
    pub(crate) fn new(bytes: &'a [u8], maximum_fields: usize) -> Self {
        Self {
            bytes,
            remaining: maximum_fields,
        }
    }

    pub(crate) fn next(&mut self) -> Result<Option<(u32, Value<'a>)>, WireRefusal> {
        if self.bytes.is_empty() {
            return Ok(None);
        }
        if self.remaining == 0 {
            return Err(WireRefusal::FieldBudget);
        }
        self.remaining -= 1;
        let key = self.integer()?;
        let field = key >> 3;
        if field == 0 || field > 0x1fff_ffff {
            return Err(WireRefusal::InvalidField);
        }
        let value = match key & 7 {
            0 => Value::Integer(self.integer()?),
            1 => {
                self.take(8)?;
                Value::Fixed64
            }
            2 => {
                let length = usize::try_from(self.integer()?).map_err(|_| WireRefusal::Overflow)?;
                Value::Bytes(self.take(length)?)
            }
            5 => {
                self.take(4)?;
                Value::Fixed32
            }
            _ => return Err(WireRefusal::UnsupportedWireType),
        };
        Ok(Some((field as u32, value)))
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WireRefusal> {
        let value = self.bytes.get(..length).ok_or(WireRefusal::Truncated)?;
        self.bytes = &self.bytes[length..];
        Ok(value)
    }

    fn integer(&mut self) -> Result<u64, WireRefusal> {
        let mut value = 0_u64;
        for index in 0..10 {
            let byte = self.take(1)?[0];
            if index == 9 && byte > 1 {
                return Err(WireRefusal::Overflow);
            }
            value |= u64::from(byte & 127) << (index * 7);
            if byte & 128 == 0 {
                return Ok(value);
            }
        }
        Err(WireRefusal::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_borrow_exact_payload_and_preserve_order() {
        let mut fields = Fields::new(&[8, 9, 18, 2, 42, 43, 29, 0, 0, 0, 0], 3);
        assert_eq!(fields.next(), Ok(Some((1, Value::Integer(9)))));
        assert_eq!(fields.next(), Ok(Some((2, Value::Bytes(&[42, 43])))));
        assert_eq!(fields.next(), Ok(Some((3, Value::Fixed32))));
        assert_eq!(fields.next(), Ok(None));
    }

    #[test]
    fn malformed_fields_and_work_exhaustion_refuse() {
        for (bytes, refusal) in [
            (&[0][..], WireRefusal::InvalidField),
            (&[11][..], WireRefusal::UnsupportedWireType),
            (&[18, 3, 1][..], WireRefusal::Truncated),
            (&[8, 128][..], WireRefusal::Truncated),
            (
                &[8, 255, 255, 255, 255, 255, 255, 255, 255, 255, 2][..],
                WireRefusal::Overflow,
            ),
        ] {
            assert_eq!(Fields::new(bytes, 8).next(), Err(refusal));
        }
        assert_eq!(
            Fields::new(&[8, 0], 0).next(),
            Err(WireRefusal::FieldBudget)
        );
        let mut fields = Fields::new(&[8, 0, 8, 1], 1);
        assert!(fields.next().is_ok());
        assert_eq!(fields.next(), Err(WireRefusal::FieldBudget));
    }
}
