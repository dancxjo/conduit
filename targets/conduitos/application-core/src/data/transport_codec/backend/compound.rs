//! Streaming compound JSON, including all externally tagged enum forms.
use super::{Error, Writer};
use serde::{Serialize, ser::*};
pub(super) struct Compound<'a, 'b> {
    pub(super) writer: &'a mut Writer<'b>,
    pub(super) first: bool,
    pub(super) close: &'static [u8],
    pub(super) awaiting_value: bool,
}
impl Compound<'_, '_> {
    fn separator(&mut self) -> Result<(), Error> {
        if self.first {
            self.first = false;
            Ok(())
        } else {
            self.writer.byte(b',')
        }
    }
    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.separator()?;
        value.serialize(&mut *self.writer)
    }
    fn field<T: Serialize + ?Sized>(&mut self, key: &str, value: &T) -> Result<(), Error> {
        self.separator()?;
        self.writer.string(key)?;
        self.writer.byte(b':')?;
        value.serialize(&mut *self.writer)
    }
    fn finish(self) -> Result<(), Error> {
        if self.awaiting_value {
            return Err(Error::Malformed);
        }
        self.writer.bytes(self.close)
    }
}
macro_rules! sequence {
    ($trait:ident,$method:ident) => {
        impl $trait for Compound<'_, '_> {
            type Ok = ();
            type Error = Error;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
                self.element(value)
            }
            fn end(self) -> Result<(), Error> {
                self.finish()
            }
        }
    };
}
sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);
macro_rules! structure {
    ($trait:ident) => {
        impl $trait for Compound<'_, '_> {
            type Ok = ();
            type Error = Error;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<(), Error> {
                self.field(key, value)
            }
            fn end(self) -> Result<(), Error> {
                self.finish()
            }
        }
    };
}
structure!(SerializeStruct);
structure!(SerializeStructVariant);
impl SerializeMap for Compound<'_, '_> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        if self.awaiting_value {
            return Err(Error::Malformed);
        }
        self.separator()?;
        self.writer.key = true;
        let result = key.serialize(&mut *self.writer);
        self.writer.key = false;
        result?;
        self.writer.byte(b':')?;
        self.awaiting_value = true;
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        if !self.awaiting_value {
            return Err(Error::Malformed);
        }
        value.serialize(&mut *self.writer)?;
        self.awaiting_value = false;
        Ok(())
    }
    fn end(self) -> Result<(), Error> {
        self.finish()
    }
}
