//! Definite, preflighted CBOR into the ordinary Serde models. No value tree,
//! scratch string, indefinite container, or alternate canonical constructor.
use core::fmt;
use serde::{
    Deserialize,
    de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor},
};
#[derive(Debug, Clone, Copy)]
pub(super) struct Error;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("malformed private binary command")
    }
}
impl de::StdError for Error {}
impl de::Error for Error {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Self
    }
}
pub(super) fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, Error> {
    let mut decoder = Decoder {
        bytes,
        at: 0,
        depth: 0,
    };
    let value = T::deserialize(&mut decoder)?;
    if decoder.at != bytes.len() {
        return Err(Error);
    }
    Ok(value)
}
struct Decoder<'de> {
    bytes: &'de [u8],
    at: usize,
    depth: usize,
}
impl<'de> Decoder<'de> {
    fn take(&mut self, n: usize) -> Result<&'de [u8], Error> {
        let end = self.at.checked_add(n).ok_or(Error)?;
        let bytes = self.bytes.get(self.at..end).ok_or(Error)?;
        self.at = end;
        Ok(bytes)
    }
    fn peek(&self) -> Result<u8, Error> {
        self.bytes.get(self.at).copied().ok_or(Error)
    }
    fn header(&mut self) -> Result<(u8, u64, u8), Error> {
        let byte = self.take(1)?[0];
        let n = match byte & 31 {
            n @ 0..=23 => u64::from(n),
            24 => u64::from(self.take(1)?[0]),
            25 => u64::from(u16::from_be_bytes(self.take(2)?.try_into().unwrap())),
            26 => u64::from(u32::from_be_bytes(self.take(4)?.try_into().unwrap())),
            27 => u64::from_be_bytes(self.take(8)?.try_into().unwrap()),
            _ => return Err(Error),
        };
        Ok((byte >> 5, n, byte & 31))
    }
    fn text(&mut self) -> Result<&'de str, Error> {
        let (major, n, _) = self.header()?;
        if major != 3 {
            return Err(Error);
        }
        core::str::from_utf8(self.take(usize::try_from(n).map_err(|_| Error)?)?).map_err(|_| Error)
    }
    fn integer<V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value, Error> {
        let (major, n, _) = self.header()?;
        let (negative, magnitude) = match major {
            0 => (false, u128::from(n)),
            1 => (true, u128::from(n)),
            6 if matches!(n, 2 | 3) => {
                let (bytes, len, _) = self.header()?;
                if bytes != 2 || len > 16 {
                    return Err(Error);
                }
                let mut magnitude = 0u128;
                for byte in self.take(len as usize)? {
                    magnitude = (magnitude << 8) | u128::from(*byte);
                }
                (n == 3, magnitude)
            }
            _ => return Err(Error),
        };
        if negative {
            let magnitude = i128::try_from(magnitude).map_err(|_| Error)?;
            let n = -1 - magnitude;
            if let Ok(n) = i64::try_from(n) {
                visitor.visit_i64(n)
            } else {
                visitor.visit_i128(n)
            }
        } else if let Ok(n) = u64::try_from(magnitude) {
            visitor.visit_u64(n)
        } else {
            visitor.visit_u128(magnitude)
        }
    }
    fn sequence<V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value, Error> {
        let (major, n, _) = self.header()?;
        if !matches!(major, 2 | 4) || (major == 4 && self.depth >= 64) {
            return Err(Error);
        }
        let count = usize::try_from(n).map_err(|_| Error)?;
        if count > self.bytes.len() - self.at {
            return Err(Error);
        }
        let nested = usize::from(major == 4);
        self.depth += nested;
        let mut access = Sequence {
            decoder: self,
            remaining: count,
            bytes: major == 2,
        };
        let result = visitor.visit_seq(&mut access);
        let complete = access.remaining == 0;
        self.depth -= nested;
        if !complete {
            return Err(Error);
        }
        result
    }
    fn map<V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value, Error> {
        let (major, n, _) = self.header()?;
        if major != 5 || self.depth >= 64 {
            return Err(Error);
        }
        let count = usize::try_from(n).map_err(|_| Error)?;
        if count > (self.bytes.len() - self.at) / 2 {
            return Err(Error);
        }
        self.depth += 1;
        let mut access = Mapping {
            decoder: self,
            remaining: count,
            pending: false,
        };
        let result = visitor.visit_map(&mut access);
        let complete = access.remaining == 0 && !access.pending;
        self.depth -= 1;
        if !complete {
            return Err(Error);
        }
        result
    }
}
struct Sequence<'a, 'de> {
    decoder: &'a mut Decoder<'de>,
    remaining: usize,
    bytes: bool,
}
impl<'de> SeqAccess<'de> for &mut Sequence<'_, 'de> {
    type Error = Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        let value = if self.bytes {
            seed.deserialize(de::value::U8Deserializer::<Error>::new(
                self.decoder.take(1)?[0],
            ))?
        } else {
            seed.deserialize(&mut *self.decoder)?
        };
        Ok(Some(value))
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}
struct Mapping<'a, 'de> {
    decoder: &'a mut Decoder<'de>,
    remaining: usize,
    pending: bool,
}
impl<'de> MapAccess<'de> for &mut Mapping<'_, 'de> {
    type Error = Error;
    fn next_key_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        if self.pending {
            return Err(Error);
        }
        if self.remaining == 0 {
            return Ok(None);
        }
        self.pending = true;
        seed.deserialize(&mut *self.decoder).map(Some)
    }
    fn next_value_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<T::Value, Error> {
        if !self.pending {
            return Err(Error);
        }
        self.pending = false;
        self.remaining -= 1;
        seed.deserialize(&mut *self.decoder)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}
