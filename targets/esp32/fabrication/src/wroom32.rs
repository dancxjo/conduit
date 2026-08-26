use crate::descriptor::{
    Esp32BoardDescriptor, Esp32BootFacts, Esp32FabricationIdentity, Esp32FlashFacts,
    Esp32MemoryKind, Esp32MemoryRegion, Esp32RadioFacts, Esp32RadioKind, Esp32TargetFacts,
    ESP32_DESCRIPTOR_SCHEMA,
};

/// One conservative descriptor for the physically inspected HW-463 specimen.
///
/// Empty pin and controller inventories are intentional: the attached board
/// established no exact board routing or usable peripheral reservations. Those
/// facilities therefore remain unavailable to PROFILE selection.
pub fn hw463_esp_wroom_32_sample() -> Esp32BoardDescriptor {
    Esp32BoardDescriptor {
        schema: ESP32_DESCRIPTOR_SCHEMA.into(),
        id: "observed/hw-463-esp-wroom-32@1".into(),
        fabrication: Esp32FabricationIdentity {
            board_marking: "HW-463".into(),
            module_marking: "ESP-WROOM-32; FCC ID:2AC7Z-ESP32WROOM32; 211-161007".into(),
            soc_marking: "ESP32-D0WD-V3".into(),
            revision: "soc-v3.1; board-unmarked".into(),
            inspection_evidence: concat!(
                "github:dancxjo/conduit#1221-comment-5304376135; ",
                "espressif:esp32-v5.3; esp32-wroom-32-v3.7"
            )
            .into(),
        },
        target: Esp32TargetFacts {
            architecture: "xtensa-lx6".into(),
            machine: "hw-463-esp-wroom-32".into(),
            chip: "ESP32-D0WD-V3".into(),
            cores: 2,
            clock_hz: 240_000_000,
        },
        memory_regions: vec![
            unavailable_memory("sram", Esp32MemoryKind::DataRam, 520 * 1024),
            unavailable_memory("rtc-fast", Esp32MemoryKind::RtcFast, 8 * 1024),
            unavailable_memory("rtc-slow", Esp32MemoryKind::RtcSlow, 8 * 1024),
        ],
        flash: Esp32FlashFacts {
            bytes: 4 * 1024 * 1024,
            mode: "external-spi; image-mode-unselected".into(),
            maximum_frequency_hz: 80_000_000,
        },
        boot: Esp32BootFacts {
            image_format: "espressif-esp32-image".into(),
            flash_transport: "rom-uart0-via-cp2102".into(),
            diagnostic_transport: "rom-uart0-via-cp2102".into(),
        },
        pins: Vec::new(),
        controllers: Vec::new(),
        radios: vec![
            Esp32RadioFacts {
                id: "wifi-2.4-ghz".into(),
                kind: Esp32RadioKind::Wifi24Ghz,
            },
            Esp32RadioFacts {
                id: "bluetooth-classic".into(),
                kind: Esp32RadioKind::BluetoothClassic,
            },
            Esp32RadioFacts {
                id: "bluetooth-low-energy".into(),
                kind: Esp32RadioKind::BluetoothLowEnergy,
            },
        ],
    }
}

fn unavailable_memory(id: &str, kind: Esp32MemoryKind, physical_bytes: u64) -> Esp32MemoryRegion {
    Esp32MemoryRegion {
        id: id.into(),
        kind,
        physical_bytes,
        usable_bytes: 0,
    }
}
