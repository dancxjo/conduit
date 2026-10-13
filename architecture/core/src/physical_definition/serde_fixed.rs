//! Variable wire admission checks its bound before extending owned storage.
use core::fmt;
use serde::de::{Error, SeqAccess, Visitor};
/// Preparation-owned variable wire, bounded before every append.
pub(crate) fn deserialize_bounded<'de,D:serde::Deserializer<'de>,const N:usize>(deserializer:D)->Result<alloc::vec::Vec<u8>,D::Error> {
    struct Bounded<const N:usize>;
    impl<'de,const N:usize> Visitor<'de> for Bounded<N> {
        type Value=alloc::vec::Vec<u8>;
        fn expecting(&self,f:&mut fmt::Formatter)->fmt::Result {write!(f,"at most {N} canonical bytes")}
        fn visit_seq<A:SeqAccess<'de>>(self,mut sequence:A)->Result<Self::Value,A::Error> {
            if sequence.size_hint().is_some_and(|size|size>N){return Err(A::Error::custom("canonical wire bound exceeded"));}
            let mut bytes=alloc::vec::Vec::new();
            while let Some(byte)=sequence.next_element::<u8>()? {
                if bytes.len()==N {return Err(A::Error::custom("canonical wire bound exceeded"));}
                bytes.push(byte);
            }
            Ok(bytes)
        }
    }
    deserializer.deserialize_seq(Bounded::<N>)
}
