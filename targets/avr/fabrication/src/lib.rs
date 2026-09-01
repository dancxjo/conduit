use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub const PACKAGE_ID: &str = "conduit-host-avr-promicro@1";
pub const PACKAGE_REVISION: u32 = 1;
pub const TARGET_ID: &str = "avr/avr5/sparkfun-pro-micro-atmega32u4-5v-16mhz";
pub const FQBN: &str = "SparkFun:avr:promicro:cpu=16MHzatmega32U4";
pub const BOARD: &str = "sparkfun-pro-micro-5v-16mhz";
pub const MCU: &str = "atmega32u4";
pub const CLOCK_HZ: u64 = 16_000_000;
pub const FLASH_BYTES: u64 = 32_768;
pub const APPLICATION_FLASH_BYTES: u64 = 28_672;
pub const SPORE_REGION_BYTES: u64 = 1_024;
pub const SPORE_REGION_START: u64 = APPLICATION_FLASH_BYTES - SPORE_REGION_BYTES;
pub const BOOT_REGION_START: u64 = APPLICATION_FLASH_BYTES;
pub const BOOT_REGION_BYTES: u64 = FLASH_BYTES - APPLICATION_FLASH_BYTES;
pub const SRAM_BYTES: u64 = 2_560;
pub const ARTIFACT_FORMAT: &str = "intel-hex";
pub const BOOTLOADER: &str = "caterina";
pub const BOOTLOADER_PROTOCOL: &str = "avr109";
pub const RESET_TRANSITION: &str = "1200-baud-touch-then-fresh-port";
pub const BUILDER_ADAPTER: &str = "conduit-host-avr/build-intel-hex@1";
pub const FABRICATION_DESCRIPTOR: &str =
    "sparkfun-pro-micro/atmega32u4/5v/16mhz/caterina-avr109/app-0x0000-0x6bff/spore-0x6c00-0x6fff";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProMicroFacts {
    pub target_id: &'static str,
    pub fqbn: &'static str,
    pub board: &'static str,
    pub mcu: &'static str,
    pub clock_hz: u64,
    pub flash_bytes: u64,
    pub application_flash_bytes: u64,
    pub boot_region_start: u64,
    pub boot_region_bytes: u64,
    pub sram_bytes: u64,
    pub artifact_format: &'static str,
    pub bootloader: &'static str,
    pub bootloader_protocol: &'static str,
    pub reset_transition: &'static str,
}

pub const PRO_MICRO: ProMicroFacts = ProMicroFacts {
    target_id: TARGET_ID,
    fqbn: FQBN,
    board: BOARD,
    mcu: MCU,
    clock_hz: CLOCK_HZ,
    flash_bytes: FLASH_BYTES,
    application_flash_bytes: APPLICATION_FLASH_BYTES,
    boot_region_start: BOOT_REGION_START,
    boot_region_bytes: BOOT_REGION_BYTES,
    sram_bytes: SRAM_BYTES,
    artifact_format: ARTIFACT_FORMAT,
    bootloader: BOOTLOADER,
    bootloader_protocol: BOOTLOADER_PROTOCOL,
    reset_transition: RESET_TRANSITION,
};

pub struct AvrProMicroFabricationPackage;

impl HostFabricationPackage for AvrProMicroFabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: PACKAGE_ID.into(),
            package_revision: PACKAGE_REVISION,
            catalog: Default::default(),
            targets: vec![TargetDescriptor {
                label: "SparkFun Pro Micro · ATmega32U4 · 5 V / 16 MHz".into(),
                family: "avr".into(),
                architecture: "avr5".into(),
                machine: "sparkfun-pro-micro-atmega32u4-5v-16mhz".into(),
                board: Some(BOARD.into()),
                os: None,
                host_core: "host-core/conduitos@1".into(),
                presenter: None,
                host_operations: Vec::new(),
                toolchain_identity: "arduino-cli:1.5.1+arduino-avr:1.8.8+sparkfun-avr:1.1.13"
                    .into(),
                builder_adapter: BUILDER_ADAPTER.into(),
                // Browser Caterina/AVR109 reset and fresh-port acquisition are not proved.
                deployment_adapter: None,
                outputs: vec![SporeOutputKind::IntelHex],
                default_output: SporeOutputKind::IntelHex,
                post_build_actions: vec![PostBuildAction::Flash, PostBuildAction::Boot],
                fabrication_descriptors: vec![FABRICATION_DESCRIPTOR.into()],
                maxima: HostBounds {
                    static_memory_bytes: SRAM_BYTES,
                    heap_arena_bytes: 1,
                    queue_items: 8,
                    buffered_bytes: 512,
                    active_instances: 4,
                    operation_slots: 4,
                    timer_slots: 4,
                    line_sessions: 1,
                    evidence_items: 16,
                },
            }],
            offers: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_exposes_only_one_exact_pro_micro() {
        let FabricationContribution::Anchor(anchor) = AvrProMicroFabricationPackage.contribution()
        else {
            panic!("Pro Micro package must be an anchor")
        };
        assert_eq!(anchor.targets.len(), 1);
        let target = &anchor.targets[0];
        assert_eq!(target.key(), TARGET_ID);
        assert_eq!(target.outputs, [SporeOutputKind::IntelHex]);
        assert_eq!(target.deployment_adapter, None);
        assert_eq!(target.maxima.static_memory_bytes, SRAM_BYTES);
        assert_eq!(APPLICATION_FLASH_BYTES + BOOT_REGION_BYTES, FLASH_BYTES);
        assert_eq!(SPORE_REGION_START + SPORE_REGION_BYTES, BOOT_REGION_START);
    }
}
