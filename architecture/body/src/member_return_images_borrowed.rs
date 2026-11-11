//! Semantic acknowledgement comparison without a second owned projection.
use super::*;
use core::{fmt, marker::PhantomData};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

// serde-json-core supports identifiers (the same path used by derived
// struct deserializers), but deliberately does not support String map keys.
struct FieldName(&'static str);
impl<'de> Deserialize<'de> for FieldName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Name;
        impl<'de> Visitor<'de> for Name {
            type Value = FieldName;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("field name")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<FieldName, E> {
                let name = match value {
                    "kind" => "kind",
                    "schema" => "schema",
                    "credential" => "credential",
                    "projection" => "projection",
                    "context" => "context",
                    "capability_images" => "capability_images",
                    "authority_grants" => "authority_grants",
                    _ => return Err(de::Error::custom("unknown field")),
                };
                Ok(FieldName(name))
            }
        }
        d.deserialize_identifier(Name)
    }
}

/// Parse the existing acknowledgement semantics, retaining only its credential.
/// Object field order and HostAdvertisement defaults remain serde's ordinary
/// semantics. Images and grants are checked one at a time in their signed order.
pub struct MemberReturnImageAcknowledgementSeed<'a> {
    pub retained: &'a MembershipCredential,
    pub actual: BorrowedHostAdvertisementImageProjection<'a>,
}
impl<'de> DeserializeSeed<'de> for MemberReturnImageAcknowledgementSeed<'_> {
    type Value = MembershipCredential;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.actual
            .validate()
            .map_err(|_| de::Error::custom("projection basis"))?;
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for MemberReturnImageAcknowledgementSeed<'_> {
    type Value = MembershipCredential;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("member return acknowledgement")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let (mut kind, mut schema, mut credential, mut projection) = (false, false, None, false);
        while let Some(key) = map.next_key::<FieldName>()? {
            match key.0 {
                "kind" if !kind => {
                    kind = true;
                    if map.next_value::<String>()? != "Admitted" {
                        return Err(de::Error::custom("kind"));
                    }
                }
                "schema" if !schema => {
                    schema = true;
                    if map.next_value::<String>()? != MEMBER_RETURN_IMAGES_SCHEMA {
                        return Err(de::Error::custom("schema"));
                    }
                }
                "credential" if credential.is_none() => {
                    credential = Some(map.next_value::<MembershipCredential>()?)
                }
                "projection" if !projection => {
                    map.next_value_seed(ProjectionSeed(self.actual))?;
                    projection = true;
                }
                _ => {
                    return Err(de::Error::custom(
                        "unknown or duplicate acknowledgement field",
                    ))
                }
            }
        }
        let credential = credential.ok_or_else(|| de::Error::missing_field("credential"))?;
        if !kind || !schema || !projection {
            return Err(de::Error::custom("missing acknowledgement field"));
        }
        // Reuse the existing relationship law. The actual context here is the
        // single original description; do not construct an owned acknowledgement.
        crate::member_return_acknowledgement::validate_relationship(
            &credential,
            self.retained,
            self.actual.context,
        )
        .map_err(|_| de::Error::custom("receipt basis"))?;
        Ok(credential)
    }
}
struct ProjectionSeed<'a>(BorrowedHostAdvertisementImageProjection<'a>);
impl<'de> DeserializeSeed<'de> for ProjectionSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for ProjectionSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("image projection")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        let (mut context, mut images, mut grants) = (false, false, false);
        while let Some(key) = map.next_key::<FieldName>()? {
            match key.0 {
                "context" if !context => {
                    context = true;
                    if map.next_value::<HostAdvertisement>()? != *self.0.context {
                        return Err(de::Error::custom("context"));
                    }
                }
                "capability_images" if !images => {
                    images = true;
                    map.next_value_seed(EqualSequence(self.0.capability_images, PhantomData))?;
                }
                "authority_grants" if !grants => {
                    grants = true;
                    map.next_value_seed(EqualSequence(self.0.authority_grants, PhantomData))?;
                }
                _ => return Err(de::Error::custom("unknown or duplicate projection field")),
            }
        }
        if !context || !images || !grants {
            return Err(de::Error::custom("missing projection field"));
        }
        Ok(())
    }
}
struct EqualSequence<'a, T>(&'a [T], PhantomData<T>);
impl<'de, T: Deserialize<'de> + PartialEq> DeserializeSeed<'de> for EqualSequence<'_, T> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de, T: Deserialize<'de> + PartialEq> Visitor<'de> for EqualSequence<'_, T> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("exact ordered descriptions")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
        for expected in self.0 {
            if seq.next_element::<T>()?.as_ref() != Some(expected) {
                return Err(de::Error::custom("description mismatch"));
            }
        }
        if seq.next_element::<de::IgnoredAny>()?.is_some() {
            return Err(de::Error::custom("extra description"));
        }
        Ok(())
    }
}
