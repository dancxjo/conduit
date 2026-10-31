//! Streaming typed Serde token digest, including unknown-length flattened maps.
//! Representation v1: distinct scalar/compound tags; strings and bytes have
//! u64-LE lengths; fixed-width numbers are little endian; compounds end in 0xff.
//! Map order is retained (the descriptor's derived Serialize order), not sorted.
//! Struct/variant/field names are included; optional container lengths are not.
//! No serializer method buffers, formats, or allocates payloads.
use core::fmt;
use serde::{ser, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub(super) struct Error;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("descriptor serialization")
    }
}
impl core::error::Error for Error {}
impl ser::Error for Error {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Self
    }
}
pub(super) fn digest<T: Serialize + ?Sized>(value: &T) -> Result<[u8; 32], Error> {
    let mut h = Hash(Sha256::new());
    h.0.update(b"conduit.body/current-host-offer-descriptor-serde-tokens@1\0");
    value.serialize(&mut h)?;
    Ok(h.0.finalize().into())
}
// HostDeclaredName's public serde `into = String` path clones its text.
// This borrowed view reproduces every public field and omission exactly while
// serializing that checked name through its borrowed spelling.
pub(super) fn advertisement_digest(
    ad: &conduit_core::HostAdvertisement,
) -> Result<[u8; 32], Error> {
    digest(&BorrowedAdvertisement(ad))
}
struct BorrowedAdvertisement<'a>(&'a conduit_core::HostAdvertisement);
impl Serialize for BorrowedAdvertisement<'_> {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use ser::SerializeStruct;
        let conduit_core::HostAdvertisement {
            declared_name,
            protocol_version,
            host_id,
            boot_id,
            offer_generation,
            profile,
            bases,
            resources,
            capabilities,
            planner_capabilities,
        } = self.0;
        let mut state = serializer.serialize_struct(
            "HostAdvertisement",
            8 + usize::from(declared_name.is_some()) + usize::from(!bases.is_empty()),
        )?;
        if let Some(name) = declared_name {
            state.serialize_field("declared_name", &Some(name.as_str()))?;
        }
        state.serialize_field("protocol_version", protocol_version)?;
        state.serialize_field("host_id", host_id)?;
        state.serialize_field("boot_id", boot_id)?;
        state.serialize_field("offer_generation", offer_generation)?;
        state.serialize_field("profile", profile)?;
        if !bases.is_empty() {
            state.serialize_field("bases", bases)?;
        }
        state.serialize_field("resources", resources)?;
        state.serialize_field("capabilities", capabilities)?;
        state.serialize_field("planner_capabilities", planner_capabilities)?;
        state.end()
    }
}
struct Hash(Sha256);
impl Hash {
    fn tag(&mut self, tag: u8) {
        self.0.update([tag]);
    }
    fn bytes(&mut self, tag: u8, bytes: &[u8]) {
        self.tag(tag);
        self.0.update((bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }
    fn name(&mut self, name: &str) {
        self.bytes(16, name.as_bytes());
    }
}
macro_rules! number {
    ($method:ident,$ty:ty,$tag:expr) => {
        fn $method(self, value: $ty) -> Result<(), Error> {
            self.tag($tag);
            self.0.update(value.to_le_bytes());
            Ok(())
        }
    };
}
impl<'a> ser::Serializer for &'a mut Hash {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Compound<'a>;
    type SerializeTuple = Compound<'a>;
    type SerializeTupleStruct = Compound<'a>;
    type SerializeTupleVariant = Compound<'a>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Compound<'a>;
    fn is_human_readable(&self) -> bool {
        false
    }
    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.tag(1);
        self.tag(u8::from(v));
        Ok(())
    }
    number!(serialize_i8, i8, 2);
    number!(serialize_i16, i16, 3);
    number!(serialize_i32, i32, 4);
    number!(serialize_i64, i64, 5);
    number!(serialize_i128, i128, 6);
    number!(serialize_u8, u8, 7);
    number!(serialize_u16, u16, 8);
    number!(serialize_u32, u32, 9);
    number!(serialize_u64, u64, 10);
    number!(serialize_u128, u128, 11);
    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        self.tag(12);
        self.0.update(v.to_bits().to_le_bytes());
        Ok(())
    }
    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        self.tag(13);
        self.0.update(v.to_bits().to_le_bytes());
        Ok(())
    }
    fn serialize_char(self, v: char) -> Result<(), Error> {
        self.tag(14);
        self.0.update((v as u32).to_le_bytes());
        Ok(())
    }
    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.bytes(15, v.as_bytes());
        Ok(())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.bytes(17, v);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Error> {
        self.tag(18);
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<(), Error> {
        self.tag(19);
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Error> {
        self.tag(20);
        Ok(())
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<(), Error> {
        self.tag(21);
        self.name(name);
        Ok(())
    }
    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        self.tag(22);
        self.name(name);
        self.0.update(index.to_le_bytes());
        self.name(variant);
        Ok(())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        v: &T,
    ) -> Result<(), Error> {
        self.tag(23);
        self.name(name);
        v.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        v: &T,
    ) -> Result<(), Error> {
        self.tag(24);
        self.name(name);
        self.0.update(index.to_le_bytes());
        self.name(variant);
        v.serialize(self)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Compound<'a>, Error> {
        self.tag(25);
        Ok(Compound(self))
    }
    fn serialize_tuple(self, _: usize) -> Result<Compound<'a>, Error> {
        self.tag(26);
        Ok(Compound(self))
    }
    fn serialize_tuple_struct(self, name: &'static str, _: usize) -> Result<Compound<'a>, Error> {
        self.tag(27);
        self.name(name);
        Ok(Compound(self))
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Compound<'a>, Error> {
        self.tag(28);
        self.name(name);
        self.0.update(index.to_le_bytes());
        self.name(variant);
        Ok(Compound(self))
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Compound<'a>, Error> {
        self.tag(29);
        Ok(Compound(self))
    }
    fn serialize_struct(self, name: &'static str, _: usize) -> Result<Compound<'a>, Error> {
        self.tag(30);
        self.name(name);
        Ok(Compound(self))
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Compound<'a>, Error> {
        self.tag(31);
        self.name(name);
        self.0.update(index.to_le_bytes());
        self.name(variant);
        Ok(Compound(self))
    }
    // Serde's default collect_str allocates. Refuse instead; descriptor values
    // must expose their typed representation, never silently allocate formatting.
    fn collect_str<T: fmt::Display + ?Sized>(self, _: &T) -> Result<(), Error> {
        Err(Error)
    }
}
struct Compound<'a>(&'a mut Hash);
macro_rules! sequence {
    ($trait:ident,$method:ident) => {
        impl ser::$trait for Compound<'_> {
            type Ok = ();
            type Error = Error;
            fn $method<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
                v.serialize(&mut *self.0)
            }
            fn end(self) -> Result<(), Error> {
                self.0.tag(255);
                Ok(())
            }
        }
    };
}
sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);
impl ser::SerializeMap for Compound<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        self.0.tag(32);
        v.serialize(&mut *self.0)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        self.0.tag(33);
        v.serialize(&mut *self.0)
    }
    fn end(self) -> Result<(), Error> {
        self.0.tag(255);
        Ok(())
    }
}
macro_rules! structure {
    ($trait:ident) => {
        impl ser::$trait for Compound<'_> {
            type Ok = ();
            type Error = Error;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                v: &T,
            ) -> Result<(), Error> {
                self.0.name(key);
                v.serialize(&mut *self.0)
            }
            fn end(self) -> Result<(), Error> {
                self.0.tag(255);
                Ok(())
            }
        }
    };
}
structure!(SerializeStruct);
structure!(SerializeStructVariant);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_boundaries_and_unknown_length_maps_are_unambiguous() {
        assert_ne!(digest(&["ab", "c"]).unwrap(), digest(&["a", "bc"]).unwrap());
        assert_ne!(digest(&Some(1u8)).unwrap(), digest(&1u8).unwrap());
        assert_ne!(digest(&[1u8, 2][..]).unwrap(), digest(&(1u8, 2u8)).unwrap());
        assert_ne!(digest(&1u8).unwrap(), digest(&1u16).unwrap());
        struct Map(bool);
        impl Serialize for Map {
            fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                use ser::SerializeMap;
                let mut map = s.serialize_map(self.0.then_some(1))?;
                map.serialize_entry("field", &42u64)?;
                map.end()
            }
        }
        assert_eq!(digest(&Map(true)).unwrap(), digest(&Map(false)).unwrap());
    }
}
