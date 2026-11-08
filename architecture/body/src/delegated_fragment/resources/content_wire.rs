//! Borrowed signed content facts, converted only after preparation is funded.
use super::*;
use conduit_core as c;

/// The existing Core content contract without owned strings or hidden allocation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FragmentContentContract<'a> {
    pub identity: c::ResourceSemanticIdentity,
    pub version: c::ResourceVersionIdentity,
    #[serde(borrow)]
    pub content_profile: &'a str,
    pub maximum_bytes: u32,
    pub maximum_items: u32,
    pub retention: c::ResourceRetention,
    pub sharing: c::ResourceSharing,
    pub access: c::ResourceAccessMode,
    pub generation_slots: u16,
    pub reader_leases: u16,
    pub publication_slots: u16,
    pub sensitive: bool,
}
impl<'a> FragmentContentContract<'a> {
    pub fn from_core(value: &'a c::ResourceContentRequirement) -> Self {
        let c::ResourceContentRequirement {
            identity,
            version,
            content_profile,
            maximum_bytes,
            maximum_items,
            retention,
            sharing,
            access,
            generation_slots,
            reader_leases,
            publication_slots,
            sensitive,
        } = value;
        Self {
            identity: *identity,
            version: *version,
            content_profile: content_profile.as_str(),
            maximum_bytes: *maximum_bytes,
            maximum_items: *maximum_items,
            retention: *retention,
            sharing: *sharing,
            access: *access,
            generation_slots: *generation_slots,
            reader_leases: *reader_leases,
            publication_slots: *publication_slots,
            sensitive: *sensitive,
        }
    }
    #[cfg(feature = "authenticated-admission")]
    pub(super) fn owned(&self) -> Result<c::ResourceContentRequirement, DelegatedResourceRefusal> {
        Ok(c::ResourceContentRequirement {
            identity: self.identity,
            version: self.version,
            content_profile: super::admission::copy_id(self.content_profile)?.into(),
            maximum_bytes: self.maximum_bytes,
            maximum_items: self.maximum_items,
            retention: self.retention,
            sharing: self.sharing,
            access: self.access,
            generation_slots: self.generation_slots,
            reader_leases: self.reader_leases,
            publication_slots: self.publication_slots,
            sensitive: self.sensitive,
        })
    }
}
/// Exact selected residence. Its owner Host/Boot are the signed fragment scope,
/// never incoming mutable inventory. Core rechecks that scope and this contract.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FragmentContent<'a> {
    #[serde(borrow)]
    pub contract: FragmentContentContract<'a>,
    pub base: &'a str,
    pub residence_profile: &'a str,
}
impl FragmentContent<'_> {
    pub(super) fn valid(&self) -> bool {
        [
            self.contract.content_profile,
            self.base,
            self.residence_profile,
        ]
        .iter()
        .all(|v| !v.is_empty() && v.len() <= 128)
    }
}
