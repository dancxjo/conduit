//! Observe the Serde data model, including names hidden by JSON encoding.
use serde::{
    ser::{self, SerializeSeq, SerializeStruct},
    Serialize,
};
#[derive(Default)]
struct Model(Vec<String>);
type Error = serde_json::Error;
macro_rules! scalar {
    ($($method:ident($ty:ty)),*) => {$ (
        fn $method(self, value: $ty) -> Result<(), Error> {
            self.0.push(format!("{}:{value:?}", stringify!($method))); Ok(())
        }
    )*};
}
impl<'a> ser::Serializer for &'a mut Model {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeStruct = Self;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;
    scalar!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char),
        serialize_str(&str),
        serialize_bytes(&[u8])
    );
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.0.push(format!("newtype:{name}"));
        value.serialize(self)
    }
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self, Error> {
        self.0.push(format!("struct:{name}:{len}"));
        Ok(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self, Error> {
        self.0.push(format!("seq:{len:?}"));
        Ok(self)
    }
    fn serialize_none(self) -> Result<(), Error> {
        Err(ser::Error::custom("unexpected none"))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<(), Error> {
        Err(ser::Error::custom("unexpected some"))
    }
    fn serialize_unit(self) -> Result<(), Error> {
        Err(ser::Error::custom("unexpected unit"))
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> {
        Err(ser::Error::custom("unexpected unit struct"))
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Error> {
        Err(ser::Error::custom("unexpected unit variant"))
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(), Error> {
        Err(ser::Error::custom("unexpected variant"))
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Error> {
        Err(ser::Error::custom("unexpected tuple"))
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(ser::Error::custom("unexpected tuple struct"))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(ser::Error::custom("unexpected tuple variant"))
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(ser::Error::custom("unexpected map"))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(ser::Error::custom("unexpected struct variant"))
    }
}
impl SerializeSeq for &mut Model {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Error> {
        self.0.push("end-seq".into());
        Ok(())
    }
}
impl SerializeStruct for &mut Model {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.0.push(format!("field:{key}"));
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Error> {
        self.0.push("end-struct".into());
        Ok(())
    }
}
#[test]
fn compact_receipts_preserve_original_struct_and_identity_newtype_tokens() {
    let mut manager = super::manager(28);
    let original: Vec<conduit_body::AdmissionReceipt> = (0..28)
        .map(|i| manager.receipts.owned(i).unwrap().clone())
        .collect();
    let mut expected = Model::default();
    original.serialize(&mut expected).unwrap();
    manager.receipts.try_compact_historical().unwrap();
    let mut actual = Model::default();
    manager.receipts.serialize(&mut actual).unwrap();
    assert_eq!(actual.0, expected.0);
}
