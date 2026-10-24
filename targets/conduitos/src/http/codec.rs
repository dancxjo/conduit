//! Bounded canonical request reader and response writer/size preflight.
use super::HttpClientFailure;
pub(super) struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], HttpClientFailure> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(HttpClientFailure::MalformedRequest)?;
        let value = self
            .input
            .get(self.offset..end)
            .ok_or(HttpClientFailure::MalformedRequest)?;
        self.offset = end;
        Ok(value)
    }
    pub(super) fn byte(&mut self) -> Result<u8, HttpClientFailure> {
        Ok(self.take(1)?[0])
    }
    pub(super) fn u32(&mut self) -> Result<u32, HttpClientFailure> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| HttpClientFailure::MalformedRequest)?,
        ))
    }
    pub(super) fn bytes(&mut self) -> Result<&'a [u8], HttpClientFailure> {
        let len = usize::try_from(self.u32()?).map_err(|_| HttpClientFailure::MalformedRequest)?;
        self.take(len)
    }
    pub(super) fn finished(&self) -> bool {
        self.offset == self.input.len()
    }
    pub(super) fn tag(&mut self, expected: u8) -> Result<(), HttpClientFailure> {
        if self.byte()? == expected {
            Ok(())
        } else {
            Err(HttpClientFailure::MalformedRequest)
        }
    }
    pub(super) fn record(&mut self, fields: usize) -> Result<(), HttpClientFailure> {
        self.tag(2)?;
        if self.u32()? as usize == fields {
            Ok(())
        } else {
            Err(HttpClientFailure::MalformedRequest)
        }
    }
    pub(super) fn collection_len(&mut self) -> Result<usize, HttpClientFailure> {
        self.tag(1)?;
        Ok(self.u32()? as usize)
    }
    pub(super) fn field(&mut self, expected: &str) -> Result<(), HttpClientFailure> {
        if self.bytes()? == expected.as_bytes() {
            Ok(())
        } else {
            Err(HttpClientFailure::MalformedRequest)
        }
    }
    pub(super) fn variant(&mut self, expected: &str) -> Result<(), HttpClientFailure> {
        self.tag(3)?;
        self.field(expected)
    }
    pub(super) fn leaf(&mut self) -> Result<&'a [u8], HttpClientFailure> {
        self.tag(0)?;
        self.bytes()
    }
}

pub(super) struct Writer<'a> {
    output: Option<&'a mut [u8]>,
    limit: usize,
    pub(super) len: usize,
}

impl<'a> Writer<'a> {
    pub(super) fn new(output: &'a mut [u8]) -> Self {
        let limit = output.len();
        Self {
            output: Some(output),
            limit,
            len: 0,
        }
    }
    pub(super) fn measure(limit: usize) -> Self {
        Self {
            output: None,
            limit,
            len: 0,
        }
    }
    pub(super) fn put(&mut self, value: &[u8]) -> Result<(), HttpClientFailure> {
        let end = self
            .len
            .checked_add(value.len())
            .ok_or(HttpClientFailure::RequestOverflow)?;
        if end > self.limit {
            return Err(HttpClientFailure::RequestOverflow);
        }
        if let Some(output) = &mut self.output {
            output[self.len..end].copy_from_slice(value);
        }
        self.len = end;
        Ok(())
    }
    pub(super) fn byte(&mut self, value: u8) -> Result<(), HttpClientFailure> {
        self.put(&[value])
    }
    pub(super) fn sized(&mut self, value: &[u8]) -> Result<(), HttpClientFailure> {
        let len =
            u32::try_from(value.len()).map_err(|_| HttpClientFailure::ResponseBodyOverflow)?;
        self.put(&len.to_le_bytes())
            .map_err(|_| HttpClientFailure::ResponseBodyOverflow)?;
        self.put(value)
            .map_err(|_| HttpClientFailure::ResponseBodyOverflow)
    }
    pub(super) fn record(&mut self, fields: usize) -> Result<(), HttpClientFailure> {
        self.byte(2)?;
        self.put(&(fields as u32).to_le_bytes())
    }
    pub(super) fn collection(&mut self, items: usize) -> Result<(), HttpClientFailure> {
        self.byte(1)?;
        self.put(&(items as u32).to_le_bytes())
    }
    pub(super) fn field(&mut self, name: &str) -> Result<(), HttpClientFailure> {
        self.sized(name.as_bytes())
    }
    pub(super) fn variant(&mut self, tag: &str) -> Result<(), HttpClientFailure> {
        self.byte(3)?;
        self.sized(tag.as_bytes())
    }
    pub(super) fn leaf(&mut self, value: &[u8]) -> Result<(), HttpClientFailure> {
        self.byte(0)?;
        self.sized(value)
    }
    pub(super) fn decimal(&mut self, mut value: usize) -> Result<(), HttpClientFailure> {
        let mut digits = [0_u8; 20];
        let mut index = digits.len();
        loop {
            index -= 1;
            digits[index] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.put(&digits[index..])
    }
}
