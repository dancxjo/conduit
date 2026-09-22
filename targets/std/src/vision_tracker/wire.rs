//! Canonical StructuredInfo cursor and allocation-free emit helpers.

pub(super) struct Cursor<'a> {
    pub(super) remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    pub(super) fn new(remaining: &'a [u8]) -> Self {
        Self { remaining }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
        let (head, tail) = self
            .remaining
            .split_at_checked(length)
            .ok_or("truncated structured Vision value")?;
        self.remaining = tail;
        Ok(head)
    }
    fn byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }
    pub(super) fn expect(&mut self, expected: u8) -> Result<(), &'static str> {
        (self.byte()? == expected)
            .then_some(())
            .ok_or("unexpected structured Vision node")
    }
    pub(super) fn length(&mut self) -> Result<usize, &'static str> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| "invalid length")?) as usize)
    }
    fn bytes(&mut self) -> Result<&'a [u8], &'static str> {
        let length = self.length()?;
        self.take(length)
    }
    pub(super) fn field(&mut self, expected: &str) -> Result<(), &'static str> {
        (self.bytes()? == expected.as_bytes())
            .then_some(())
            .ok_or("unexpected structured Vision field")
    }
    pub(super) fn record(&mut self, fields: usize) -> Result<(), &'static str> {
        self.expect(2)?;
        (self.length()? == fields)
            .then_some(())
            .ok_or("unexpected structured Vision record")
    }
    pub(super) fn variant(&mut self) -> Result<(), &'static str> {
        self.expect(3)?;
        self.bytes()?;
        Ok(())
    }
    pub(super) fn leaf(&mut self) -> Result<&'a [u8], &'static str> {
        self.expect(0)?;
        self.bytes()
    }
}

pub(super) fn wire_collection(output: &mut Vec<u8>, length: u16) {
    output.push(1);
    output.extend_from_slice(&u32::from(length).to_le_bytes());
}
pub(super) fn wire_record(output: &mut Vec<u8>, length: u32) {
    output.push(2);
    output.extend_from_slice(&length.to_le_bytes());
}
pub(super) fn wire_variant(output: &mut Vec<u8>, tag: &str) {
    output.push(3);
    wire_text(output, tag);
}
pub(super) fn wire_leaf(output: &mut Vec<u8>, value: &[u8]) {
    output.push(0);
    wire_bytes(output, value);
}
pub(super) fn wire_text(output: &mut Vec<u8>, value: &str) {
    wire_bytes(output, value.as_bytes());
}
fn wire_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}
pub(super) fn text_field(output: &mut Vec<u8>, name: &str, value: &str) {
    wire_text(output, name);
    wire_leaf(output, value.as_bytes());
}
pub(super) fn count_field(output: &mut Vec<u8>, name: &str, value: u64) {
    wire_text(output, name);
    wire_leaf(output, &conduit_core::encode_count(value));
}
