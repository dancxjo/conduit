//! Product contract for carrying an already-built Body-bound artifact.
//!
//! Carrier realization is distinct from fabrication and from later Boot, join,
//! or membership observation. Target catalogs declare these descriptors; an
//! installed product may execute only a matching reviewed implementation.

use serde::{Deserialize, Serialize};

pub const DEPLOYMENT_CARRIER_SCHEMA: &str = "conduit.carrier/descriptor@1";
pub const DEPLOYMENT_RECEIPT_SCHEMA: &str = "conduit.carrier/realization@1";
pub const MAXIMUM_CARRIER_TEXT_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentCarrierKind {
    ArtifactDownload,
    MicrocontrollerFlash,
    RemovableWholeDeviceWrite,
    VirtualMachineLaunch,
    NativeInstallStart,
    NetworkBootServe,
}

impl DeploymentCarrierKind {
    pub const fn consequential(self) -> bool {
        !matches!(self, Self::ArtifactDownload)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentCarrierDescriptor {
    pub schema: String,
    pub carrier_id: String,
    pub target_id: String,
    pub kind: DeploymentCarrierKind,
    pub implementation_id: String,
    pub maximum_artifact_bytes: u64,
    pub requires_explicit_authority: bool,
    pub verifies_written_bytes: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyBoundArtifactIdentity {
    pub target_id: String,
    pub image_id: String,
    pub image_content_sha256: String,
    pub spore_id: String,
    pub artifact_content_sha256: String,
    pub artifact_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentCarrierRequest<'a> {
    pub carrier_id: &'a str,
    pub target_id: &'a str,
    pub artifact: &'a BodyBoundArtifactIdentity,
    pub explicit_authority: bool,
    /// Required for a whole-device writer and matched exactly by its helper.
    pub destructive_destination: Option<&'a str>,
    pub confirmed_destination: Option<&'a str>,
    pub destination_is_removable: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CarrierTerminal {
    Completed,
    Interrupted,
    Refused,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentRealizationReceipt {
    pub schema: String,
    pub carrier_id: String,
    pub implementation_id: String,
    pub target_id: String,
    pub spore_id: String,
    pub artifact_content_sha256: String,
    pub terminal: CarrierTerminal,
    pub bytes_realized: u64,
    pub byte_verification_completed: bool,
    pub explicit_authority_observed: bool,
    pub boot_observed: bool,
    pub join_observed: bool,
    pub membership_admitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeploymentCarrierRefusal {
    InvalidDescriptor,
    WrongCarrier,
    WrongTarget,
    InvalidArtifactIdentity,
    ArtifactBound,
    AuthorityRequired,
    AmbiguousDestination,
    NonRemovableDestination,
    InvalidReceipt,
    UnprovenLaterStage,
}

impl DeploymentCarrierDescriptor {
    pub fn validate_request(
        &self,
        request: &DeploymentCarrierRequest<'_>,
    ) -> Result<(), DeploymentCarrierRefusal> {
        if self.schema != DEPLOYMENT_CARRIER_SCHEMA
            || !bounded(&self.carrier_id)
            || !bounded(&self.target_id)
            || !bounded(&self.implementation_id)
            || self.maximum_artifact_bytes == 0
            || (self.kind.consequential() && !self.requires_explicit_authority)
        {
            return Err(DeploymentCarrierRefusal::InvalidDescriptor);
        }
        if request.carrier_id != self.carrier_id {
            return Err(DeploymentCarrierRefusal::WrongCarrier);
        }
        if request.target_id != self.target_id || request.artifact.target_id != self.target_id {
            return Err(DeploymentCarrierRefusal::WrongTarget);
        }
        if !artifact_identity_valid(request.artifact) {
            return Err(DeploymentCarrierRefusal::InvalidArtifactIdentity);
        }
        if request.artifact.artifact_bytes > self.maximum_artifact_bytes {
            return Err(DeploymentCarrierRefusal::ArtifactBound);
        }
        if self.requires_explicit_authority && !request.explicit_authority {
            return Err(DeploymentCarrierRefusal::AuthorityRequired);
        }
        if self.kind == DeploymentCarrierKind::RemovableWholeDeviceWrite {
            if request.destructive_destination.is_none()
                || request.destructive_destination != request.confirmed_destination
            {
                return Err(DeploymentCarrierRefusal::AmbiguousDestination);
            }
            if request.destination_is_removable != Some(true) {
                return Err(DeploymentCarrierRefusal::NonRemovableDestination);
            }
        }
        Ok(())
    }

    pub fn validate_receipt(
        &self,
        artifact: &BodyBoundArtifactIdentity,
        receipt: &DeploymentRealizationReceipt,
    ) -> Result<(), DeploymentCarrierRefusal> {
        if receipt.schema != DEPLOYMENT_RECEIPT_SCHEMA
            || receipt.carrier_id != self.carrier_id
            || receipt.implementation_id != self.implementation_id
            || receipt.target_id != self.target_id
            || receipt.spore_id != artifact.spore_id
            || receipt.artifact_content_sha256 != artifact.artifact_content_sha256
            || receipt.bytes_realized > artifact.artifact_bytes
            || receipt.explicit_authority_observed != self.requires_explicit_authority
        {
            return Err(DeploymentCarrierRefusal::InvalidReceipt);
        }
        if receipt.boot_observed || receipt.join_observed || receipt.membership_admitted {
            return Err(DeploymentCarrierRefusal::UnprovenLaterStage);
        }
        if receipt.terminal == CarrierTerminal::Completed
            && (receipt.bytes_realized != artifact.artifact_bytes
                || (self.verifies_written_bytes && !receipt.byte_verification_completed))
        {
            return Err(DeploymentCarrierRefusal::InvalidReceipt);
        }
        Ok(())
    }
}

fn artifact_identity_valid(artifact: &BodyBoundArtifactIdentity) -> bool {
    bounded(&artifact.target_id)
        && bounded(&artifact.image_id)
        && bounded(&artifact.spore_id)
        && valid_sha256(&artifact.image_content_sha256)
        && valid_sha256(&artifact.artifact_content_sha256)
        && artifact.artifact_bytes > 0
}

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_CARRIER_TEXT_BYTES
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact() -> BodyBoundArtifactIdentity {
        BodyBoundArtifactIdentity {
            target_id: "conduitos/x86_64/pc".into(),
            image_id: "image/reviewed".into(),
            image_content_sha256: format!("sha256:{}", "1".repeat(64)),
            spore_id: "spore/body-one/host-two".into(),
            artifact_content_sha256: format!("sha256:{}", "2".repeat(64)),
            artifact_bytes: 4096,
        }
    }

    fn writer() -> DeploymentCarrierDescriptor {
        DeploymentCarrierDescriptor {
            schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
            carrier_id: "conduit-carrier/removable-whole-device@1".into(),
            target_id: "conduitos/x86_64/pc".into(),
            kind: DeploymentCarrierKind::RemovableWholeDeviceWrite,
            implementation_id: "conduit-helper/linux-removable-writer@1".into(),
            maximum_artifact_bytes: 8192,
            requires_explicit_authority: true,
            verifies_written_bytes: true,
        }
    }

    #[test]
    fn destructive_writer_requires_exact_removable_confirmation() {
        let artifact = artifact();
        let descriptor = writer();
        let mut request = DeploymentCarrierRequest {
            carrier_id: &descriptor.carrier_id,
            target_id: &descriptor.target_id,
            artifact: &artifact,
            explicit_authority: true,
            destructive_destination: Some("device/by-id/usb-reviewed"),
            confirmed_destination: Some("device/by-id/usb-reviewed"),
            destination_is_removable: Some(true),
        };
        assert_eq!(descriptor.validate_request(&request), Ok(()));
        request.confirmed_destination = Some("device/by-id/other");
        assert_eq!(
            descriptor.validate_request(&request),
            Err(DeploymentCarrierRefusal::AmbiguousDestination)
        );
    }

    #[test]
    fn interrupted_realization_cannot_be_promoted_to_boot_or_membership() {
        let artifact = artifact();
        let descriptor = writer();
        let mut receipt = DeploymentRealizationReceipt {
            schema: DEPLOYMENT_RECEIPT_SCHEMA.into(),
            carrier_id: descriptor.carrier_id.clone(),
            implementation_id: descriptor.implementation_id.clone(),
            target_id: descriptor.target_id.clone(),
            spore_id: artifact.spore_id.clone(),
            artifact_content_sha256: artifact.artifact_content_sha256.clone(),
            terminal: CarrierTerminal::Interrupted,
            bytes_realized: 2048,
            byte_verification_completed: false,
            explicit_authority_observed: true,
            boot_observed: false,
            join_observed: false,
            membership_admitted: false,
        };
        assert_eq!(descriptor.validate_receipt(&artifact, &receipt), Ok(()));
        receipt.boot_observed = true;
        assert_eq!(
            descriptor.validate_receipt(&artifact, &receipt),
            Err(DeploymentCarrierRefusal::UnprovenLaterStage)
        );
    }

    #[test]
    fn artifact_download_is_available_without_consequential_authority() {
        let artifact = artifact();
        let descriptor = DeploymentCarrierDescriptor {
            schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
            carrier_id: "conduit-carrier/download@1".into(),
            target_id: artifact.target_id.clone(),
            kind: DeploymentCarrierKind::ArtifactDownload,
            implementation_id: "conduit/download@1".into(),
            maximum_artifact_bytes: 8192,
            requires_explicit_authority: false,
            verifies_written_bytes: false,
        };
        assert_eq!(
            descriptor.validate_request(&DeploymentCarrierRequest {
                carrier_id: &descriptor.carrier_id,
                target_id: &descriptor.target_id,
                artifact: &artifact,
                explicit_authority: false,
                destructive_destination: None,
                confirmed_destination: None,
                destination_is_removable: None,
            }),
            Ok(())
        );
    }
}
