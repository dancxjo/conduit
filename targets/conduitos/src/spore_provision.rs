//! Bounded validation of the Body invitation embedded in native ConduitOS media.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Deserializer};

pub const MAGIC: &[u8] = b"CONDUIT_SPORE_MEDIA@1\0";
pub const REGION_BYTES: usize = 4096;
const HEADER_BYTES: usize = 32;
const MAX_ID_BYTES: usize = 192;
const MAX_RENDEZVOUS_CANDIDATES: usize = 4;
const MAX_RENDEZVOUS_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeMediaProvision {
    pub schema: String,
    pub image_bytes: usize,
    pub spore: SporeManifest,
    pub invitation_provision: InvitationProvision,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SporeManifest {
    pub schema: String,
    pub spore_id: String,
    pub body_id: String,
    pub binding: SporeBinding,
    pub body_description_id: String,
    pub host_entry_name: String,
    pub host_configuration_id: String,
    pub profile_id: String,
    pub build_id: String,
    pub image_id: String,
    pub image_content_digest: String,
    pub target: String,
    pub output: String,
    pub fabrication: serde_json::Value,
    pub source_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SporeBinding {
    SelfJoining { invitation_id: String },
    Prejoined { part_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvitationProvision {
    pub invitation_id: String,
    pub nonce: Vec<u8>,
    pub expires_at_millis: u64,
    pub secret: Vec<u8>,
    #[serde(default)]
    pub rendezvous_candidates: Vec<RendezvousCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendezvousCandidate {
    pub schema: String,
    pub line_family: String,
    pub locator: String,
    pub body_id: String,
    pub rendezvous_identity: String,
    pub expires_at_millis: u64,
    pub maximum_attempts: u8,
    pub connection_timeout_millis: u32,
    pub authentication: RendezvousAuthentication,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendezvousAuthentication {
    pub mode: String,
    pub server_identity: String,
    #[serde(default, deserialize_with = "deserialize_optional_text")]
    pub credential_reference: Option<String>,
}

fn deserialize_optional_text<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionError {
    WrongRegion,
    UnsupportedVersion,
    InvalidLength,
    InvalidJson,
    WrongSchema,
    WrongBinding,
    InvalidIdentity,
    InvalidSecret,
    WrongImageBinding,
}

impl ProvisionError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WrongRegion => "spore-region-invalid",
            Self::UnsupportedVersion => "spore-version-unsupported",
            Self::InvalidLength => "spore-length-invalid",
            Self::InvalidJson => "spore-provision-invalid",
            Self::WrongSchema => "spore-schema-unsupported",
            Self::WrongBinding => "spore-binding-invalid",
            Self::InvalidIdentity => "spore-identity-invalid",
            Self::InvalidSecret => "spore-secret-invalid",
            Self::WrongImageBinding => "spore-image-binding-invalid",
        }
    }
}

/// Prove that an invitation was fabricated for this exact booted product.
pub fn validate_image_binding(
    provision: &NativeMediaProvision,
    target: &str,
    profile_id: &str,
    build_id: &str,
) -> Result<(), ProvisionError> {
    if provision.spore.target != target
        || provision.spore.profile_id != profile_id
        || provision.spore.build_id != build_id
        || provision.spore.output != "disk-image"
    {
        return Err(ProvisionError::WrongImageBinding);
    }
    Ok(())
}

/// Decode an initialized spore region. A reviewed blank region returns `None`.
pub fn decode(region: &[u8]) -> Result<Option<NativeMediaProvision>, ProvisionError> {
    if region.len() != REGION_BYTES || region.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(ProvisionError::WrongRegion);
    }
    let version = u32::from_le_bytes(
        region[24..28]
            .try_into()
            .map_err(|_| ProvisionError::WrongRegion)?,
    );
    let length = u32::from_le_bytes(
        region[28..32]
            .try_into()
            .map_err(|_| ProvisionError::WrongRegion)?,
    ) as usize;
    if version == 0 && length == 0 {
        return region[HEADER_BYTES..]
            .iter()
            .all(|byte| *byte == 0xff)
            .then_some(None)
            .ok_or(ProvisionError::WrongRegion);
    }
    if version != 1 {
        return Err(ProvisionError::UnsupportedVersion);
    }
    if !(1..=REGION_BYTES - HEADER_BYTES).contains(&length)
        || region[HEADER_BYTES + length..]
            .iter()
            .any(|byte| *byte != 0xff)
    {
        return Err(ProvisionError::InvalidLength);
    }
    let provision: NativeMediaProvision =
        serde_json::from_slice(&region[HEADER_BYTES..HEADER_BYTES + length])
            .map_err(|_| ProvisionError::InvalidJson)?;
    validate(&provision)?;
    Ok(Some(provision))
}

fn validate(provision: &NativeMediaProvision) -> Result<(), ProvisionError> {
    if provision.schema != "conduit.spore/native-media-provision@1"
        || provision.spore.schema != "conduit.body/spore-manifest@2"
    {
        return Err(ProvisionError::WrongSchema);
    }
    let invitation_id = match &provision.spore.binding {
        SporeBinding::SelfJoining { invitation_id } => invitation_id,
        SporeBinding::Prejoined { .. } => return Err(ProvisionError::WrongBinding),
    };
    if invitation_id != &provision.invitation_provision.invitation_id {
        return Err(ProvisionError::WrongBinding);
    }
    for value in [
        provision.spore.spore_id.as_str(),
        provision.spore.body_id.as_str(),
        provision.spore.profile_id.as_str(),
        provision.spore.build_id.as_str(),
        provision.spore.image_id.as_str(),
        invitation_id.as_str(),
    ] {
        if value.is_empty() || value.len() > MAX_ID_BYTES {
            return Err(ProvisionError::InvalidIdentity);
        }
    }
    if provision.invitation_provision.nonce.len() != 32
        || provision.invitation_provision.secret.len() != 32
        || provision
            .invitation_provision
            .nonce
            .iter()
            .all(|byte| *byte == 0)
        || provision
            .invitation_provision
            .secret
            .iter()
            .all(|byte| *byte == 0)
        || provision.invitation_provision.expires_at_millis == 0
    {
        return Err(ProvisionError::InvalidSecret);
    }
    if provision.invitation_provision.rendezvous_candidates.len() > MAX_RENDEZVOUS_CANDIDATES
        || provision
            .invitation_provision
            .rendezvous_candidates
            .iter()
            .any(|candidate| !valid_rendezvous_candidate(candidate, provision))
    {
        return Err(ProvisionError::InvalidIdentity);
    }
    Ok(())
}

fn valid_rendezvous_candidate(
    candidate: &RendezvousCandidate,
    provision: &NativeMediaProvision,
) -> bool {
    candidate.schema == "conduit.body/rendezvous-candidate@1"
        && candidate.body_id == provision.spore.body_id
        && bounded_text(&candidate.line_family)
        && bounded_text(&candidate.locator)
        && bounded_text(&candidate.rendezvous_identity)
        && candidate.expires_at_millis <= provision.invitation_provision.expires_at_millis
        && (1..=8).contains(&candidate.maximum_attempts)
        && (1..=60_000).contains(&candidate.connection_timeout_millis)
        && matches!(
            candidate.authentication.mode.as_str(),
            "mutual-tls" | "conduit-authenticated-line"
        )
        && bounded_text(&candidate.authentication.server_identity)
        && candidate
            .authentication
            .credential_reference
            .as_deref()
            .is_none_or(bounded_text)
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_RENDEZVOUS_TEXT_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, vec};

    fn region(document: serde_json::Value) -> Vec<u8> {
        let encoded = serde_json::to_vec(&document).unwrap();
        let mut bytes = vec![0xff; REGION_BYTES];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[24..28].copy_from_slice(&1_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&(encoded.len() as u32).to_le_bytes());
        bytes[HEADER_BYTES..HEADER_BYTES + encoded.len()].copy_from_slice(&encoded);
        bytes
    }

    fn fixture() -> serde_json::Value {
        serde_json::json!({
            "schema":"conduit.spore/native-media-provision@1", "image_bytes":8192,
            "spore": {"schema":"conduit.body/spore-manifest@2", "spore_id":"spore/one",
                "body_id":"body/one", "binding":{"mode":"self-joining","invitation_id":"invitation/one"},
                "body_description_id":"description/one", "host_entry_name":"host", "host_configuration_id":"config/one",
                "profile_id":"profile/one", "build_id":"build/one", "image_id":"image/one",
                "image_content_digest":format!("sha256:{}", "1".repeat(64)), "target":"conduitos/x86_64/pc",
                "output":"disk-image", "fabrication":{}, "source_identity":"source/one"},
            "invitation_provision":{"invitation_id":"invitation/one", "nonce":vec![1;32],
                "expires_at_millis":1_800_000_000_000_u64, "secret":vec![2;32],
                "rendezvous_candidates":[{
                    "schema":"conduit.body/rendezvous-candidate@1", "line_family":"authenticated-conduit-line",
                    "locator":"relay.example.test:443", "body_id":"body/one", "rendezvous_identity":"rendezvous/one",
                    "expires_at_millis":1_800_000_000_000_u64, "maximum_attempts":3, "connection_timeout_millis":30_000,
                    "authentication":{"mode":"conduit-authenticated-line", "server_identity":"server/one"}
                }]}
        })
    }

    #[test]
    fn blank_is_absent_and_initialized_region_is_validated() {
        let mut blank = vec![0xff; REGION_BYTES];
        blank[..MAGIC.len()].copy_from_slice(MAGIC);
        blank[24..32].fill(0);
        assert_eq!(decode(&blank), Ok(None));
        let decoded = decode(&region(fixture())).unwrap().unwrap();
        assert_eq!(decoded.spore.body_id, "body/one");
    }

    #[test]
    fn mismatched_invitation_and_secret_are_refused() {
        let mut wrong = fixture();
        wrong["invitation_provision"]["invitation_id"] = "invitation/two".into();
        assert_eq!(decode(&region(wrong)), Err(ProvisionError::WrongBinding));
        let mut weak = fixture();
        weak["invitation_provision"]["secret"] = serde_json::json!(vec![0; 32]);
        assert_eq!(decode(&region(weak)), Err(ProvisionError::InvalidSecret));
    }

    #[test]
    fn exact_booted_product_binding_is_required() {
        let decoded = decode(&region(fixture())).unwrap().unwrap();
        assert_eq!(
            validate_image_binding(&decoded, "conduitos/x86_64/pc", "profile/one", "build/one"),
            Ok(())
        );
        assert_eq!(
            validate_image_binding(
                &decoded,
                "conduitos/aarch64/virt",
                "profile/one",
                "build/one"
            ),
            Err(ProvisionError::WrongImageBinding)
        );
    }
}
