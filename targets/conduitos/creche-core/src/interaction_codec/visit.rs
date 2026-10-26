use super::{
    access::{Enum, Map, Seq},
    decode::Sequences,
    positive::Positive,
};
use alloc::vec::Vec;
use core::{fmt, marker::PhantomData};
use serde::de::{self, Visitor};
#[derive(Clone, Copy)]
pub(super) enum Gate {
    Any,
    Seq(Option<usize>),
    Identifier,
}
pub(super) struct Wrapped<V> {
    pub visitor: V,
    pub depth: u8,
    pub gate: Gate,
}
impl<V> Wrapped<V> {
    fn scalar<E: de::Error>(&self) -> Result<(), E> {
        if matches!(self.gate, Gate::Seq(_)) {
            Err(E::custom("expected sequence"))
        } else {
            Ok(())
        }
    }
    fn descend<E: de::Error>(&self) -> Result<u8, E> {
        self.depth
            .checked_sub(1)
            .ok_or_else(|| E::custom("recursion limit exceeded"))
    }
}
impl<'de, V: Visitor<'de>> Visitor<'de> for Wrapped<V> {
    type Value = V::Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.visitor.expecting(f)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_bool(value)
    }
    fn visit_i8<E: de::Error>(self, value: i8) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_i8(value)
    }
    fn visit_i16<E: de::Error>(self, value: i16) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_i16(value)
    }
    fn visit_i32<E: de::Error>(self, value: i32) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_i32(value)
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_i64(value)
    }
    fn visit_i128<E: de::Error>(self, value: i128) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_i128(value)
    }
    fn visit_u8<E: de::Error>(self, value: u8) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_u8(value)
    }
    fn visit_u16<E: de::Error>(self, value: u16) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_u16(value)
    }
    fn visit_u32<E: de::Error>(self, value: u32) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_u32(value)
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_u64(value)
    }
    fn visit_u128<E: de::Error>(self, value: u128) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_u128(value)
    }
    fn visit_f32<E: de::Error>(self, value: f32) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_f32(value)
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_f64(value)
    }
    fn visit_char<E: de::Error>(self, value: char) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_char(value)
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_str(value)
    }
    fn visit_borrowed_str<E: de::Error>(self, value: &'de str) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_borrowed_str(value)
    }
    fn visit_string<E: de::Error>(self, value: alloc::string::String) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_string(value)
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_unit()
    }
    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        self.scalar()?;
        self.visitor.visit_none()
    }
    fn visit_some<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.scalar()?;
        self.visitor.visit_some(Sequences {
            inner: d,
            depth: self.depth,
        })
    }
    fn visit_newtype_struct<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.scalar()?;
        self.visitor.visit_newtype_struct(Sequences {
            inner: d,
            depth: self.depth,
        })
    }
    fn visit_seq<A: de::SeqAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        let depth = self.descend()?;
        let expected = if let Gate::Seq(n) = self.gate {
            n
        } else {
            None
        };
        let mut access = Seq {
            inner: access,
            depth,
            count: 0,
            ended: false,
        };
        let result = self.visitor.visit_seq(&mut access)?;
        if !access.ended && de::SeqAccess::next_element::<de::IgnoredAny>(&mut access)?.is_some() {
            return Err(de::Error::custom("unconsumed sequence element"));
        }
        if expected.is_some_and(|n| n != access.count) {
            return Err(de::Error::custom("wrong tuple length"));
        }
        Ok(result)
    }
    fn visit_map<A: de::MapAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        self.scalar()?;
        let depth = self.descend()?;
        self.visitor.visit_map(Map {
            inner: access,
            depth,
        })
    }
    fn visit_enum<A: de::EnumAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        self.scalar()?;
        let depth = self.descend()?;
        self.visitor.visit_enum(Enum {
            inner: access,
            depth,
        })
    }
    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
        self.packed(value)
    }
    fn visit_borrowed_bytes<E: de::Error>(self, value: &'de [u8]) -> Result<Self::Value, E> {
        self.packed(value)
    }
    fn visit_byte_buf<E: de::Error>(self, value: Vec<u8>) -> Result<Self::Value, E> {
        self.packed(&value)
    }
}
impl<'de, V: Visitor<'de>> Wrapped<V> {
    fn packed<E: de::Error>(self, bytes: &[u8]) -> Result<V::Value, E> {
        if matches!(self.gate, Gate::Identifier) {
            return Err(E::custom("packed sequence is not an identifier"));
        }
        self.descend::<E>()?;
        if let Gate::Seq(Some(n)) = self.gate
            && n != bytes.len()
        {
            return Err(E::custom("wrong tuple length"));
        }
        let mut access = Bytes::<E> {
            bytes,
            marker: PhantomData,
        };
        let result = self.visitor.visit_seq(&mut access)?;
        if !access.bytes.is_empty() {
            return Err(E::custom("unconsumed packed sequence element"));
        }
        Ok(result)
    }
}
struct Bytes<'a, E> {
    bytes: &'a [u8],
    marker: PhantomData<E>,
}
impl<'de, E: de::Error> de::SeqAccess<'de> for Bytes<'_, E> {
    type Error = E;
    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, E> {
        let Some((&byte, rest)) = self.bytes.split_first() else {
            return Ok(None);
        };
        self.bytes = rest;
        seed.deserialize(Positive(byte, PhantomData)).map(Some)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.bytes.len())
    }
}
