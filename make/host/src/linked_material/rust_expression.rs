//! Make-only typed Rust expressions. No decoder or authorizing constructor is
//! generated. Unsupported serialization shapes refuse Make rather than fall
//! back to runtime JSON decoding.
use serde::ser::{self, Serialize, Serializer};
use std::fmt;
#[derive(Debug)]
pub(super) struct Error;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unsupported linked Rust shape")
    }
}
impl std::error::Error for Error {}
impl ser::Error for Error {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Self
    }
}
pub fn emit<T: Serialize + ?Sized>(value: &T) -> Result<String, fmt::Error> {
    value.serialize(Rust).map_err(|_| fmt::Error)
}
struct Rust;
struct Compound {
    prefix: String,
    fields: Vec<(Option<&'static str>, String)>,
    kind: Kind,
}
enum Kind {
    List,
    Tuple,
    Struct(&'static str),
    Variant,
}
fn ty(name: &str) -> String {
    if name == "CheckedGear" {
        "conduit_plot::CheckedGear".into()
    } else if matches!(name, "MaskPlot" | "MaskPlotBoundary") {
        format!("conduit_face::{name}")
    } else {
        format!("c::{name}")
    }
}
impl Compound {
    fn push<T: Serialize + ?Sized>(
        &mut self,
        key: Option<&'static str>,
        value: &T,
    ) -> Result<(), Error> {
        self.fields.push((key, value.serialize(Rust)?));
        Ok(())
    }
    fn finish(mut self) -> Result<String, Error> {
        if let Kind::Struct(name) = self.kind {
            let defaults: &[(&str, &str)] = match name {
                "PortDescriptor" => &[("abnormal_kind", "None")],
                "CapabilityOffer" => &[
                    ("state_retention", "None"),
                    ("realization_properties", "alloc::vec![]"),
                ],
                _ => &[],
            };
            for &(key, value) in defaults {
                if !self.fields.iter().any(|(k, _)| *k == Some(key)) {
                    self.fields.push((Some(key), value.into()));
                }
            }
        }
        let body = self
            .fields
            .into_iter()
            .map(|(key, value)| match key {
                Some(key) => format!("{key}:{value},"),
                None => format!("{value},"),
            })
            .collect::<String>();
        Ok(match self.kind {
            Kind::List => format!("alloc::vec![{body}]"),
            Kind::Tuple => format!("({body})"),
            Kind::Struct(_) | Kind::Variant => format!("{}{{{body}}}", self.prefix),
        })
    }
}
macro_rules! number {($($method:ident:$t:ty),*)=>{$(fn $method(self,v:$t)->Result<String,Error>{Ok(format!("{v}"))})*}}
impl Serializer for Rust {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = Compound;
    type SerializeTuple = Compound;
    type SerializeTupleStruct = Compound;
    type SerializeTupleVariant = Compound;
    type SerializeMap = ser::Impossible<String, Error>;
    type SerializeStruct = Compound;
    type SerializeStructVariant = Compound;
    fn is_human_readable(&self) -> bool {
        false
    }
    number!(serialize_i8:i8,serialize_i16:i16,serialize_i32:i32,serialize_i64:i64,serialize_i128:i128,serialize_u8:u8,serialize_u16:u16,serialize_u32:u32,serialize_u64:u64,serialize_u128:u128);
    fn serialize_bool(self, v: bool) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_f32(self, v: f32) -> Result<String, Error> {
        if v.is_finite() {
            Ok(format!("{v:?}"))
        } else {
            Err(Error)
        }
    }
    fn serialize_f64(self, v: f64) -> Result<String, Error> {
        if v.is_finite() {
            Ok(format!("{v:?}"))
        } else {
            Err(Error)
        }
    }
    fn serialize_char(self, v: char) -> Result<String, Error> {
        Ok(format!("{v:?}"))
    }
    fn serialize_str(self, v: &str) -> Result<String, Error> {
        Ok(format!("{v:?}.into()"))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<String, Error> {
        Ok(format!("alloc::vec!{v:?}"))
    }
    fn serialize_none(self) -> Result<String, Error> {
        Ok("None".into())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<String, Error> {
        Ok(format!("Some({})", v.serialize(Rust)?))
    }
    fn serialize_unit(self) -> Result<String, Error> {
        Ok("()".into())
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<String, Error> {
        Ok(ty(name))
    }
    fn serialize_unit_variant(
        self,
        name: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<String, Error> {
        Ok(format!("{}::{variant}", ty(name)))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        v: &T,
    ) -> Result<String, Error> {
        v.serialize(Rust)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        _: u32,
        variant: &'static str,
        v: &T,
    ) -> Result<String, Error> {
        Ok(format!("{}::{variant}({})", ty(name), v.serialize(Rust)?))
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Compound, Error> {
        Ok(Compound {
            prefix: String::new(),
            fields: vec![],
            kind: Kind::List,
        })
    }
    fn serialize_tuple(self, _: usize) -> Result<Compound, Error> {
        Ok(Compound {
            prefix: String::new(),
            fields: vec![],
            kind: Kind::Tuple,
        })
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Compound, Error> {
        Err(Error)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Compound, Error> {
        Err(Error)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(Error)
    }
    fn serialize_struct(self, name: &'static str, _: usize) -> Result<Compound, Error> {
        Ok(Compound {
            prefix: ty(name),
            fields: vec![],
            kind: Kind::Struct(name),
        })
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Compound, Error> {
        Ok(Compound {
            prefix: format!("{}::{variant}", ty(name)),
            fields: vec![],
            kind: Kind::Variant,
        })
    }
}
impl ser::SerializeSeq for Compound {
    type Ok = String;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        self.push(None, v)
    }
    fn end(self) -> Result<String, Error> {
        self.finish()
    }
}
impl ser::SerializeTuple for Compound {
    type Ok = String;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        self.push(None, v)
    }
    fn end(self) -> Result<String, Error> {
        self.finish()
    }
}
impl ser::SerializeTupleStruct for Compound {
    type Ok = String;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        self.push(None, v)
    }
    fn end(self) -> Result<String, Error> {
        self.finish()
    }
}
impl ser::SerializeTupleVariant for Compound {
    type Ok = String;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        self.push(None, v)
    }
    fn end(self) -> Result<String, Error> {
        self.finish()
    }
}
impl ser::SerializeStruct for Compound {
    type Ok = String;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        k: &'static str,
        v: &T,
    ) -> Result<(), Error> {
        self.push(Some(k), v)
    }
    fn end(self) -> Result<String, Error> {
        self.finish()
    }
}
impl ser::SerializeStructVariant for Compound {
    type Ok = String;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        k: &'static str,
        v: &T,
    ) -> Result<(), Error> {
        self.push(Some(k), v)
    }
    fn end(self) -> Result<String, Error> {
        self.finish()
    }
}
