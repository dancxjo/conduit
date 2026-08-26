use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use conduit_host_fabrication::{TargetSelection, MAX_PROFILE_ID_BYTES, MAX_PROFILE_ITEMS};

pub const ESP32_DESCRIPTOR_SCHEMA: &str = "conduit.host/esp32-board-descriptor@2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32BoardDescriptor {
    pub schema: String,
    pub id: String,
    pub fabrication: Esp32FabricationIdentity,
    pub target: Esp32TargetFacts,
    pub memory_regions: Vec<Esp32MemoryRegion>,
    pub flash: Esp32FlashFacts,
    pub boot: Esp32BootFacts,
    pub pins: Vec<Esp32PinFacts>,
    pub controllers: Vec<Esp32ControllerFacts>,
    pub radios: Vec<Esp32RadioFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32FabricationIdentity {
    pub board_marking: String,
    pub module_marking: String,
    pub soc_marking: String,
    pub revision: String,
    pub inspection_evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32TargetFacts {
    pub architecture: String,
    pub machine: String,
    pub chip: String,
    pub cores: u8,
    pub clock_hz: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum Esp32MemoryKind {
    InstructionRam,
    DataRam,
    ReadOnlyData,
    RtcFast,
    RtcSlow,
    ExternalRam,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32MemoryRegion {
    pub id: String,
    pub kind: Esp32MemoryKind,
    pub physical_bytes: u64,
    pub usable_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32FlashFacts {
    pub bytes: u64,
    pub mode: String,
    pub maximum_frequency_hz: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32BootFacts {
    pub image_format: String,
    pub flash_transport: String,
    pub diagnostic_transport: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Esp32PinFunction {
    DigitalInput,
    DigitalOutput,
    Adc,
    Pwm,
    I2c,
    Spi,
    Uart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32PinFacts {
    pub gpio: u8,
    pub functions: Vec<Esp32PinFunction>,
    pub reservation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Esp32ControllerKind {
    Adc,
    Pwm,
    I2c,
    Spi,
    Uart,
    Timer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32ControllerFacts {
    pub id: String,
    pub kind: Esp32ControllerKind,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Esp32RadioKind {
    Wifi24Ghz,
    BluetoothClassic,
    BluetoothLowEnergy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Esp32RadioFacts {
    pub id: String,
    pub kind: Esp32RadioKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Esp32DescriptorDiagnostic {
    UnsupportedSchema {
        found: String,
    },
    InvalidIdentity {
        field: &'static str,
        value: String,
    },
    TooManyItems {
        field: &'static str,
        found: usize,
    },
    DuplicateIdentity {
        field: &'static str,
        value: String,
    },
    ZeroCapacity {
        field: &'static str,
        identity: String,
    },
    UsableExceedsPhysical {
        region: String,
        usable: u64,
        physical: u64,
    },
    PinWithoutFunction {
        gpio: u8,
    },
    DuplicatePinFunction {
        gpio: u8,
        function: Esp32PinFunction,
    },
    TargetMismatch {
        descriptor: String,
        profile: String,
    },
    DescriptorBindingMismatch {
        expected: String,
        found: String,
    },
}

pub fn esp32_descriptor_binding(
    descriptor: &Esp32BoardDescriptor,
) -> Result<String, Esp32DescriptorDiagnostic> {
    let mut canonical = descriptor.clone();
    canonical
        .memory_regions
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical.pins.sort_by_key(|pin| pin.gpio);
    for pin in &mut canonical.pins {
        pin.functions.sort();
    }
    canonical
        .controllers
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .radios
        .sort_by(|left, right| left.id.cmp(&right.id));
    serde_json::to_vec(&canonical)
        .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
        .map_err(|error| Esp32DescriptorDiagnostic::InvalidIdentity {
            field: "descriptor.encoding",
            value: error.to_string(),
        })
}

pub fn validate_esp32_descriptor(
    descriptor: &Esp32BoardDescriptor,
) -> Result<(), Vec<Esp32DescriptorDiagnostic>> {
    let mut diagnostics = Vec::new();
    if descriptor.schema != ESP32_DESCRIPTOR_SCHEMA {
        diagnostics.push(Esp32DescriptorDiagnostic::UnsupportedSchema {
            found: descriptor.schema.clone(),
        });
    }
    for (field, value) in [
        ("id", &descriptor.id),
        (
            "fabrication.board_marking",
            &descriptor.fabrication.board_marking,
        ),
        (
            "fabrication.module_marking",
            &descriptor.fabrication.module_marking,
        ),
        (
            "fabrication.soc_marking",
            &descriptor.fabrication.soc_marking,
        ),
        ("fabrication.revision", &descriptor.fabrication.revision),
        (
            "fabrication.inspection_evidence",
            &descriptor.fabrication.inspection_evidence,
        ),
        ("target.architecture", &descriptor.target.architecture),
        ("target.machine", &descriptor.target.machine),
        ("target.chip", &descriptor.target.chip),
        ("flash.mode", &descriptor.flash.mode),
        ("boot.image_format", &descriptor.boot.image_format),
        ("boot.flash_transport", &descriptor.boot.flash_transport),
        (
            "boot.diagnostic_transport",
            &descriptor.boot.diagnostic_transport,
        ),
    ] {
        if value.is_empty() || value.len() > MAX_PROFILE_ID_BYTES {
            diagnostics.push(Esp32DescriptorDiagnostic::InvalidIdentity {
                field,
                value: value.clone(),
            });
        }
    }
    for (field, count) in [
        ("memory_regions", descriptor.memory_regions.len()),
        ("pins", descriptor.pins.len()),
        ("controllers", descriptor.controllers.len()),
        ("radios", descriptor.radios.len()),
    ] {
        if count > MAX_PROFILE_ITEMS {
            diagnostics.push(Esp32DescriptorDiagnostic::TooManyItems {
                field,
                found: count,
            });
        }
    }
    unique(
        "memory_region",
        descriptor
            .memory_regions
            .iter()
            .map(|item| item.id.as_str()),
        &mut diagnostics,
    );
    unique(
        "controller",
        descriptor.controllers.iter().map(|item| item.id.as_str()),
        &mut diagnostics,
    );
    unique(
        "radio",
        descriptor.radios.iter().map(|item| item.id.as_str()),
        &mut diagnostics,
    );
    let mut gpios = BTreeSet::new();
    for pin in &descriptor.pins {
        if !gpios.insert(pin.gpio) {
            diagnostics.push(Esp32DescriptorDiagnostic::DuplicateIdentity {
                field: "gpio",
                value: pin.gpio.to_string(),
            });
        }
        if pin.functions.is_empty() {
            diagnostics.push(Esp32DescriptorDiagnostic::PinWithoutFunction { gpio: pin.gpio });
        }
        let mut functions = BTreeSet::new();
        for function in &pin.functions {
            if !functions.insert(function.clone()) {
                diagnostics.push(Esp32DescriptorDiagnostic::DuplicatePinFunction {
                    gpio: pin.gpio,
                    function: function.clone(),
                });
            }
        }
    }
    for region in &descriptor.memory_regions {
        if region.physical_bytes == 0 {
            diagnostics.push(Esp32DescriptorDiagnostic::ZeroCapacity {
                field: "memory.physical_bytes",
                identity: region.id.clone(),
            });
        }
        if region.usable_bytes > region.physical_bytes {
            diagnostics.push(Esp32DescriptorDiagnostic::UsableExceedsPhysical {
                region: region.id.clone(),
                usable: region.usable_bytes,
                physical: region.physical_bytes,
            });
        }
    }
    for controller in &descriptor.controllers {
        if controller.channels == 0 {
            diagnostics.push(Esp32DescriptorDiagnostic::ZeroCapacity {
                field: "controller.channels",
                identity: controller.id.clone(),
            });
        }
    }
    for (field, value) in [
        ("target.cores", u64::from(descriptor.target.cores)),
        ("target.clock_hz", descriptor.target.clock_hz),
        ("flash.bytes", descriptor.flash.bytes),
        (
            "flash.maximum_frequency_hz",
            descriptor.flash.maximum_frequency_hz,
        ),
    ] {
        if value == 0 {
            diagnostics.push(Esp32DescriptorDiagnostic::ZeroCapacity {
                field,
                identity: descriptor.id.clone(),
            });
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub fn validate_esp32_target(
    target: &TargetSelection,
    descriptor: &Esp32BoardDescriptor,
) -> Result<(), Esp32DescriptorDiagnostic> {
    let descriptor_target = format!(
        "esp32/{}/{}",
        descriptor.target.architecture, descriptor.target.machine
    );
    if target.key() == descriptor_target {
        Ok(())
    } else {
        Err(Esp32DescriptorDiagnostic::TargetMismatch {
            descriptor: descriptor_target,
            profile: target.key(),
        })
    }
}

pub fn validate_esp32_binding(
    selected: &str,
    descriptor: &Esp32BoardDescriptor,
) -> Result<(), Esp32DescriptorDiagnostic> {
    let expected = esp32_descriptor_binding(descriptor)?;
    if selected == expected {
        Ok(())
    } else {
        Err(Esp32DescriptorDiagnostic::DescriptorBindingMismatch {
            expected,
            found: selected.to_owned(),
        })
    }
}

fn unique<'a>(
    field: &'static str,
    values: impl Iterator<Item = &'a str>,
    diagnostics: &mut Vec<Esp32DescriptorDiagnostic>,
) {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            diagnostics.push(Esp32DescriptorDiagnostic::DuplicateIdentity {
                field,
                value: value.to_owned(),
            });
        }
    }
}
