use core::{fmt, str::FromStr};

use conduit_host_fabrication::{HostBounds, PostBuildAction, SporeOutputKind, TargetDescriptor};
use serde::{Deserialize, Serialize};

use crate::{
    descriptor::{esp32_descriptor_binding, validate_esp32_descriptor, Esp32BoardDescriptor},
    wroom32::hw463_esp_wroom_32_sample,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Esp32FamilyTarget {
    Wroom,
    C3,
    S3,
}

impl Esp32FamilyTarget {
    pub const ALL: [Self; 3] = [Self::Wroom, Self::C3, Self::S3];

    pub const fn facts(self) -> &'static Esp32FamilyTargetFacts {
        match self {
            Self::Wroom => &WROOM,
            Self::C3 => &C3,
            Self::S3 => &S3,
        }
    }

    pub fn board_descriptor(self) -> Esp32BoardDescriptor {
        match self {
            Self::Wroom => hw463_esp_wroom_32_sample(),
            Self::C3 => serde_json::from_str(include_str!(
                "../../firmware/c3-signal/board-descriptor.json"
            ))
            .expect("package-owned C3 descriptor must decode"),
            Self::S3 => serde_json::from_str(include_str!(
                "../../firmware/s3-signal/board-descriptor.json"
            ))
            .expect("package-owned S3 descriptor must decode"),
        }
    }

    pub fn target_descriptor(self) -> TargetDescriptor {
        let facts = self.facts();
        let descriptor = self.board_descriptor();
        validate_esp32_descriptor(&descriptor)
            .expect("package-owned ESP32 descriptor must remain valid");
        let descriptor_binding = esp32_descriptor_binding(&descriptor)
            .expect("package-owned ESP32 descriptor must have an exact binding");
        TargetDescriptor {
            label: facts.label.into(),
            family: "esp32".into(),
            architecture: facts.architecture.into(),
            machine: facts.machine.into(),
            board: Some(facts.machine.into()),
            os: None,
            host_core: "host-core/conduitos@1".into(),
            presenter: None,
            host_operations: Vec::new(),
            toolchain_identity: facts.toolchain_identity.into(),
            builder_adapter: facts.builder_adapter.into(),
            deployment_adapter: facts.deployment_adapter.map(str::to_owned),
            outputs: vec![SporeOutputKind::Esp32Image],
            default_output: SporeOutputKind::Esp32Image,
            post_build_actions: facts.post_build_actions.to_vec(),
            fabrication_descriptors: vec![descriptor_binding],
            maxima: HostBounds {
                static_memory_bytes: 64 * 1024 * 1024,
                heap_arena_bytes: 64 * 1024 * 1024,
                queue_items: 4096,
                buffered_bytes: 4 * 1024 * 1024,
                active_instances: 4096,
                operation_slots: 4096,
                timer_slots: 4096,
                line_sessions: 4096,
                evidence_items: 4096,
            },
        }
    }
}

impl FromStr for Esp32FamilyTarget {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "wroom" => Ok(Self::Wroom),
            "c3" => Ok(Self::C3),
            "s3" => Ok(Self::S3),
            _ => Err(format!("unsupported ESP32 family target {value:?}")),
        }
    }
}

impl fmt::Display for Esp32FamilyTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.facts().selector)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Esp32FamilyTargetFacts {
    pub selector: &'static str,
    pub label: &'static str,
    pub architecture: &'static str,
    pub machine: &'static str,
    pub package_dir: &'static str,
    pub rust_toolchain: &'static str,
    pub cargo_target: &'static str,
    pub artifact_name: &'static str,
    pub espflash_chip: &'static str,
    pub usb_serial: &'static str,
    pub toolchain_identity: &'static str,
    pub builder_adapter: &'static str,
    pub deployment_adapter: Option<&'static str>,
    pub post_build_actions: &'static [PostBuildAction],
}

const FLASH_AND_BOOT: &[PostBuildAction] = &[PostBuildAction::Flash, PostBuildAction::Boot];

const WROOM: Esp32FamilyTargetFacts = Esp32FamilyTargetFacts {
    selector: "wroom",
    label: "ESP32-WROOM-32",
    architecture: "xtensa-lx6",
    machine: "hw-463-esp-wroom-32",
    package_dir: "targets/esp32/firmware/wroom-signal",
    rust_toolchain: "esp-conduit-1.91.1",
    cargo_target: "xtensa-esp32-none-elf",
    artifact_name: "conduit-esp32-wroom-signal",
    espflash_chip: "esp32",
    usb_serial: "0001",
    toolchain_identity: "esp-rs/rust-build@v1.91.1.0",
    builder_adapter: "conduit-host-esp32/build-image@1",
    deployment_adapter: Some("conduit-host-esp32/flash-wroom@1"),
    post_build_actions: FLASH_AND_BOOT,
};

const C3: Esp32FamilyTargetFacts = Esp32FamilyTargetFacts {
    selector: "c3",
    label: "ESP32-C3",
    architecture: "riscv32imc",
    machine: "usb-dcf8355d-esp32-c3",
    package_dir: "targets/esp32/firmware/c3-signal",
    rust_toolchain: "1.91.1",
    cargo_target: "riscv32imc-unknown-none-elf",
    artifact_name: "conduit-esp32-c3-signal",
    espflash_chip: "esp32c3",
    usb_serial: "dcf8355da19ded11a7205f84e259fb3e",
    toolchain_identity: "rust/riscv32imc@1.91.1",
    builder_adapter: "conduit-host-esp32/build-c3-image@1",
    deployment_adapter: Some("conduit-host-esp32/flash-c3@1"),
    post_build_actions: FLASH_AND_BOOT,
};

const S3: Esp32FamilyTargetFacts = Esp32FamilyTargetFacts {
    selector: "s3",
    label: "ESP32-S3",
    architecture: "xtensa-lx7",
    machine: "usb-54e2006398-esp32-s3",
    package_dir: "targets/esp32/firmware/s3-signal",
    rust_toolchain: "esp-conduit-1.91.1",
    cargo_target: "xtensa-esp32s3-none-elf",
    artifact_name: "conduit-esp32-s3-signal",
    espflash_chip: "esp32s3",
    usb_serial: "54E2006398",
    toolchain_identity: "esp-rs/rust-build@v1.91.1.0",
    builder_adapter: "conduit-host-esp32/build-s3-image@1",
    deployment_adapter: Some("conduit-host-esp32/flash-s3@1"),
    post_build_actions: FLASH_AND_BOOT,
};
