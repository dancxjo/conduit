//! Equality of the plain Host context without decoding owned Host fields.
//! The supplied deserializer controls its own parsing and error storage.
//! This checks JSON semantics, not authority. Rich expected capability/compute/
//! content catalogs are explicitly unsupported and retain their ordinary decoder.
use crate::*;
use core::fmt;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

pub fn bounded_host_advertisement_comparison_supported(host: &HostAdvertisement) -> bool {
    host.capabilities.is_empty()
        && host.planner_capabilities.is_empty()
        && host
            .resources
            .iter()
            .all(|r| r.compute.is_none() && r.content.is_none())
}
pub struct HostAdvertisementEqualitySeed<'a>(&'a HostAdvertisement);
impl<'a> HostAdvertisementEqualitySeed<'a> {
    pub fn new(expected: &'a HostAdvertisement) -> Option<Self> {
        bounded_host_advertisement_comparison_supported(expected).then_some(Self(expected))
    }
}
impl<'de> DeserializeSeed<'de> for HostAdvertisementEqualitySeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        Compare(self.0).deserialize(d)
    }
}
fn mismatch<E: de::Error>() -> E {
    E::custom("Host context mismatch")
}
struct Text<'a>(&'a str);
impl<'de> DeserializeSeed<'de> for Text<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_str(self)
    }
}
impl<'de> Visitor<'de> for Text<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("exact text")
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<(), E> {
        if value == self.0 {
            Ok(())
        } else {
            Err(mismatch())
        }
    }
}
struct OptionalText<'a>(Option<&'a str>);
impl<'de> DeserializeSeed<'de> for OptionalText<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_option(self)
    }
}
impl<'de> Visitor<'de> for OptionalText<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("exact optional text")
    }
    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        if self.0.is_none() {
            Ok(())
        } else {
            Err(mismatch())
        }
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        self.visit_none()
    }
    fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        Text(self.0.ok_or_else(mismatch)?).deserialize(d)
    }
}
struct Empty;
impl<'de> DeserializeSeed<'de> for Empty {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for Empty {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("empty sequence")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
        seq.next_element_seed(Reject)?;
        Ok(())
    }
}
struct Reject;
impl<'de> DeserializeSeed<'de> for Reject {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
        Err(mismatch())
    }
}
struct Texts<'a, T>(&'a [T], fn(&T) -> &str);
impl<'de, T> DeserializeSeed<'de> for Texts<'_, T> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de, T> Visitor<'de> for Texts<'_, T> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("exact text sequence")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
        for item in self.0 {
            if seq.next_element_seed(Text((self.1)(item)))?.is_none() {
                return Err(mismatch());
            }
        }
        seq.next_element_seed(Reject)?;
        Ok(())
    }
}
trait Fields {
    const NAMES: &'static [&'static str];
    const REQUIRED: u16;
    fn field<'de, M: MapAccess<'de>>(&self, index: usize, map: &mut M) -> Result<(), M::Error>;
    fn defaults(&self, _: u16) -> bool {
        true
    }
}
struct Compare<'a, T>(&'a T);
impl<'de, T: Fields> DeserializeSeed<'de> for Compare<'_, T> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de, T: Fields> Visitor<'de> for Compare<'_, T> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("exact Host description")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        let mut seen = 0;
        while let Some(index) = map.next_key_seed(Key(T::NAMES))? {
            if index == usize::MAX {
                map.next_value::<de::IgnoredAny>()?;
                continue;
            }
            let bit = 1 << index;
            if seen & bit != 0 {
                return Err(de::Error::custom("duplicate field"));
            }
            seen |= bit;
            self.0.field(index, &mut map)?;
        }
        if seen & T::REQUIRED != T::REQUIRED || !self.0.defaults(seen) {
            return Err(mismatch());
        }
        Ok(())
    }
}
struct Key(&'static [&'static str]);
impl<'de> DeserializeSeed<'de> for Key {
    type Value = usize;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<usize, D::Error> {
        d.deserialize_identifier(self)
    }
}
impl<'de> Visitor<'de> for Key {
    type Value = usize;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("field name")
    }
    fn visit_str<E: de::Error>(self, key: &str) -> Result<usize, E> {
        Ok(self
            .0
            .iter()
            .position(|name| *name == key)
            .unwrap_or(usize::MAX))
    }
}
struct Rows<'a, T>(&'a [T]);
impl<'de, T: Fields> DeserializeSeed<'de> for Rows<'_, T> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de, T: Fields> Visitor<'de> for Rows<'_, T> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("exact ordered rows")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
        for item in self.0 {
            if seq.next_element_seed(Compare(item))?.is_none() {
                return Err(mismatch());
            }
        }
        seq.next_element_seed(Reject)?;
        Ok(())
    }
}
fn equal<'de, M: MapAccess<'de>, T: Deserialize<'de> + PartialEq>(
    map: &mut M,
    expected: &T,
) -> Result<(), M::Error> {
    if map.next_value::<T>()? == *expected {
        Ok(())
    } else {
        Err(mismatch())
    }
}
impl Fields for HostAdvertisement {
    const NAMES: &'static [&'static str] = &[
        "declared_name",
        "protocol_version",
        "host_id",
        "boot_id",
        "offer_generation",
        "profile",
        "bases",
        "resources",
        "capabilities",
        "planner_capabilities",
    ];
    const REQUIRED: u16 = 0b01_1011_1110;
    fn defaults(&self, seen: u16) -> bool {
        (seen & 1 != 0 || self.declared_name.is_none())
            && (seen & (1 << 6) != 0 || self.bases.is_empty())
    }
    fn field<'de, M: MapAccess<'de>>(&self, index: usize, map: &mut M) -> Result<(), M::Error> {
        match index {
            0 => map.next_value_seed(OptionalText(
                self.declared_name.as_ref().map(HostDeclaredName::as_str),
            )),
            1 => equal(map, &self.protocol_version),
            2 => map.next_value_seed(Text(self.host_id.as_str())),
            3 => map.next_value_seed(Text(self.boot_id.as_str())),
            4 => equal(map, &self.offer_generation),
            5 => map.next_value_seed(Text(self.profile.as_str())),
            6 => map.next_value_seed(Rows(&self.bases)),
            7 => map.next_value_seed(Rows(&self.resources)),
            8 | 9 => map.next_value_seed(Empty),
            _ => unreachable!(),
        }
    }
}
impl Fields for ResourceOffer {
    const NAMES: &'static [&'static str] = &[
        "pool_id",
        "class_id",
        "capacity_units",
        "compute",
        "content",
    ];
    const REQUIRED: u16 = 7;
    fn field<'de, M: MapAccess<'de>>(&self, index: usize, map: &mut M) -> Result<(), M::Error> {
        match index {
            0 => map.next_value_seed(Text(self.pool_id.as_str())),
            1 => map.next_value_seed(Text(self.class_id.as_str())),
            2 => equal(map, &self.capacity_units),
            3 | 4 => map.next_value_seed(OptionalText(None)),
            _ => unreachable!(),
        }
    }
}
impl Fields for BaseProviderAdvertisement {
    const NAMES: &'static [&'static str] = &[
        "base_id",
        "provider_instance_id",
        "provider_generation",
        "implementation_id",
        "mechanism_family",
        "enforcement_class",
        "lifecycle",
        "capability_ids",
        "resource_pool_ids",
    ];
    const REQUIRED: u16 = 511;
    fn field<'de, M: MapAccess<'de>>(&self, index: usize, map: &mut M) -> Result<(), M::Error> {
        match index {
            0 => map.next_value_seed(Text(self.base_id.as_str())),
            1 => map.next_value_seed(Text(self.provider_instance_id.as_str())),
            2 => equal(map, &self.provider_generation),
            3 => map.next_value_seed(Text(self.implementation_id.as_str())),
            4 => map.next_value_seed(Text(self.mechanism_family.as_str())),
            5 => equal(map, &self.enforcement_class),
            6 => equal(map, &self.lifecycle),
            7 => map.next_value_seed(Texts(&self.capability_ids, CapabilityId::as_str)),
            8 => map.next_value_seed(Texts(&self.resource_pool_ids, ResourcePoolId::as_str)),
            _ => unreachable!(),
        }
    }
}
