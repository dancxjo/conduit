//! Nonallocating backing quote for this profile's cloneable descriptor trees.
//! Struct/enum storage is counted at its owning root or sequence element;
//! recursive Strings and Vec elements add their newly cloned backing. This is
//! requested Rust storage, not allocator arenas or whole-process RSS. Maps are
//! not in this descriptor profile and refuse rather than assume node occupancy.
use serde::ser::Error as _;
use serde::{ser::*, Serialize, Serializer};
#[derive(Clone, Copy)]
struct Heap;
struct Children {
    bytes: usize,
}
type Error = serde_json::Error;
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b)
        .ok_or_else(|| Error::custom("descriptor backing overflow"))
}
pub(super) fn count<T: Serialize>(value: &T) -> Result<usize, String> {
    add(
        std::mem::size_of::<T>(),
        value.serialize(Heap).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
macro_rules! scalar {($($name:ident:$ty:ty),*)=>{$(fn $name(self,_:$ty)->Result<usize,Error>{Ok(0)})*};}
impl Serializer for Heap {
    fn is_human_readable(&self) -> bool {
        false
    }
    type Ok = usize;
    type Error = Error;
    type SerializeSeq = Children;
    type SerializeTuple = Children;
    type SerializeTupleStruct = Children;
    type SerializeTupleVariant = Children;
    type SerializeMap = Impossible<usize, Error>;
    type SerializeStruct = Children;
    type SerializeStructVariant = Children;
    scalar!(serialize_bool:bool,serialize_i8:i8,serialize_i16:i16,serialize_i32:i32,serialize_i64:i64,serialize_i128:i128,serialize_u8:u8,serialize_u16:u16,serialize_u32:u32,serialize_u64:u64,serialize_u128:u128,serialize_f32:f32,serialize_f64:f64,serialize_char:char);
    fn serialize_str(self, value: &str) -> Result<usize, Error> {
        Ok(value.len())
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<usize, Error> {
        Ok(value.len())
    }
    fn serialize_none(self) -> Result<usize, Error> {
        Ok(0)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<usize, Error> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<usize, Error> {
        Ok(0)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<usize, Error> {
        Ok(0)
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<usize, Error> {
        Ok(0)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<usize, Error> {
        add(std::mem::size_of_val(value), value.serialize(self)?)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        value: &T,
    ) -> Result<usize, Error> {
        value.serialize(self)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Children, Error> {
        Ok(Children { bytes: 0 })
    }
    fn serialize_tuple(self, _: usize) -> Result<Children, Error> {
        Ok(Children { bytes: 0 })
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Children, Error> {
        Ok(Children { bytes: 0 })
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Children, Error> {
        Ok(Children { bytes: 0 })
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(Error::custom(
            "map-backed descriptor is outside the selected storage preparation profile",
        ))
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Children, Error> {
        Ok(Children { bytes: 0 })
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Children, Error> {
        Ok(Children { bytes: 0 })
    }
    fn collect_str<T: ?Sized + std::fmt::Display>(self, _: &T) -> Result<usize, Error> {
        Err(Error::custom(
            "display-serialized descriptor is outside the selected storage preparation profile",
        ))
    }
}
impl Children {
    fn child<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.bytes = add(self.bytes, value.serialize(Heap)?)?;
        // Inline field storage is intentionally charged again. In particular,
        // serde transparently serializes a Box's pointee; charging fields and
        // alignment prevents that owned node from disappearing from the quote.
        self.bytes = add(self.bytes, std::mem::size_of_val(value))?;
        self.bytes = add(self.bytes, std::mem::align_of_val(value))?;
        Ok(())
    }
}
macro_rules! sequence {
    ($trait:ident,$method:ident) => {
        impl $trait for Children {
            type Ok = usize;
            type Error = Error;
            fn $method<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
                self.child(value)
            }
            fn end(self) -> Result<usize, Error> {
                Ok(self.bytes)
            }
        }
    };
}
sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);
impl SerializeStruct for Children {
    type Ok = usize;
    type Error = Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.child(value)
    }
    fn end(self) -> Result<usize, Error> {
        Ok(self.bytes)
    }
}
impl SerializeStructVariant for Children {
    type Ok = usize;
    type Error = Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.child(value)
    }
    fn end(self) -> Result<usize, Error> {
        Ok(self.bytes)
    }
}
