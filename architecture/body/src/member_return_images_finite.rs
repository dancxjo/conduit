//! Finite fallible text ownership for the already bounded return credential and
//! one grant row. Derivation preserves their ordinary field/default semantics.
use crate::{BodyId, MembershipCredential, MembershipCredentialId, PartId, MAX_LIFECYCLE_ID_BYTES};
use alloc::string::String;
use conduit_core::AuthorityGrant;
use core::fmt;
use serde::{
    de::{self, DeserializeSeed, Visitor},
    Deserialize,
};

pub(super) struct ExactText<'a>(pub &'a str);
impl<'de> DeserializeSeed<'de> for ExactText<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_str(self)
    }
}
impl<'de> Visitor<'de> for ExactText<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("exact text")
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<(), E> {
        if value == self.0 {
            Ok(())
        } else {
            Err(E::custom("text mismatch"))
        }
    }
}
struct FiniteText(String);
impl<'de> Deserialize<'de> for FiniteText {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Text;
        impl<'de> Visitor<'de> for Text {
            type Value = FiniteText;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("finite lifecycle identifier")
            }
            fn visit_str<E: de::Error>(self, text: &str) -> Result<FiniteText, E> {
                if text.is_empty() || text.len() > MAX_LIFECYCLE_ID_BYTES {
                    return Err(E::custom("identifier bounds"));
                }
                let mut value = String::new();
                value
                    .try_reserve_exact(text.len())
                    .map_err(|_| E::custom("identifier allocation"))?;
                value.push_str(text);
                Ok(FiniteText(value))
            }
        }
        d.deserialize_string(Text)
    }
}
#[derive(Deserialize)]
pub(super) struct FiniteCredential {
    credential_id: FiniteText,
    body_id: FiniteText,
    part_id: FiniteText,
    host_id: FiniteText,
    boot_id: FiniteText,
    issued_at_millis: u64,
}
impl From<FiniteCredential> for MembershipCredential {
    fn from(value: FiniteCredential) -> Self {
        Self {
            credential_id: MembershipCredentialId::bound(value.credential_id.0),
            body_id: BodyId::bound(value.body_id.0),
            part_id: PartId::bound(value.part_id.0),
            host_id: value.host_id.0.into(),
            boot_id: value.boot_id.0.into(),
            issued_at_millis: value.issued_at_millis,
        }
    }
}
#[derive(Deserialize)]
pub(super) struct FiniteGrant {
    grant_id: FiniteText,
    contract_id: FiniteText,
    host_call_contract_id: FiniteText,
    subject_kind: FiniteText,
    host_id: FiniteText,
    boot_id: FiniteText,
    capability_id: FiniteText,
}
impl From<FiniteGrant> for AuthorityGrant {
    fn from(value: FiniteGrant) -> Self {
        Self {
            grant_id: value.grant_id.0.into(),
            contract_id: value.contract_id.0.into(),
            host_call_contract_id: value.host_call_contract_id.0.into(),
            subject_kind: value.subject_kind.0.into(),
            host_id: value.host_id.0.into(),
            boot_id: value.boot_id.0.into(),
            capability_id: value.capability_id.0.into(),
        }
    }
}
