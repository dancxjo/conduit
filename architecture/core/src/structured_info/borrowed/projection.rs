//! Borrow exact sequence elements and active variant payloads without copying.
use super::{
    malformed, split_type, validate_value, Cursor, StructuredInfoRefusal,
    ValidatedCanonicalStructuredValue, MAXIMUM_STRUCTURED_INFO_NODES,
};
impl<'a> ValidatedCanonicalStructuredValue<'a> {
    pub fn collection_index(self, index: u16) -> Result<Option<Self>, StructuredInfoRefusal> {
        let mut representation = self.type_bytes;
        for _ in 0..super::MAXIMUM_STRUCTURED_INFO_DEPTH {
            let mut nominal = Cursor::new(representation);
            if nominal.byte()? != 5 {
                break;
            }
            nominal.text()?;
            representation = nominal.remaining;
        }
        let mut kind = Cursor::new(representation);
        match kind.byte()? {
            1 => {
                kind.u16()?;
            }
            4 => {
                kind.u16()?;
                kind.u16()?;
            }
            _ => return Err(StructuredInfoRefusal::WrongType),
        }
        let mut scratch = MAXIMUM_STRUCTURED_INFO_NODES;
        let (element, remaining) = split_type(kind.remaining, 1, &mut scratch)?;
        if !remaining.is_empty() {
            return Err(malformed());
        }
        let mut value = Cursor::new(self.value_node);
        if value.byte()? != 1 {
            return Err(malformed());
        }
        let count = value.length()?;
        if usize::from(index) >= count {
            return Ok(None);
        }
        let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        for current in 0..=usize::from(index) {
            let beginning = value.remaining;
            validate_value(element, &mut value, 1, &mut nodes)?;
            if current == usize::from(index) {
                return Ok(Some(Self {
                    type_bytes: element,
                    value_node: &beginning[..beginning.len() - value.remaining.len()],
                }));
            }
        }
        Ok(None)
    }
    pub fn variant_payload(self, wanted: &str) -> Result<Option<Self>, StructuredInfoRefusal> {
        let mut kind = Cursor::new(self.type_bytes);
        if kind.byte()? != 3 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        kind.text()?;
        let count = kind.length()?;
        let mut value = Cursor::new(self.value_node);
        if value.byte()? != 3 {
            return Err(malformed());
        }
        let active = value.text()?;
        if active != wanted {
            return Ok(None);
        }
        for _ in 0..count {
            let tag = kind.text()?;
            let mut scratch = MAXIMUM_STRUCTURED_INFO_NODES;
            let (payload, remaining) = split_type(kind.remaining, 1, &mut scratch)?;
            kind.remaining = remaining;
            if tag == wanted {
                return Ok(Some(Self {
                    type_bytes: payload,
                    value_node: value.remaining,
                }));
            }
        }
        Err(malformed())
    }
}
