//! Versioned inert artifact facts for selecting the matching bootstrap codec.
use serde::{Deserialize, Serialize};
pub const BOOTSTRAP_CONTRACT_METADATA: &str = "rp2040_bootstrap";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapContract {
    pub schema: String,
    pub image_id: String,
    pub configuration_id: String,
    pub bootstrap_schema: String,
    pub bootstrap_magic: [u8; 16],
}
impl BootstrapContract {
    pub fn for_image(image_id: &str, configuration_id: &str) -> Self {
        Self {
            schema: "conduit.rp2040/bootstrap-contract@1".into(),
            image_id: image_id.into(),
            configuration_id: configuration_id.into(),
            bootstrap_schema: super::MEMBERSHIP_SEED_SCHEMA.into(),
            bootstrap_magic: *super::MEMBERSHIP_SEED_MAGIC,
        }
    }
    /// Describes the reviewed bytes' codec, never physical membership or readiness.
    pub fn matches_image(&self, image: &str, configuration: &str) -> bool {
        self.schema == "conduit.rp2040/bootstrap-contract@1"
            && self.image_id == image
            && self.configuration_id == configuration
            && self.bootstrap_schema == super::MEMBERSHIP_SEED_SCHEMA
            && self.bootstrap_magic == *super::MEMBERSHIP_SEED_MAGIC
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_image_configuration_or_codec_refuses() {
        let mut support = BootstrapContract::for_image("image/1", "config/1");
        assert!(support.matches_image("image/1", "config/1"));
        assert!(!support.matches_image("image/2", "config/1"));
        assert!(!support.matches_image("image/1", "config/2"));
        support.bootstrap_schema = "older-codec".into();
        assert!(!support.matches_image("image/1", "config/1"));
    }
}
