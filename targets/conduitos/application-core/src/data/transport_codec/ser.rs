//! Recursive Serde adaptation; no intermediate dynamic JSON or typed DTO copy.
use serde::{Serialize, Serializer, ser::*};
pub(super) struct Adapt<'a, T: ?Sized>(pub &'a T);
pub(super) struct Envelope<'a, T: ?Sized>(pub &'a T);
impl<T: Serialize + ?Sized> Serialize for Envelope<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(3)?;
        tuple.serialize_element(&2usize)?;
        tuple.serialize_element(super::VERSION)?;
        tuple.serialize_element(&Adapt(self.0))?;
        tuple.end()
    }
}
impl<T: Serialize + ?Sized> Serialize for Adapt<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(Encoder(serializer))
    }
}
struct Encoder<S>(S);
struct Compound<C>(C);
macro_rules! scalar {($($method:ident($ty:ty)),*$(,)?)=>{$(
    fn $method(self,value:$ty)->Result<Self::Ok,Self::Error>{self.0.$method(value)}
)*};}
fn framed<E: serde::ser::Error>(length: usize) -> Result<usize, E> {
    length
        .checked_add(1)
        .ok_or_else(|| E::custom("array length overflow"))
}
impl<S: Serializer> Serializer for Encoder<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Compound<S::SerializeSeq>;
    type SerializeTuple = Compound<S::SerializeTuple>;
    type SerializeTupleStruct = Compound<S::SerializeTupleStruct>;
    type SerializeTupleVariant = Compound<S::SerializeTupleVariant>;
    type SerializeMap = Compound<S::SerializeMap>;
    type SerializeStruct = Compound<S::SerializeStruct>;
    type SerializeStructVariant = Compound<S::SerializeStructVariant>;
    scalar!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char),
        serialize_str(&str)
    );
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        let mut sequence = self.serialize_seq(Some(value.len()))?;
        for byte in value {
            sequence.serialize_element(byte)?;
        }
        sequence.end()
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_none()
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_some(&Adapt(value))
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit()
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_struct(name)
    }
    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_variant(name, index, variant)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_newtype_struct(name, &Adapt(value))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0
            .serialize_newtype_variant(name, index, variant, &Adapt(value))
    }
    fn serialize_seq(self, length: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let length =
            length.ok_or_else(|| S::Error::custom("private sequence requires exact length"))?;
        let mut output = self.0.serialize_seq(Some(framed::<S::Error>(length)?))?;
        output.serialize_element(&length)?;
        Ok(Compound(output))
    }
    fn serialize_tuple(self, length: usize) -> Result<Self::SerializeTuple, Self::Error> {
        let mut output = self.0.serialize_tuple(framed::<S::Error>(length)?)?;
        output.serialize_element(&length)?;
        Ok(Compound(output))
    }
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        let mut output = self
            .0
            .serialize_tuple_struct(name, framed::<S::Error>(length)?)?;
        output.serialize_field(&length)?;
        Ok(Compound(output))
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        let mut output =
            self.0
                .serialize_tuple_variant(name, index, variant, framed::<S::Error>(length)?)?;
        output.serialize_field(&length)?;
        Ok(Compound(output))
    }
    fn serialize_map(self, length: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.0.serialize_map(length).map(Compound)
    }
    fn serialize_struct(
        self,
        name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.0.serialize_struct(name, length).map(Compound)
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.0
            .serialize_struct_variant(name, index, variant, length)
            .map(Compound)
    }
    fn collect_str<T: core::fmt::Display + ?Sized>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.collect_str(value)
    }
    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}
macro_rules! sequence {
    ($trait:ident,$method:ident) => {
        impl<C: $trait> $trait for Compound<C> {
            type Ok = C::Ok;
            type Error = C::Error;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
                self.0.$method(&Adapt(value))
            }
            fn end(self) -> Result<Self::Ok, Self::Error> {
                self.0.end()
            }
        }
    };
}
sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);
impl<C: SerializeMap> SerializeMap for Compound<C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.0.serialize_key(&Adapt(key))
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.0.serialize_value(&Adapt(value))
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}
macro_rules! structure {
    ($trait:ident) => {
        impl<C: $trait> $trait for Compound<C> {
            type Ok = C::Ok;
            type Error = C::Error;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<(), Self::Error> {
                self.0.serialize_field(key, &Adapt(value))
            }
            fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
                self.0.skip_field(key)
            }
            fn end(self) -> Result<Self::Ok, Self::Error> {
                self.0.end()
            }
        }
    };
}
structure!(SerializeStruct);
structure!(SerializeStructVariant);
