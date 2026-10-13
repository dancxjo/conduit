//! Bounded internal cache keys, never producer authentication or admission.
use core::fmt;
use serde::ser::{self, Serialize, Serializer};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refusal {
    Extent,
    Depth,
    Unsupported,
}
impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "cache fingerprint refused: {self:?}")
    }
}
impl core::error::Error for Refusal {}
impl ser::Error for Refusal {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Self::Unsupported
    }
}
#[cfg(test)]
pub(super) fn fingerprint<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
) -> Result<([u8; 32], usize), Refusal> {
    let (result, bytes) = attempt(value, limit);
    result.map(|digest| (digest, bytes))
}
pub(super) fn attempt<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
) -> (Result<[u8; 32], Refusal>, usize) {
    let mut state = State {
        hash: Sha256::new(),
        bytes: 0,
        limit,
        depth: 0,
    };
    state.hash.update(b"conduit.planner/global-hard-cache@1");
    let result = value.serialize(&mut state).and({
        if state.depth != 0 {
            Err(Refusal::Unsupported)
        } else {
            Ok(())
        }
    });
    let bytes = state.bytes;
    (result.map(|()| state.hash.finalize().into()), bytes)
}

struct State {
    hash: Sha256,
    bytes: usize,
    limit: usize,
    depth: u8,
}
impl State {
    fn write(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        let next = self.bytes.checked_add(bytes.len()).ok_or(Refusal::Extent)?;
        if next > self.limit {
            return Err(Refusal::Extent);
        }
        self.hash.update(bytes);
        self.bytes = next;
        Ok(())
    }
    fn sized(&mut self, tag: u8, bytes: &[u8]) -> Result<(), Refusal> {
        self.write(&[tag])?;
        self.write(&(bytes.len() as u64).to_le_bytes())?;
        self.write(bytes)
    }
    fn begin(&mut self, tag: u8, len: Option<usize>) -> Result<(), Refusal> {
        if self.depth == 64 {
            return Err(Refusal::Depth);
        }
        self.write(&[tag])?;
        self.write(&len.map_or(u64::MAX, |n| n as u64).to_le_bytes())?;
        self.depth += 1;
        Ok(())
    }
    fn end(&mut self) -> Result<(), Refusal> {
        self.depth = self.depth.checked_sub(1).ok_or(Refusal::Unsupported)?;
        self.write(&[255])
    }
}
impl Serializer for &mut State {
    type Ok = ();
    type Error = Refusal;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;
    fn is_human_readable(&self) -> bool {
        false
    }
    fn serialize_bool(self, v: bool) -> Result<(), Refusal> {
        self.write(&[1])?;
        self.write(&[v as u8])
    }
    fn serialize_i8(self, v: i8) -> Result<(), Refusal> {
        self.write(&[2])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_i16(self, v: i16) -> Result<(), Refusal> {
        self.write(&[3])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_i32(self, v: i32) -> Result<(), Refusal> {
        self.write(&[4])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_i64(self, v: i64) -> Result<(), Refusal> {
        self.write(&[5])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_i128(self, v: i128) -> Result<(), Refusal> {
        self.write(&[6])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_u8(self, v: u8) -> Result<(), Refusal> {
        self.write(&[7])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_u16(self, v: u16) -> Result<(), Refusal> {
        self.write(&[8])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_u32(self, v: u32) -> Result<(), Refusal> {
        self.write(&[9])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_u64(self, v: u64) -> Result<(), Refusal> {
        self.write(&[10])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_u128(self, v: u128) -> Result<(), Refusal> {
        self.write(&[11])?;
        self.write(&v.to_le_bytes())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Refusal> {
        self.write(&[12])?;
        self.write(&v.to_bits().to_le_bytes())
    }
    fn serialize_f64(self, v: f64) -> Result<(), Refusal> {
        self.write(&[13])?;
        self.write(&v.to_bits().to_le_bytes())
    }
    fn serialize_char(self, v: char) -> Result<(), Refusal> {
        self.write(&[14])?;
        self.write(&(v as u32).to_le_bytes())
    }
    fn serialize_str(self, v: &str) -> Result<(), Refusal> {
        self.sized(20, v.as_bytes())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Refusal> {
        self.sized(21, v)
    }
    fn serialize_none(self) -> Result<(), Refusal> {
        self.write(&[22])
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<(), Refusal> {
        self.begin(23, Some(1))?;
        v.serialize(&mut *self)?;
        self.end()
    }
    fn serialize_unit(self) -> Result<(), Refusal> {
        self.write(&[24])
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<(), Refusal> {
        self.sized(25, name.as_bytes())
    }
    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<(), Refusal> {
        self.sized(26, name.as_bytes())?;
        self.write(&index.to_le_bytes())?;
        self.sized(27, variant.as_bytes())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<(), Refusal> {
        self.begin(28, Some(1))?;
        self.sized(29, name.as_bytes())?;
        value.serialize(&mut *self)?;
        self.end()
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Refusal> {
        self.begin(30, Some(1))?;
        self.sized(31, name.as_bytes())?;
        self.write(&index.to_le_bytes())?;
        self.sized(32, variant.as_bytes())?;
        value.serialize(&mut *self)?;
        self.end()
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self, Refusal> {
        self.begin(40, len)?;
        Ok(self)
    }
    fn serialize_tuple(self, len: usize) -> Result<Self, Refusal> {
        self.begin(41, Some(len))?;
        Ok(self)
    }
    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self, Refusal> {
        self.begin(42, Some(len))?;
        self.sized(43, name.as_bytes())?;
        Ok(self)
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self, Refusal> {
        self.begin(44, Some(len))?;
        self.sized(45, name.as_bytes())?;
        self.write(&index.to_le_bytes())?;
        self.sized(46, variant.as_bytes())?;
        Ok(self)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self, Refusal> {
        self.begin(47, len)?;
        Ok(self)
    }
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self, Refusal> {
        self.begin(48, Some(len))?;
        self.sized(49, name.as_bytes())?;
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self, Refusal> {
        self.begin(50, Some(len))?;
        self.sized(51, name.as_bytes())?;
        self.write(&index.to_le_bytes())?;
        self.sized(52, variant.as_bytes())?;
        Ok(self)
    }
    fn collect_str<T: fmt::Display + ?Sized>(self, _: &T) -> Result<(), Refusal> {
        Err(Refusal::Unsupported)
    }
}
impl ser::SerializeSeq for &mut State {
    type Ok = ();
    type Error = Refusal;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Refusal> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Refusal> {
        State::end(self)
    }
}
impl ser::SerializeTuple for &mut State {
    type Ok = ();
    type Error = Refusal;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Refusal> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Refusal> {
        State::end(self)
    }
}
impl ser::SerializeTupleStruct for &mut State {
    type Ok = ();
    type Error = Refusal;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Refusal> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Refusal> {
        State::end(self)
    }
}
impl ser::SerializeTupleVariant for &mut State {
    type Ok = ();
    type Error = Refusal;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Refusal> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Refusal> {
        State::end(self)
    }
}
impl ser::SerializeMap for &mut State {
    type Ok = ();
    type Error = Refusal;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Refusal> {
        self.write(&[60])?;
        key.serialize(&mut **self)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Refusal> {
        self.write(&[61])?;
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Refusal> {
        State::end(self)
    }
}
impl ser::SerializeStruct for &mut State {
    type Ok = ();
    type Error = Refusal;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Refusal> {
        self.sized(62, key.as_bytes())?;
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Refusal> {
        State::end(self)
    }
}
impl ser::SerializeStructVariant for &mut State {
    type Ok = ();
    type Error = Refusal;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Refusal> {
        self.sized(62, key.as_bytes())?;
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Refusal> {
        State::end(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_types_and_order_do_not_alias() {
        let a = fingerprint(&("ab", "c"), 1024).unwrap().0;
        assert_ne!(a, fingerprint(&("a", "bc"), 1024).unwrap().0);
        assert_ne!(
            fingerprint(&1_u8, 1024).unwrap().0,
            fingerprint(&1_u64, 1024).unwrap().0
        );
        assert_ne!(
            fingerprint(&(1_u8, 2_u8), 1024).unwrap().0,
            fingerprint(&(2_u8, 1_u8), 1024).unwrap().0
        );
    }
    #[test]
    fn exact_extent_boundary_refuses_before_excess_data() {
        let value = ("host", 10_u64);
        let (digest, bytes) = fingerprint(&value, 1024).unwrap();
        assert_eq!(fingerprint(&value, bytes).unwrap(), (digest, bytes));
        assert_eq!(fingerprint(&value, bytes - 1), Err(Refusal::Extent));
        assert_eq!(fingerprint(&value, 0), Err(Refusal::Extent));
    }
    #[derive(serde::Serialize)]
    enum Node {
        End,
        Nested(alloc::boxed::Box<Node>),
    }
    #[test]
    fn nesting_has_an_independent_finite_limit() {
        let mut value = Node::End;
        for _ in 0..65 {
            value = Node::Nested(alloc::boxed::Box::new(value));
        }
        assert_eq!(fingerprint(&value, 65536), Err(Refusal::Depth));
    }
}
