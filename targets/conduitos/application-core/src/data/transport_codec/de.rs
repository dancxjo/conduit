//! Verified sequence extents become exact Serde allocation hints.
use core::{fmt, marker::PhantomData};
use serde::{Deserialize, Deserializer, de::*};

pub(super) struct EnvelopeSeed<T>(PhantomData<T>);
impl<T> EnvelopeSeed<T> {
    pub(super) fn new() -> Self {
        Self(PhantomData)
    }
}
impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for EnvelopeSeed<T> {
    type Value = T;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<T, D::Error> {
        deserializer.deserialize_tuple(3, EnvelopeVisitor(PhantomData))
    }
}
struct EnvelopeVisitor<T>(PhantomData<T>);
impl<'de, T: Deserialize<'de>> Visitor<'de> for EnvelopeVisitor<T> {
    type Value = T;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("private application transport envelope")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<T, A::Error> {
        if seq.next_element::<usize>()? != Some(2)
            || seq.next_element::<&str>()? != Some(super::VERSION)
        {
            return Err(A::Error::custom("private transport version differs"));
        }
        let value = seq
            .next_element_seed(ValueSeed::<T>(PhantomData))?
            .ok_or_else(|| A::Error::custom("missing private payload"))?;
        if seq.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("trailing private envelope"));
        }
        Ok(value)
    }
}
struct ValueSeed<T>(PhantomData<T>);
impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for ValueSeed<T> {
    type Value = T;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<T, D::Error> {
        T::deserialize(Decoder(deserializer))
    }
}
struct Seed<S>(S);
impl<'de, S: DeserializeSeed<'de>> DeserializeSeed<'de> for Seed<S> {
    type Value = S::Value;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        self.0.deserialize(Decoder(deserializer))
    }
}
struct Decoder<D>(D);
struct Bridge<V> {
    visitor: V,
    expected: Option<usize>,
}
impl<V> Bridge<V> {
    fn new(visitor: V) -> Self {
        Self {
            visitor,
            expected: None,
        }
    }
}
macro_rules! forward {($($method:ident($($arg:ident:$ty:ty),*)),*$(,)?)=>{$(
    fn $method<V:Visitor<'de>>(self,$($arg:$ty,)*visitor:V)->Result<V::Value,Self::Error>{self.0.$method($($arg,)*Bridge::new(visitor))}
)*};}
impl<'de, D: Deserializer<'de>> Deserializer<'de> for Decoder<D> {
    type Error = D::Error;
    forward!(deserialize_any(),deserialize_bool(),deserialize_i8(),deserialize_i16(),deserialize_i32(),deserialize_i64(),deserialize_i128(),deserialize_u8(),deserialize_u16(),deserialize_u32(),deserialize_u64(),deserialize_u128(),deserialize_f32(),deserialize_f64(),deserialize_char(),deserialize_str(),deserialize_string(),deserialize_option(),deserialize_unit(),deserialize_unit_struct(name:&'static str),deserialize_newtype_struct(name:&'static str),deserialize_seq(),deserialize_map(),deserialize_struct(name:&'static str,fields:&'static [&'static str]),deserialize_enum(name:&'static str,variants:&'static [&'static str]),deserialize_identifier(),deserialize_ignored_any());
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.0.deserialize_seq(Bridge::new(visitor))
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.0.deserialize_seq(Bridge::new(visitor))
    }
    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        length: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_tuple(
            length
                .checked_add(1)
                .ok_or_else(|| D::Error::custom("tuple length overflow"))?,
            Bridge {
                visitor,
                expected: Some(length),
            },
        )
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        length: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_tuple_struct(
            name,
            length
                .checked_add(1)
                .ok_or_else(|| D::Error::custom("tuple length overflow"))?,
            Bridge {
                visitor,
                expected: Some(length),
            },
        )
    }
    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}
macro_rules! visit_scalar {($($method:ident($ty:ty)),*$(,)?)=>{$(
    fn $method<E:Error>(self,value:$ty)->Result<Self::Value,E>{self.visitor.$method(value)}
)*};}
impl<'de, V: Visitor<'de>> Visitor<'de> for Bridge<V> {
    type Value = V::Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.visitor.expecting(f)
    }
    visit_scalar!(
        visit_bool(bool),
        visit_i8(i8),
        visit_i16(i16),
        visit_i32(i32),
        visit_i64(i64),
        visit_i128(i128),
        visit_u8(u8),
        visit_u16(u16),
        visit_u32(u32),
        visit_u64(u64),
        visit_u128(u128),
        visit_f32(f32),
        visit_f64(f64),
        visit_char(char),
        visit_str(&str),
        visit_borrowed_str(&'de str),
        visit_string(alloc::string::String),
        visit_bytes(&[u8]),
        visit_borrowed_bytes(&'de [u8]),
        visit_byte_buf(alloc::vec::Vec<u8>)
    );
    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        self.visitor.visit_none()
    }
    fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
        self.visitor.visit_unit()
    }
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        self.visitor.visit_some(Decoder(deserializer))
    }
    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        self.visitor.visit_newtype_struct(Decoder(deserializer))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let remaining = access
            .next_element::<usize>()?
            .ok_or_else(|| A::Error::custom("missing array length"))?;
        if self.expected.is_some_and(|expected| expected != remaining) {
            return Err(A::Error::custom("tuple extent differs"));
        }
        let mut sequence = Sequence { access, remaining };
        let result = self.visitor.visit_seq(&mut sequence)?;
        if sequence.remaining != 0 || sequence.access.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom("array extent differs"));
        }
        Ok(result)
    }
    fn visit_map<A: MapAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        self.visitor.visit_map(Map(access))
    }
    fn visit_enum<A: EnumAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        self.visitor.visit_enum(Enum(access))
    }
}
struct Sequence<A> {
    access: A,
    remaining: usize,
}
impl<'de, A: SeqAccess<'de>> SeqAccess<'de> for &mut Sequence<A> {
    type Error = A::Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        if self.remaining == 0 {
            return Ok(None);
        }
        let value = self
            .access
            .next_element_seed(Seed(seed))?
            .ok_or_else(|| A::Error::custom("truncated array"))?;
        self.remaining -= 1;
        Ok(Some(value))
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}
struct Map<A>(A);
impl<'de, A: MapAccess<'de>> MapAccess<'de> for Map<A> {
    type Error = A::Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        self.0.next_key_seed(Seed(seed))
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.next_value_seed(Seed(seed))
    }
    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}
struct Enum<A>(A);
impl<'de, A: EnumAccess<'de>> EnumAccess<'de> for Enum<A> {
    type Error = A::Error;
    type Variant = Variant<A::Variant>;
    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error> {
        let (value, variant) = self.0.variant_seed(Seed(seed))?;
        Ok((value, Variant(variant)))
    }
}
struct Variant<A>(A);
impl<'de, A: VariantAccess<'de>> VariantAccess<'de> for Variant<A> {
    type Error = A::Error;
    fn unit_variant(self) -> Result<(), Self::Error> {
        self.0.unit_variant()
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Self::Error> {
        self.0.newtype_variant_seed(Seed(seed))
    }
    fn tuple_variant<V: Visitor<'de>>(
        self,
        length: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.tuple_variant(
            length
                .checked_add(1)
                .ok_or_else(|| A::Error::custom("tuple variant length overflow"))?,
            Bridge {
                visitor,
                expected: Some(length),
            },
        )
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.struct_variant(fields, Bridge::new(visitor))
    }
}
