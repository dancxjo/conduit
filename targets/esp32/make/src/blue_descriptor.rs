//! Physical UART/GPIO Host profile, separate from the unavailable Signal fixture.
use crate::descriptor::*;
/// Conservative construction envelope for the Blue specimen. The pinned esp-hal
/// 1.1.2 ESP32 memory.x reserves ROM/cache and provides a 192KiB DRAM segment;
/// this composition exposes 128KiB plus the dedicated 96KiB secondary arena,
/// excluding ROM data/stacks. It must additionally pass its ELF's
/// actual static-DRAM inventory. It advertises no radio, ADC or spare GPIO owner.
pub fn blue_wroom32_native() -> Esp32BoardDescriptor {
    Esp32BoardDescriptor {
        schema:ESP32_DESCRIPTOR_SCHEMA.into(),id:"observed/blue-wroom32-uart0-gpio@1".into(),
        make:Esp32MakeIdentity {board_marking:"Blue (user inventory)".into(),module_marking:"ESP-WROOM-32".into(),soc_marking:"ESP32; ROM revision v3.1".into(),revision:"ROM v3.1; 4MiB flash; 40MHz crystal".into(),inspection_evidence:"Blue ROM board-info 2026-10-29 MAC b0:a7:32:28:61:78; user GPIO attachment; esp-hal1.1.2 ld/esp32/memory.x; per-build ELF inventory mandatory".into()},
        target:Esp32TargetFacts {architecture:"xtensa-lx6".into(),machine:"hw-463-esp-wroom-32".into(),chip:"ESP32 rev3.1".into(),cores:2,clock_hz:240_000_000},
        memory_regions:vec![Esp32MemoryRegion {id:"native-static-dram".into(),kind:Esp32MemoryKind::DataRam,physical_bytes:192*1024,usable_bytes:128*1024},Esp32MemoryRegion {id:"native-secondary-heap-after-rom".into(),kind:Esp32MemoryKind::DataRam,physical_bytes:98768,usable_bytes:96*1024}],
        flash:Esp32FlashFacts {bytes:4*1024*1024,mode:"external-spi; dio-40MHz application".into(),maximum_frequency_hz:40_000_000},
        boot:Esp32BootFacts {image_format:"espressif-esp32-image".into(),flash_transport:"rom-uart0-via-cp2102".into(),diagnostic_transport:"exclusive-uart0-framed-conduit".into()},
        pins:[(1,vec![Esp32PinFunction::Uart],"uart0-tx"),(3,vec![Esp32PinFunction::Uart],"uart0-rx"),(18,vec![Esp32PinFunction::DigitalInput,Esp32PinFunction::DigitalOutput],"tm1637-clock"),(19,vec![Esp32PinFunction::DigitalInput,Esp32PinFunction::DigitalOutput],"tm1637-data"),(27,vec![Esp32PinFunction::DigitalInput,Esp32PinFunction::DigitalOutput],"pulse-capture")].into_iter().map(|(gpio,functions,reservation)|Esp32PinFacts {gpio,functions,reservation:Some(reservation.into())}).collect(),
        controllers:vec![Esp32ControllerFacts {id:"uart0".into(),kind:Esp32ControllerKind::Uart,channels:1},Esp32ControllerFacts {id:"monotonic-clock".into(),kind:Esp32ControllerKind::Timer,channels:1}],
        radios:vec![],
    }
}
