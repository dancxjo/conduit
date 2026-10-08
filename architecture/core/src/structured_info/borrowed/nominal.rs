//! Representation observations retain the validated immutable child scope.
use super::validated_extent::split_validated_type;
use super::{malformed, Cursor, StructuredInfoRefusal, ValidatedCanonicalStructuredValue};

impl<'a> ValidatedCanonicalStructuredValue<'a> {
    /// Observe the representation of an already validated nominal value.
    /// Callers must check the complete outer Type before choosing this view.
    pub fn nominal_representation(self) -> Result<Self, StructuredInfoRefusal> {
        let mut kind = Cursor::new(self.type_bytes);
        if kind.byte()? != 5 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        kind.text()?;
        let (representation, rest) = split_validated_type(kind.remaining)?;
        if !rest.is_empty() {
            return Err(malformed());
        }
        Ok(Self {
            type_bytes: representation,
            value_node: self.value_node,
        })
    }

    /// Exact primitive identity and bytes, including nested nominal wrappers.
    pub fn primitive(self) -> Result<(&'a str, &'a [u8]), StructuredInfoRefusal> {
        let mut view = self;
        for _ in 0..super::MAXIMUM_STRUCTURED_INFO_DEPTH {
            let mut kind = Cursor::new(view.type_bytes);
            match kind.byte()? {
                5 => view = view.nominal_representation()?,
                0 => {
                    let identity = kind.text()?;
                    let mut value = Cursor::new(view.value_node);
                    if !kind.remaining.is_empty() || value.byte()? != 0 {
                        return Err(malformed());
                    }
                    let bytes = value.bytes()?;
                    if !value.remaining.is_empty() {
                        return Err(malformed());
                    }
                    return Ok((identity, bytes));
                }
                _ => return Err(StructuredInfoRefusal::WrongType),
            }
        }
        Err(StructuredInfoRefusal::TooDeep)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        kind_id, validate_canonical_structured_value, StructuredInfoType, StructuredInfoValue,
    };
    use alloc::vec;

    #[test]
    fn nominal_child_observation_preserves_exact_outer_identity_and_bytes() {
        let primitive = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        let leaf = StructuredInfoValue::leaf(primitive.clone(), vec![7]).unwrap();
        let inner = StructuredInfoType::nominal(kind_id("test/inner"), primitive).unwrap();
        let value = StructuredInfoValue::nominal(inner.clone(), leaf).unwrap();
        let outer = StructuredInfoType::nominal(kind_id("test/outer"), inner).unwrap();
        let value = StructuredInfoValue::nominal(outer, value).unwrap();
        let encoded = value.canonical_bytes().unwrap();
        let view = validate_canonical_structured_value(&encoded).unwrap();
        let original = view.type_bytes();
        let child = view.nominal_representation().unwrap();
        assert_ne!(original, child.type_bytes());
        assert_eq!(view.type_bytes(), original);
        assert_eq!(view.primitive().unwrap(), ("value/u8", &[7][..]));
        assert_eq!(child.primitive().unwrap(), view.primitive().unwrap());
        assert_eq!(
            child
                .nominal_representation()
                .unwrap()
                .nominal_representation(),
            Err(crate::StructuredInfoRefusal::WrongType)
        );
    }
}
