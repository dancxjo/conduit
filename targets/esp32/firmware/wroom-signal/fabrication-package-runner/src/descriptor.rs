use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::selection::{base_labels, FeatureProjection};

pub const DESCRIPTOR_RELATIVE_PATH: &str =
    "targets/esp32/firmware/wroom-signal/fabrication-package.json";
pub const PACKAGE_RELATIVE_PATH: &str = "targets/esp32/firmware/wroom-signal";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FabricationPackageDescriptor {
    pub schema: String,
    pub package: String,
    pub revision: u32,
    pub chip: String,
    pub board_descriptor: String,
    pub target: String,
    pub toolchain: String,
    pub toolchain_name: String,
    pub toolchain_action: String,
    pub linker_adapter: String,
    pub linker_command: String,
    pub builder_adapter: String,
    pub minimal_features: Vec<String>,
    pub full_features: Vec<String>,
    pub minimal_bases: Vec<String>,
    pub full_bases: Vec<String>,
    pub artifact: String,
}

impl FabricationPackageDescriptor {
    pub fn read(repo_root: &Path) -> Result<(Self, Vec<u8>), Box<dyn std::error::Error>> {
        let bytes = fs::read(repo_root.join(DESCRIPTOR_RELATIVE_PATH))?;
        let descriptor = serde_json::from_slice(&bytes)?;
        Ok((descriptor, bytes))
    }

    pub fn validate(
        &self,
        projection: &FeatureProjection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.schema != "conduit.host/fabrication-package/esp32-firmware@1"
            || self.package != "conduit-esp32-wroom-signal"
            || self.revision != 1
            || self.chip != "esp32"
            || self.board_descriptor != "observed/hw-463-esp-wroom-32@1"
            || self.target != "xtensa-esp32-none-elf"
            || self.toolchain != "esp-rs/rust-build@v1.91.1.0"
            || self.toolchain_name != "esp-conduit-1.91.1"
            || self.toolchain_action
                != "esp-rs/xtensa-toolchain@ec6d36527049a7f4fb2cb0c1a644668c1bb8a2a4"
            || self.linker_adapter != "xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin"
            || self.linker_command != "xtensa-esp32-elf-gcc"
            || self.builder_adapter != "conduit-host-esp32/build-image@1"
            || self.artifact != "target/xtensa-esp32-none-elf/release/conduit-esp32-wroom-signal"
        {
            return Err("exact ESP32 fabrication-package descriptor identity refused".into());
        }
        if self.minimal_bases != base_labels(&projection.minimal_bases)
            || self.full_bases != base_labels(&projection.full_bases)
        {
            return Err("ESP32 descriptor Base selections do not match checked selections".into());
        }
        if self.minimal_features != projection.minimal_features
            || self.full_features != projection.full_features
        {
            return Err("ESP32 descriptor features do not match derived Base closure".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::checked_feature_projection;

    fn descriptor() -> FabricationPackageDescriptor {
        FabricationPackageDescriptor {
            schema: "conduit.host/fabrication-package/esp32-firmware@1".into(),
            package: "conduit-esp32-wroom-signal".into(),
            revision: 1,
            chip: "esp32".into(),
            board_descriptor: "observed/hw-463-esp-wroom-32@1".into(),
            target: "xtensa-esp32-none-elf".into(),
            toolchain: "esp-rs/rust-build@v1.91.1.0".into(),
            toolchain_name: "esp-conduit-1.91.1".into(),
            toolchain_action: "esp-rs/xtensa-toolchain@ec6d36527049a7f4fb2cb0c1a644668c1bb8a2a4"
                .into(),
            linker_adapter: "xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin".into(),
            linker_command: "xtensa-esp32-elf-gcc".into(),
            builder_adapter: "conduit-host-esp32/build-image@1".into(),
            minimal_features: vec!["kernel-signal".into()],
            full_features: vec!["bluetooth".into(), "kernel-signal".into()],
            minimal_bases: vec!["kernel-signal".into()],
            full_bases: vec!["kernel-signal".into(), "bluetooth-le-gatt".into()],
            artifact: "target/xtensa-esp32-none-elf/release/conduit-esp32-wroom-signal".into(),
        }
    }

    #[test]
    fn exact_revision_one_descriptor_is_accepted() {
        descriptor()
            .validate(&checked_feature_projection().unwrap())
            .unwrap();
    }

    #[test]
    fn wrong_identity_or_projection_is_refused() {
        let projection = checked_feature_projection().unwrap();
        let mut wrong_chip = descriptor();
        wrong_chip.chip = "esp32-c3".into();
        assert!(wrong_chip.validate(&projection).is_err());

        let mut wrong_features = descriptor();
        wrong_features.minimal_features.clear();
        assert!(wrong_features.validate(&projection).is_err());

        let mut wrong_builder = descriptor();
        wrong_builder.builder_adapter = "generic-xtask".into();
        assert!(wrong_builder.validate(&projection).is_err());

        let mut ambient_toolchain = descriptor();
        ambient_toolchain.toolchain_name = "esp".into();
        assert!(ambient_toolchain.validate(&projection).is_err());

        let mut ambient_linker = descriptor();
        ambient_linker.linker_adapter = "/usr/bin".into();
        assert!(ambient_linker.validate(&projection).is_err());
    }
}