macro_rules! integers {($($name:ident),*) => {$(fn $name<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> { self.integer(visitor) })*};}
macro_rules! strings {($($name:ident),*) => {$(fn $name<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> { visitor.visit_borrowed_str(self.text()?) })*};}
impl<'de> de::Deserializer<'de> for &mut Decoder<'de> {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.peek()? >> 5 {
            0 | 1 | 6 => self.integer(visitor),
            2 => self.deserialize_bytes(visitor),
            3 => self.deserialize_str(visitor),
            4 => self.sequence(visitor),
            5 => self.map(visitor),
            7 => {
                let (_, n, width) = self.header()?;
                match width {
                    20 => visitor.visit_bool(false),
                    21 => visitor.visit_bool(true),
                    22 => visitor.visit_unit(),
                    25..=27 => {
                        let value = match width {
                            25 => half(n as u16) as f64,
                            26 => f32::from_bits(n as u32) as f64,
                            _ => f64::from_bits(n),
                        };
                        if !value.is_finite() {
                            return Err(Error);
                        }
                        visitor.visit_f64(value)
                    }
                    _ => Err(Error),
                }
            }
            _ => Err(Error),
        }
    }
    integers!(
        deserialize_i8,
        deserialize_i16,
        deserialize_i32,
        deserialize_i64,
        deserialize_i128,
        deserialize_u8,
        deserialize_u16,
        deserialize_u32,
        deserialize_u64,
        deserialize_u128
    );
    strings!(deserialize_str, deserialize_string, deserialize_identifier);
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.take(1)?[0] {
            0xf4 => visitor.visit_bool(false),
            0xf5 => visitor.visit_bool(true),
            _ => Err(Error),
        }
    }
    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let mut chars = self.text()?.chars();
        let ch = chars.next().ok_or(Error)?;
        if chars.next().is_some() {
            return Err(Error);
        }
        visitor.visit_char(ch)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.peek()? >> 5 == 4 {
            return self.sequence(visitor);
        }
        let (major, n, _) = self.header()?;
        if major != 2 {
            return Err(Error);
        }
        visitor.visit_borrowed_bytes(self.take(usize::try_from(n).map_err(|_| Error)?)?)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_bytes(visitor)
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.peek()? == 0xf6 {
            self.at += 1;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.take(1)?[0] != 0xf6 {
            return Err(Error);
        }
        visitor.visit_unit()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_unit(visitor)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.sequence(visitor)
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Error> {
        self.sequence(visitor)
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.sequence(visitor)
    }
    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.map(visitor)
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        if self.peek()? >> 5 == 4 {
            self.sequence(visitor)
        } else {
            self.map(visitor)
        }
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        if self.peek()? >> 5 == 3 {
            return visitor.visit_enum(de::value::BorrowedStrDeserializer::<Error>::new(
                self.text()?,
            ));
        }
        let (major, count, _) = self.header()?;
        if major != 5 || count != 1 || self.depth >= 64 {
            return Err(Error);
        }
        self.depth += 1;
        let result = visitor.visit_enum(Variant(self));
        self.depth -= 1;
        result
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }
    fn is_human_readable(&self) -> bool {
        false
    }
}
struct Variant<'a, 'de>(&'a mut Decoder<'de>);
impl<'a, 'de> EnumAccess<'de> for Variant<'a, 'de> {
    type Error = Error;
    type Variant = Self;
    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), Error> {
        let variant = seed.deserialize(&mut *self.0)?;
        Ok((variant, self))
    }
}
impl<'de> VariantAccess<'de> for Variant<'_, 'de> {
    type Error = Error;
    fn unit_variant(self) -> Result<(), Error> {
        Deserialize::deserialize(&mut *self.0)
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(self.0)
    }
    fn tuple_variant<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Error> {
        self.0.sequence(visitor)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.0.map(visitor)
    }
}
// Exact IEEE binary16 -> binary32, including subnormals. Preflight rejects
// nonfinite values; no floating arithmetic or allocation is needed to expand.
fn half(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 31;
    let fraction = u32::from(bits & 1023);
    let value = if exponent == 0 {
        if fraction == 0 {
            sign
        } else {
            let shift = fraction.leading_zeros() - 21;
            sign | ((113 - shift) << 23) | ((fraction << shift) & 1023) << 13
        }
    } else if exponent == 31 {
        sign | 0x7f80_0000 | (fraction << 13)
    } else {
        sign | (u32::from(exponent + 112) << 23) | (fraction << 13)
    };
    f32::from_bits(value)
}

#[cfg(test)]
mod tests;
