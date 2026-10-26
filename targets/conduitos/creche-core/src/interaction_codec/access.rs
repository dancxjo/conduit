//! Every seed re-enters the adapter, including enum payloads and Content visitors.
use super::decode::Sequences;
use serde::de::{self, DeserializeSeed, Deserializer, Visitor};
pub(super) struct Seed<T> {
    pub inner: T,
    pub depth: u8,
}
impl<'de, T: DeserializeSeed<'de>> DeserializeSeed<'de> for Seed<T> {
    type Value = T::Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.inner.deserialize(Sequences {
            inner: d,
            depth: self.depth,
        })
    }
}
pub(super) struct Seq<A> {
    pub inner: A,
    pub depth: u8,
    pub count: usize,
    pub ended: bool,
}
impl<'de, A: de::SeqAccess<'de>> de::SeqAccess<'de> for Seq<A> {
    type Error = A::Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, A::Error> {
        if self.ended {
            return Ok(None);
        }
        let value = self.inner.next_element_seed(Seed {
            inner: seed,
            depth: self.depth,
        })?;
        self.ended = value.is_none();
        if value.is_some() {
            self.count += 1;
        }
        Ok(value)
    }
    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}
pub(super) struct Map<A> {
    pub inner: A,
    pub depth: u8,
}
impl<'de, A: de::MapAccess<'de>> de::MapAccess<'de> for Map<A> {
    type Error = A::Error;
    fn next_key_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, A::Error> {
        self.inner.next_key_seed(Seed {
            inner: seed,
            depth: self.depth,
        })
    }
    fn next_value_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<T::Value, A::Error> {
        self.inner.next_value_seed(Seed {
            inner: seed,
            depth: self.depth,
        })
    }
    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}
pub(super) struct Enum<A> {
    pub inner: A,
    pub depth: u8,
}
impl<'de, A: de::EnumAccess<'de>> de::EnumAccess<'de> for Enum<A> {
    type Error = A::Error;
    type Variant = Variant<A::Variant>;
    fn variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<(T::Value, Self::Variant), A::Error> {
        let (value, inner) = self.inner.variant_seed(Seed {
            inner: seed,
            depth: self.depth,
        })?;
        Ok((
            value,
            Variant {
                inner,
                depth: self.depth,
            },
        ))
    }
}
pub(super) struct Variant<A> {
    inner: A,
    depth: u8,
}
impl<'de, A: de::VariantAccess<'de>> de::VariantAccess<'de> for Variant<A> {
    type Error = A::Error;
    fn unit_variant(self) -> Result<(), A::Error> {
        self.inner.unit_variant()
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, A::Error> {
        self.inner.newtype_variant_seed(Seed {
            inner: seed,
            depth: self.depth,
        })
    }
    // Re-enter through a seed so native VariantAccess cannot bypass the
    // packed sequence's numeric dispatch, depth accounting, or tuple width.
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, A::Error> {
        self.inner.newtype_variant_seed(Tuple {
            visitor,
            len,
            depth: self.depth,
        })
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, A::Error> {
        self.inner.newtype_variant_seed(Struct {
            visitor,
            fields,
            depth: self.depth,
        })
    }
}
struct Tuple<V> {
    visitor: V,
    len: usize,
    depth: u8,
}
impl<'de, V: Visitor<'de>> DeserializeSeed<'de> for Tuple<V> {
    type Value = V::Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        Sequences {
            inner: d,
            depth: self.depth,
        }
        .deserialize_tuple(self.len, self.visitor)
    }
}
struct Struct<V> {
    visitor: V,
    fields: &'static [&'static str],
    depth: u8,
}
impl<'de, V: Visitor<'de>> DeserializeSeed<'de> for Struct<V> {
    type Value = V::Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        Sequences {
            inner: d,
            depth: self.depth,
        }
        .deserialize_struct("", self.fields, self.visitor)
    }
}
