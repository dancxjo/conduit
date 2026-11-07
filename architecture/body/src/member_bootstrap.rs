//! Private construction material, never evidence of current-Boot membership.
use crate::{
    validate_pico_spawn_provision, BodyId, FederationPeer, MembershipCredential,
    PicoSpawnProvision, MAX_LIFECYCLE_ID_BYTES,
};
use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const MEMBER_BOOTSTRAP_SCHEMA: &str = "conduit.body/member-bootstrap@2";

/// One membership entrance. Returning members carry their existing key and
/// credential, rather than an unused or fabricated new invitation.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum MemberBootstrapMaterial<'a> {
    Spawn {
        #[serde(borrow)]
        provision: PicoSpawnProvision<'a>,
    },
    Retained {
        credential: MembershipCredential,
        secret: [u8; 32],
    },
}
impl MemberBootstrapMaterial<'_> {
    pub fn secret(&self) -> &[u8; 32] {
        match self {
            Self::Spawn { provision } => &provision.secret,
            Self::Retained { secret, .. } => secret,
        }
    }
    pub fn credential(&self) -> Option<&MembershipCredential> {
        match self {
            Self::Retained { credential, .. } => Some(credential),
            _ => None,
        }
    }
    pub fn spawn(&self) -> Option<&PicoSpawnProvision<'_>> {
        match self {
            Self::Spawn { provision } => Some(provision),
            _ => None,
        }
    }
}
impl Drop for MemberBootstrapMaterial<'_> {
    fn drop(&mut self) {
        let secret = match self {
            Self::Spawn { provision } => &mut provision.secret,
            Self::Retained { secret, .. } => secret,
        };
        for byte in secret {
            unsafe { core::ptr::write_volatile(byte, 0) }
        }
    }
}

/// Target-independent membership material inside a target-owned Seed carrier.
/// Deliberately has no Debug implementation: both entrances contain private keys.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberBootstrap<'a> {
    pub schema: String,
    pub body_id: BodyId,
    pub host_id: String,
    pub image_id: String,
    pub configuration_id: String,
    pub owner: FederationPeer,
    #[serde(borrow)]
    pub admission: MemberBootstrapMaterial<'a>,
}
impl MemberBootstrap<'_> {
    /// Native facts are supplied independently by the target composition root.
    /// This checks construction scope only, not credential validity or authority.
    pub fn matches_native(&self, host: &str, image: &str, configuration: &str) -> bool {
        let id = |s: &str| !s.is_empty() && s.len() <= MAX_LIFECYCLE_ID_BYTES;
        if self.schema != MEMBER_BOOTSTRAP_SCHEMA
            || ![
                self.body_id.as_str(),
                self.host_id.as_str(),
                self.image_id.as_str(),
                self.configuration_id.as_str(),
                self.owner.host_id.as_str(),
                self.owner.boot_id.as_str(),
            ]
            .into_iter()
            .all(id)
            || self.host_id != host
            || self.image_id != image
            || self.configuration_id != configuration
            || self.owner.verifying_key == [0; 32]
            || self.owner.offer_generation.0 == 0
            || self.admission.secret() == &[0; 32]
        {
            return false;
        }
        match &self.admission {
            MemberBootstrapMaterial::Spawn { provision } => {
                validate_pico_spawn_provision(provision)
                    && provision.body_id == self.body_id.as_str()
                    && provision.image_id == self.image_id
            }
            MemberBootstrapMaterial::Retained { credential, .. } => {
                credential.body_id == self.body_id
                    && credential.host_id.as_str() == self.host_id
                    && id(credential.part_id.as_str())
                    && id(credential.credential_id.as_str())
                    && id(credential.boot_id.as_str())
            }
        }
    }
}

#[cfg(test)]
mod tests;
