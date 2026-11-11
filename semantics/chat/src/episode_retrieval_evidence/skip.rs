//! A bounded visitor for opaque fields. Never interpret source/content as authority.
use core::fmt;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

pub(super) struct Skip<'a> {
    pub depth: usize,
    pub nodes: &'a mut usize,
}
impl<'de> DeserializeSeed<'de> for Skip<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        if self.depth > 8 || *self.nodes >= 512 {
            return Err(de::Error::custom("opaque field bound"));
        }
        *self.nodes += 1;
        deserializer.deserialize_ignored_any(self)
    }
}
impl<'de> Visitor<'de> for Skip<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded opaque value")
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq
            .next_element_seed(Skip {
                depth: self.depth + 1,
                nodes: self.nodes,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while map
            .next_key_seed(Skip {
                depth: self.depth + 1,
                nodes: self.nodes,
            })?
            .is_some()
        {
            map.next_value_seed(Skip {
                depth: self.depth + 1,
                nodes: self.nodes,
            })?;
        }
        Ok(())
    }
}

pub(super) struct BoundedDocument;
impl<'de> serde::Deserialize<'de> for BoundedDocument {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut nodes = 0;
        Skip {
            depth: 1,
            nodes: &mut nodes,
        }
        .deserialize(deserializer)?;
        Ok(Self)
    }
}
