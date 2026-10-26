//! Serialize-only compact JSON stream. Canonical derives supply every field and
//! omission. Numeric leaf spelling uses serde_json; strings use its JSON escape
//! rules without allocating a whole scalar/document. No ingress or authority.
use super::{CodecError, JsonSink};
use serde::{
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant,
    },
    Serialize, Serializer,
};

pub(super) struct Json<'a> {
    pub encoder: &'a mut dyn JsonSink,
    pub key: bool,
}
impl<'a> Json<'a> {
    fn primitive(&mut self, value: &impl Serialize) -> Result<(), CodecError> {
        let bytes = serde_json::to_vec(value).map_err(|_| CodecError::Invalid)?;
        if self.key {
            self.encoder.write(b"\"")?;
        }
        self.encoder.write(&bytes)?;
        if self.key {
            self.encoder.write(b"\"")?;
        }
        Ok(())
    }
    #[inline(never)]
    fn string(&mut self, value: &str) -> Result<(), CodecError> {
        self.encoder.write(b"\"")?;
        escape(self.encoder, value)?;
        self.encoder.write(b"\"")
    }
    #[inline(never)]
    fn compound(
        self,
        opening: &[u8],
        closing: &'static [u8],
        variant: bool,
    ) -> Result<Compound<'a>, CodecError> {
        if self.key {
            return Err(CodecError::Invalid);
        }
        self.encoder.write(opening)?;
        Ok(Compound {
            encoder: self.encoder,
            first: true,
            pending_key: false,
            closing,
            variant,
        })
    }
}
#[inline(never)]
fn escape(encoder: &mut dyn JsonSink, value: &str) -> Result<(), CodecError> {
    let bytes = value.as_bytes();
    let mut start = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        let escaped: Option<&[u8]> = match byte {
            b'"' => Some(b"\\\""),
            b'\\' => Some(b"\\\\"),
            b'\n' => Some(b"\\n"),
            b'\r' => Some(b"\\r"),
            b'\t' => Some(b"\\t"),
            8 => Some(b"\\b"),
            12 => Some(b"\\f"),
            0..=31 => Some(b""),
            _ => None,
        };
        if let Some(escaped) = escaped {
            encoder.write(&bytes[start..index])?;
            if escaped.is_empty() {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                encoder.write(&[
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[usize::from(byte >> 4)],
                    HEX[usize::from(byte & 15)],
                ])?;
            } else {
                encoder.write(escaped)?;
            }
            start = index + 1;
        }
    }
    encoder.write(&bytes[start..])
}
pub(super) struct Compound<'a> {
    encoder: &'a mut dyn JsonSink,
    first: bool,
    pending_key: bool,
    closing: &'static [u8],
    variant: bool,
}
impl Compound<'_> {
    #[inline(never)]
    fn comma(&mut self) -> Result<(), CodecError> {
        if self.first {
            self.first = false;
            Ok(())
        } else {
            self.encoder.write(b",")
        }
    }
    fn element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        self.comma()?;
        value.serialize(Json {
            encoder: self.encoder,
            key: false,
        })
    }
    #[inline(never)]
    fn field_prefix(&mut self, key: &'static str) -> Result<(), CodecError> {
        self.comma()?;
        Json {
            encoder: self.encoder,
            key: true,
        }
        .string(key)?;
        self.encoder.write(b":")
    }
    fn field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), CodecError> {
        self.field_prefix(key)?;
        value.serialize(Json {
            encoder: self.encoder,
            key: false,
        })
    }
    #[inline(never)]
    fn finish(self) -> Result<(), CodecError> {
        if self.pending_key {
            return Err(CodecError::Invalid);
        }
        self.encoder.write(self.closing)?;
        if self.variant {
            self.encoder.write(b"}")?;
        }
        Ok(())
    }
}
impl<'a> Serializer for Json<'a> {
    type Ok = ();
    type Error = CodecError;
    type SerializeSeq = Compound<'a>;
    type SerializeTuple = Compound<'a>;
    type SerializeTupleStruct = Compound<'a>;
    type SerializeTupleVariant = Compound<'a>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Compound<'a>;
    fn serialize_bool(mut self, value: bool) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_i8(mut self, value: i8) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_i16(mut self, value: i16) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_i32(mut self, value: i32) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_i64(mut self, value: i64) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_i128(mut self, value: i128) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_u8(mut self, value: u8) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_u16(mut self, value: u16) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_u32(mut self, value: u32) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_u64(mut self, value: u64) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_u128(mut self, value: u128) -> Result<(), CodecError> {
        self.primitive(&value)
    }
    fn serialize_f32(mut self, value: f32) -> Result<(), CodecError> {
        if self.key && !value.is_finite() {
            return Err(CodecError::Invalid);
        }
        self.primitive(&value)
    }
    fn serialize_f64(mut self, value: f64) -> Result<(), CodecError> {
        if self.key && !value.is_finite() {
            return Err(CodecError::Invalid);
        }
        self.primitive(&value)
    }
    fn serialize_str(mut self, value: &str) -> Result<(), CodecError> {
        self.string(value)
    }
    fn serialize_char(mut self, value: char) -> Result<(), CodecError> {
        self.string(value.encode_utf8(&mut [0; 4]))
    }
    fn serialize_bytes(self, bytes: &[u8]) -> Result<(), CodecError> {
        let mut seq = self.compound(b"[", b"]", false)?;
        for byte in bytes {
            seq.element(byte)?;
        }
        seq.finish()
    }
    fn serialize_none(self) -> Result<(), CodecError> {
        self.serialize_unit()
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), CodecError> {
        if self.key {
            return Err(CodecError::Invalid);
        }
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), CodecError> {
        if self.key {
            return Err(CodecError::Invalid);
        }
        self.encoder.write(b"null")
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), CodecError> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<(), CodecError> {
        self.serialize_str(variant)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), CodecError> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        mut self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), CodecError> {
        if self.key {
            return Err(CodecError::Invalid);
        }
        self.encoder.write(b"{")?;
        self.string(variant)?;
        self.encoder.write(b":")?;
        value.serialize(Json {
            encoder: self.encoder,
            key: false,
        })?;
        self.encoder.write(b"}")
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, CodecError> {
        self.compound(b"[", b"]", false)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, CodecError> {
        self.compound(b"[", b"]", false)
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, CodecError> {
        self.compound(b"[", b"]", false)
    }
    fn serialize_tuple_variant(
        mut self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, CodecError> {
        if self.key {
            return Err(CodecError::Invalid);
        }
        self.encoder.write(b"{")?;
        self.string(variant)?;
        self.encoder.write(b":")?;
        self.compound(b"[", b"]", true)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, CodecError> {
        self.compound(b"{", b"}", false)
    }
    fn serialize_struct(
        self,
        name: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStruct, CodecError> {
        // RawValue/arbitrary-precision serializer escape hatches require their
        // own exact implementation; never reinterpret them as ordinary fields.
        if name.starts_with("$serde_json::private::") {
            return Err(CodecError::Invalid);
        }
        self.compound(b"{", b"}", false)
    }
    fn serialize_struct_variant(
        mut self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, CodecError> {
        if self.key {
            return Err(CodecError::Invalid);
        }
        self.encoder.write(b"{")?;
        self.string(variant)?;
        self.encoder.write(b":")?;
        self.compound(b"{", b"}", true)
    }
    fn collect_str<T: ?Sized + core::fmt::Display>(self, value: &T) -> Result<(), CodecError> {
        self.encoder.write(b"\"")?;
        let mut writer = DisplayWriter {
            encoder: self.encoder,
            refusal: None,
        };
        if core::fmt::write(&mut writer, format_args!("{value}")).is_err() {
            return Err(writer.refusal.unwrap_or(CodecError::Invalid));
        }
        writer.encoder.write(b"\"")
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
struct DisplayWriter<'a> {
    encoder: &'a mut dyn JsonSink,
    refusal: Option<CodecError>,
}
impl core::fmt::Write for DisplayWriter<'_> {
    fn write_str(&mut self, value: &str) -> core::fmt::Result {
        escape(self.encoder, value).map_err(|error| {
            self.refusal = Some(error);
            core::fmt::Error
        })
    }
}
impl SerializeSeq for Compound<'_> {
    type Ok = ();
    type Error = CodecError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        self.element(value)
    }
    fn end(self) -> Result<(), CodecError> {
        self.finish()
    }
}
impl SerializeTuple for Compound<'_> {
    type Ok = ();
    type Error = CodecError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        self.element(value)
    }
    fn end(self) -> Result<(), CodecError> {
        self.finish()
    }
}
impl SerializeTupleStruct for Compound<'_> {
    type Ok = ();
    type Error = CodecError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        self.element(value)
    }
    fn end(self) -> Result<(), CodecError> {
        self.finish()
    }
}
impl SerializeTupleVariant for Compound<'_> {
    type Ok = ();
    type Error = CodecError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        self.element(value)
    }
    fn end(self) -> Result<(), CodecError> {
        self.finish()
    }
}
impl SerializeStruct for Compound<'_> {
    type Ok = ();
    type Error = CodecError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), CodecError> {
        self.field(key, value)
    }
    fn end(self) -> Result<(), CodecError> {
        self.finish()
    }
}
impl SerializeStructVariant for Compound<'_> {
    type Ok = ();
    type Error = CodecError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), CodecError> {
        self.field(key, value)
    }
    fn end(self) -> Result<(), CodecError> {
        self.finish()
    }
}
impl SerializeMap for Compound<'_> {
    type Ok = ();
    type Error = CodecError;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), CodecError> {
        if self.pending_key {
            return Err(CodecError::Invalid);
        }
        self.comma()?;
        key.serialize(Json {
            encoder: self.encoder,
            key: true,
        })?;
        self.pending_key = true;
        Ok(())
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        if !self.pending_key {
            return Err(CodecError::Invalid);
        }
        self.encoder.write(b":")?;
        value.serialize(Json {
            encoder: self.encoder,
            key: false,
        })?;
        self.pending_key = false;
        Ok(())
    }
    fn end(self) -> Result<(), CodecError> {
        self.finish()
    }
}
