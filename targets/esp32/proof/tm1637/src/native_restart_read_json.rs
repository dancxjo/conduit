//! Owned JSON decoding from authenticated bounded flash input. The reader owns
//! authentication; this decoder retains only one token and 128 input bytes.
use alloc::{string::String, vec::Vec};
use core::fmt;
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};

#[cfg(test)]
#[path = "native_restart_read_json_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("restart JSON refused")
    }
}
impl de::StdError for Error {}
impl de::Error for Error {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Self
    }
}
fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}
struct Token(Vec<u8>);
impl Token {
    fn push(&mut self, byte: u8, limit: usize) -> Result<(), Error> {
        if self.0.len() >= limit {
            return Err(Error);
        }
        if self.0.len() == self.0.capacity() {
            let capacity = self.0.capacity().saturating_mul(2).max(32).min(limit);
            let mut next = Vec::new();
            next.try_reserve_exact(capacity).map_err(|_| Error)?;
            next.extend_from_slice(&self.0);
            wipe(&mut self.0);
            self.0 = next;
        }
        self.0.push(byte);
        Ok(())
    }
}
impl Drop for Token {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}
struct Decoder<R> {
    read: R,
    input: [u8; 128],
    position: usize,
    available: usize,
    total: usize,
    limit: usize,
    eof: bool,
    depth: u16,
}
impl<R> Drop for Decoder<R> {
    fn drop(&mut self) {
        wipe(&mut self.input);
    }
}
impl<R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> Decoder<R> {
    fn peek(&mut self) -> Result<Option<u8>, Error> {
        if self.position == self.available && !self.eof {
            wipe(&mut self.input);
            let count = (self.read)(&mut self.input).map_err(|_| Error)?;
            if count > self.input.len() {
                return Err(Error);
            }
            self.total = self.total.checked_add(count).ok_or(Error)?;
            if self.total > self.limit {
                return Err(Error);
            }
            self.position = 0;
            self.available = count;
            self.eof = count == 0;
        }
        Ok((self.position < self.available).then(|| self.input[self.position]))
    }
    fn next(&mut self) -> Result<Option<u8>, Error> {
        let byte = self.peek()?;
        if byte.is_some() {
            self.position += 1;
        }
        Ok(byte)
    }
    fn whitespace(&mut self) -> Result<Option<u8>, Error> {
        loop {
            match self.peek()? {
                Some(b' ' | b'\t' | b'\r' | b'\n') => {
                    self.next()?;
                }
                byte => return Ok(byte),
            }
        }
    }
    fn expect(&mut self, byte: u8) -> Result<(), Error> {
        if self.next()? == Some(byte) {
            Ok(())
        } else {
            Err(Error)
        }
    }
    fn enter(&mut self) -> Result<(), Error> {
        // serde_json starts with 128 and refuses when decrement reaches zero.
        if self.depth >= 127 {
            return Err(Error);
        }
        self.depth += 1;
        Ok(())
    }
    fn token(&mut self) -> Result<Token, Error> {
        let first = self.whitespace()?.ok_or(Error)?;
        let mut token = Token(Vec::new());
        token.push(self.next()?.ok_or(Error)?, self.limit)?;
        if first == b'"' {
            let mut escaped = false;
            loop {
                let byte = self.next()?.ok_or(Error)?;
                token.push(byte, self.limit)?;
                if byte == b'"' && !escaped {
                    return Ok(token);
                }
                escaped = byte == b'\\' && !escaped;
            }
        }
        while let Some(byte) = self.peek()? {
            if matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}') {
                break;
            }
            token.push(self.next()?.ok_or(Error)?, self.limit)?;
        }
        Ok(token)
    }
    fn scalar<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        let token = self.token()?;
        serde_json::from_slice(&token.0).map_err(|_| Error)
    }
    /// Validate an ignored value with bounded container state rather than one
    /// Rust/Serde frame for every archived array and object. Scalar dispatch is
    /// unchanged, including numeric extent and string/escape validation.
    #[inline(never)]
    fn skip_value(&mut self) -> Result<(), Error> {
        const SEQUENCE_FIRST: u8 = 0;
        const SEQUENCE_AFTER: u8 = 1;
        const MAP_FIRST: u8 = 2;
        const MAP_AFTER: u8 = 3;
        let original_depth = self.depth;
        let result = (|| {
            let mut containers = [SEQUENCE_FIRST; 127];
            let mut length = 0usize;
            let mut value = true;
            loop {
                if value {
                    match self.whitespace()?.ok_or(Error)? {
                        b'[' => {
                            self.expect(b'[')?;
                            self.enter()?;
                            *containers.get_mut(length).ok_or(Error)? = SEQUENCE_FIRST;
                            length += 1;
                        }
                        b'{' => {
                            self.expect(b'{')?;
                            self.enter()?;
                            *containers.get_mut(length).ok_or(Error)? = MAP_FIRST;
                            length += 1;
                        }
                        _ => self.skip_scalar()?,
                    }
                    value = false;
                    if length == 0 {
                        return Ok(());
                    }
                }
                let state = containers[length - 1];
                let end = if state <= SEQUENCE_AFTER { b']' } else { b'}' };
                if self.whitespace()? == Some(end) {
                    self.expect(end)?;
                    self.depth -= 1;
                    length -= 1;
                    if length == 0 {
                        return Ok(());
                    }
                    continue;
                }
                if matches!(state, SEQUENCE_AFTER | MAP_AFTER) {
                    self.expect(b',')?;
                    if self.whitespace()? == Some(end) {
                        return Err(Error);
                    }
                }
                if state >= MAP_FIRST {
                    if self.whitespace()? != Some(b'"') {
                        return Err(Error);
                    }
                    let _: String = self.scalar()?;
                    self.whitespace()?;
                    self.expect(b':')?;
                    containers[length - 1] = MAP_AFTER;
                } else {
                    containers[length - 1] = SEQUENCE_AFTER;
                }
                value = true;
            }
        })();
        // Recursive sequence/mapping visitors likewise unwind their depth on
        // refusal. Keep the same decoder state if a caller handles that error.
        if result.is_err() {
            self.depth = original_depth;
        }
        result
    }

    fn skip_scalar(&mut self) -> Result<(), Error> {
        struct Ignore;
        impl<'de> Visitor<'de> for Ignore {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON scalar")
            }
            fn visit_unit<E: de::Error>(self) -> Result<(), E> {
                Ok(())
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
                Ok(())
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
                Ok(())
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
                Ok(())
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
                Ok(())
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
                Ok(())
            }
            fn visit_string<E: de::Error>(self, _: String) -> Result<(), E> {
                Ok(())
            }
        }
        de::Deserializer::deserialize_any(&mut *self, Ignore)
    }
    fn sequence<'de, V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value, Error> {
        if self.whitespace()? != Some(b'[') {
            return Err(Error);
        }
        self.expect(b'[')?;
        self.enter()?;
        let result = visitor.visit_seq(Sequence {
            decoder: self,
            first: true,
        });
        self.depth -= 1;
        let value = result?;
        self.whitespace()?;
        self.expect(b']')?;
        Ok(value)
    }
    fn mapping<'de, V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value, Error> {
        if self.whitespace()? != Some(b'{') {
            return Err(Error);
        }
        self.expect(b'{')?;
        self.enter()?;
        let result = visitor.visit_map(Mapping {
            decoder: self,
            first: true,
            value_pending: false,
        });
        self.depth -= 1;
        let value = result?;
        self.whitespace()?;
        self.expect(b'}')?;
        Ok(value)
    }
}
pub fn from_reader<T: DeserializeOwned>(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    limit: usize,
) -> Result<T, Error> {
    struct Owned<T>(core::marker::PhantomData<T>);
    impl<'de, T: DeserializeOwned> DeserializeSeed<'de> for Owned<T> {
        type Value = T;
        fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<T, D::Error> {
            T::deserialize(decoder)
        }
    }
    from_reader_seed(read, limit, Owned(core::marker::PhantomData))
}
/// Same bounded reader/EOF contract with caller-owned validation context.
pub(crate) fn from_reader_seed<T, S>(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    limit: usize,
    seed: S,
) -> Result<T, Error>
where
    S: for<'de> DeserializeSeed<'de, Value = T>,
{
    let mut decoder = Decoder {
        read,
        input: [0; 128],
        position: 0,
        available: 0,
        total: 0,
        limit,
        eof: false,
        depth: 0,
    };
    let value = seed.deserialize(&mut decoder)?;
    if decoder.whitespace()?.is_some() {
        return Err(Error);
    }
    Ok(value)
}
struct Sequence<'a, R> {
    decoder: &'a mut Decoder<R>,
    first: bool,
}
impl<'de, R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> SeqAccess<'de> for Sequence<'_, R> {
    type Error = Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        if self.decoder.whitespace()? == Some(b']') {
            return Ok(None);
        }
        if !self.first {
            self.decoder.expect(b',')?;
            if self.decoder.whitespace()? == Some(b']') {
                return Err(Error);
            }
        }
        self.first = false;
        seed.deserialize(&mut *self.decoder).map(Some)
    }
}
struct Mapping<'a, R> {
    decoder: &'a mut Decoder<R>,
    first: bool,
    value_pending: bool,
}
impl<'de, R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> MapAccess<'de> for Mapping<'_, R> {
    type Error = Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        if self.value_pending {
            return Err(Error);
        }
        if self.decoder.whitespace()? == Some(b'}') {
            return Ok(None);
        }
        if !self.first {
            self.decoder.expect(b',')?;
            if self.decoder.whitespace()? == Some(b'}') {
                return Err(Error);
            }
        }
        self.first = false;
        if self.decoder.whitespace()? != Some(b'"') {
            return Err(Error);
        }
        let raw = self.decoder.token()?;
        let text: String = serde_json::from_slice(&raw.0).map_err(|_| Error)?;
        self.decoder.whitespace()?;
        self.decoder.expect(b':')?;
        self.value_pending = true;
        seed.deserialize(Key { text, raw }).map(Some)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        if !self.value_pending {
            return Err(Error);
        }
        self.value_pending = false;
        seed.deserialize(&mut *self.decoder)
    }
}
struct ScalarVisitor<'de, V>(V, core::marker::PhantomData<&'de ()>);
macro_rules! scalar_visit {
    ($method:ident, $ty:ty) => {
        fn $method<E: de::Error>(self, value: $ty) -> Result<Self::Value, E> {
            self.0.$method::<Error>(value).map_err(E::custom)
        }
    };
}
impl<'input, 'de, V: Visitor<'de>> Visitor<'input> for ScalarVisitor<'de, V> {
    type Value = V::Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.expecting(f)
    }
    scalar_visit!(visit_bool, bool);
    scalar_visit!(visit_i64, i64);
    scalar_visit!(visit_u64, u64);
    scalar_visit!(visit_i128, i128);
    scalar_visit!(visit_u128, u128);
    scalar_visit!(visit_f64, f64);
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        self.0.visit_unit::<Error>().map_err(E::custom)
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        self.0
            .visit_string::<Error>(String::from(value))
            .map_err(E::custom)
    }
    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
        self.0
            .visit_byte_buf::<Error>(Vec::from(value))
            .map_err(E::custom)
    }
}
macro_rules! numeric {
    ($method:ident, $ty:ty, $visit:ident) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            let token = self.token()?;
            let mut decoder = serde_json::Deserializer::from_slice(&token.0);
            let result = de::Deserializer::$method(
                &mut decoder,
                ScalarVisitor(visitor, core::marker::PhantomData),
            )
            .map_err(|_| Error)?;
            decoder.end().map_err(|_| Error)?;
            Ok(result)
        }
    };
}
impl<'de, R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> de::Deserializer<'de> for &mut Decoder<R> {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.whitespace()?.ok_or(Error)? {
            b'[' => self.sequence(visitor),
            b'{' => self.mapping(visitor),
            b'"' => visitor.visit_string(self.scalar::<String>()?),
            _ => {
                // Delegate primitive syntax/numeric dispatch directly. The
                // owned bridge prevents a token borrow escaping and avoids
                // linking a second recursive JSON Value materializer.
                let token = self.token()?;
                let mut decoder = serde_json::Deserializer::from_slice(&token.0);
                let value = de::Deserializer::deserialize_any(
                    &mut decoder,
                    ScalarVisitor(visitor, core::marker::PhantomData),
                )
                .map_err(|_| Error)?;
                decoder.end().map_err(|_| Error)?;
                Ok(value)
            }
        }
    }
    numeric!(deserialize_bool, bool, visit_bool);
    numeric!(deserialize_i8, i8, visit_i8);
    numeric!(deserialize_i16, i16, visit_i16);
    numeric!(deserialize_i32, i32, visit_i32);
    numeric!(deserialize_i64, i64, visit_i64);
    numeric!(deserialize_i128, i128, visit_i128);
    numeric!(deserialize_u8, u8, visit_u8);
    numeric!(deserialize_u16, u16, visit_u16);
    numeric!(deserialize_u32, u32, visit_u32);
    numeric!(deserialize_u64, u64, visit_u64);
    numeric!(deserialize_u128, u128, visit_u128);
    numeric!(deserialize_f32, f32, visit_f32);
    numeric!(deserialize_f64, f64, visit_f64);
    numeric!(deserialize_char, char, visit_char);
    numeric!(deserialize_str, String, visit_string);
    numeric!(deserialize_string, String, visit_string);
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.whitespace()? == Some(b'"') {
            let token = self.token()?;
            let mut decoder = serde_json::Deserializer::from_slice(&token.0);
            let value = de::Deserializer::deserialize_bytes(
                &mut decoder,
                ScalarVisitor(visitor, core::marker::PhantomData),
            )
            .map_err(|_| Error)?;
            decoder.end().map_err(|_| Error)?;
            Ok(value)
        } else {
            self.sequence(visitor)
        }
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_bytes(visitor)
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.whitespace()? == Some(b'n') {
            self.scalar::<()>()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.scalar::<()>()?;
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
        self.mapping(visitor)
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.whitespace()? {
            Some(b'[') => self.sequence(visitor),
            _ => self.mapping(visitor),
        }
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.whitespace()? {
            Some(b'"') => visitor.visit_enum(de::value::StringDeserializer::<Error>::new(
                self.scalar::<String>()?,
            )),
            Some(b'{') => {
                self.expect(b'{')?;
                self.enter()?;
                if self.whitespace()? != Some(b'"') {
                    return Err(Error);
                }
                let name = self.scalar::<String>()?;
                self.whitespace()?;
                self.expect(b':')?;
                let result = visitor.visit_enum(External {
                    decoder: self,
                    name,
                });
                self.depth -= 1;
                let value = result?;
                self.whitespace()?;
                self.expect(b'}')?;
                Ok(value)
            }
            _ => Err(Error),
        }
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.skip_value()?;
        visitor.visit_unit()
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
struct External<'a, R> {
    decoder: &'a mut Decoder<R>,
    name: String,
}
struct Variant<'a, R> {
    decoder: &'a mut Decoder<R>,
}
impl<'de, 'a, R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> EnumAccess<'de> for External<'a, R> {
    type Error = Error;
    type Variant = Variant<'a, R>;
    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let name = seed.deserialize(de::value::StringDeserializer::<Error>::new(self.name))?;
        Ok((
            name,
            Variant {
                decoder: self.decoder,
            },
        ))
    }
}
impl<'de, R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> VariantAccess<'de> for Variant<'_, R> {
    type Error = Error;
    fn unit_variant(self) -> Result<(), Error> {
        de::Deserialize::deserialize(self.decoder)
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(self.decoder)
    }
    fn tuple_variant<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Error> {
        self.decoder.sequence(visitor)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.decoder.mapping(visitor)
    }
}
struct Key {
    text: String,
    raw: Token,
}
macro_rules! key_numeric {
    ($method:ident, $ty:ty, $visit:ident) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            // JSON numeric keys are parsed inside the original quotes: escaped
            // digits and whitespace cannot become accepted numeric spellings.
            let bytes = &self.raw.0[1..self.raw.0.len() - 1];
            if bytes.is_empty()
                || bytes
                    .iter()
                    .any(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
            {
                return Err(Error);
            }
            let mut decoder = serde_json::Deserializer::from_slice(bytes);
            let value = de::Deserializer::$method(
                &mut decoder,
                ScalarVisitor(visitor, core::marker::PhantomData),
            )
            .map_err(|_| Error)?;
            decoder.end().map_err(|_| Error)?;
            Ok(value)
        }
    };
}
impl<'de> de::Deserializer<'de> for Key {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(self.text)
    }
    key_numeric!(deserialize_bool, bool, visit_bool);
    key_numeric!(deserialize_i8, i8, visit_i8);
    key_numeric!(deserialize_i16, i16, visit_i16);
    key_numeric!(deserialize_i32, i32, visit_i32);
    key_numeric!(deserialize_i64, i64, visit_i64);
    key_numeric!(deserialize_i128, i128, visit_i128);
    key_numeric!(deserialize_u8, u8, visit_u8);
    key_numeric!(deserialize_u16, u16, visit_u16);
    key_numeric!(deserialize_u32, u32, visit_u32);
    key_numeric!(deserialize_u64, u64, visit_u64);
    key_numeric!(deserialize_u128, u128, visit_u128);
    key_numeric!(deserialize_f32, f32, visit_f32);
    key_numeric!(deserialize_f64, f64, visit_f64);
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_some(self)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_enum(de::value::StringDeserializer::<Error>::new(self.text))
    }
    serde::forward_to_deserialize_any! { char str string bytes byte_buf unit unit_struct seq tuple tuple_struct map struct identifier ignored_any }
}
