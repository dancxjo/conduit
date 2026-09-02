//! Bounded immutable fabrication truth linked into the freestanding artifact.

pub const FABRICATION_SCHEMA: &str = "conduit.conduitos/fabrication-record@1";
pub const MAX_ID_BYTES: usize = 160;
pub const IMPL_TIME_TICK: u16 = 1 << 0;
pub const IMPL_TICK_PRESENTATION: u16 = 1 << 1;
pub const IMPL_TEXT_LITERAL: u16 = 1 << 2;
pub const IMPL_TEXT_UPPER: u16 = 1 << 3;
pub const IMPL_TEXT_PRESENTATION: u16 = 1 << 4;
pub const IMPL_KEYBOARD: u16 = 1 << 5;
pub const IMPL_PC_SPEAKER: u16 = 1 << 6;
pub const IMPL_OPL2: u16 = 1 << 7;
pub const IMPL_NATIVE_PRESENTER: u16 = 1 << 8;
pub const IMPL_LINEAR_PRESENTER: u16 = 1 << 9;
pub const IMPL_HTTP_CLIENT: u16 = 1 << 10;
pub const ALL_KNOWN_IMPLEMENTATIONS: u16 = (1 << 11) - 1;
pub const FACILITY_NATIVE_COMPOSITOR: u16 = 1;
pub const FACILITY_HTTP_CLIENT: u16 = 1 << 1;
pub const RESOURCE_PRESENTATION_SURFACE: u16 = 1;
pub const RESOURCE_HTTP_CLIENT: u16 = 1 << 1;
pub const BASE_DISPLAY_SCANOUT: u16 = 1;
pub const BASE_HTTP_NETWORK: u16 = 1 << 2;
pub const DRIVER_LINEAR_FRAMEBUFFER: u16 = 1;
pub const DRIVER_HTTP_NETWORK: u16 = 1 << 2;
pub const DRIVER_DW_APB_UART2: u16 = 1 << 3;
pub const DRIVER_IA32_DEBUGCON_SERIAL: u16 = 1 << 4;
pub const PRESENTER_NATIVE_GRAPHICAL: u16 = 1;
pub const BASE_SERIAL_TEXT: u16 = 1 << 1;
pub const DRIVER_PL011_SERIAL: u16 = 1 << 1;
pub const PRESENTER_LINEAR_SERIAL: u16 = 1 << 1;
pub const PROOF_HOTPLUG: u16 = 1 << 0;
pub const PROOF_SCRIPTED_KEYBOARD: u16 = 1 << 1;
pub const ALL_KNOWN_PROOF_INSTRUMENTATION: u16 = PROOF_HOTPLUG | PROOF_SCRIPTED_KEYBOARD;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FabricationRecord {
    pub schema: &'static str,
    pub profile_id: &'static str,
    pub build_id: &'static str,
    /// Binding of the embedded resolved description. The whole-ISO ImageId is
    /// external because embedding its own digest would be circular.
    pub image_binding: &'static str,
    pub target: &'static str,
    pub implementations: u16,
    pub facilities: u16,
    pub resources: u16,
    pub bases: u16,
    pub drivers: u16,
    pub presenters: u16,
    pub proof_instrumentation: u16,
    pub presentation_surface_slots: u32,
    pub presentation_surface_bytes: u64,
    pub runtime_arena_ceiling: u64,
    pub operation_slot_ceiling: u32,
    pub timer_slot_ceiling: u32,
    pub evidence_item_ceiling: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FabricationError {
    UnsupportedSchema,
    MalformedIdentity,
    WrongTarget,
    UnknownInventory,
    InvalidCeiling,
    RuntimeArenaExceeded,
}

impl FabricationError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedSchema => "fabrication-schema-unsupported",
            Self::MalformedIdentity => "fabrication-identity-malformed",
            Self::WrongTarget => "fabrication-target-mismatch",
            Self::UnknownInventory => "fabrication-inventory-malformed",
            Self::InvalidCeiling => "fabrication-ceiling-invalid",
            Self::RuntimeArenaExceeded => "fabrication-runtime-arena-exceeded",
        }
    }
}

impl FabricationRecord {
    pub const fn legacy() -> Self {
        Self {
            schema: "legacy-unbound",
            profile_id: "legacy-unbound",
            build_id: "legacy-unbound",
            image_binding: "legacy-unbound",
            target: "legacy-unbound",
            implementations: 0,
            facilities: 0,
            resources: 0,
            bases: 0,
            drivers: 0,
            presenters: 0,
            proof_instrumentation: 0,
            presentation_surface_slots: 0,
            presentation_surface_bytes: 0,
            runtime_arena_ceiling: 0,
            operation_slot_ceiling: 0,
            timer_slot_ceiling: 0,
            evidence_item_ceiling: 0,
        }
    }

    pub fn validate(&self, runtime_arena_bytes: u64) -> Result<(), FabricationError> {
        if self.schema != FABRICATION_SCHEMA {
            return Err(FabricationError::UnsupportedSchema);
        }
        if !valid_id(self.profile_id) || !valid_id(self.build_id) || !valid_id(self.image_binding) {
            return Err(FabricationError::MalformedIdentity);
        }
        if !matches!(
            self.target,
            "conduitos/x86_64/pc"
                | "conduitos/ia32/pc"
                | "conduitos/aarch64/virt"
                | "conduitos/aarch64/orange-pi-5-rk3588s"
        ) {
            return Err(FabricationError::WrongTarget);
        }
        if self.implementations == 0
            || self.implementations & !ALL_KNOWN_IMPLEMENTATIONS != 0
            || self.facilities & !(FACILITY_NATIVE_COMPOSITOR | FACILITY_HTTP_CLIENT) != 0
            || self.resources & !(RESOURCE_PRESENTATION_SURFACE | RESOURCE_HTTP_CLIENT) != 0
            || self.bases & !(BASE_DISPLAY_SCANOUT | BASE_SERIAL_TEXT | BASE_HTTP_NETWORK) != 0
            || self.drivers
                & !(DRIVER_LINEAR_FRAMEBUFFER
                    | DRIVER_PL011_SERIAL
                    | DRIVER_HTTP_NETWORK
                    | DRIVER_DW_APB_UART2
                    | DRIVER_IA32_DEBUGCON_SERIAL)
                != 0
            || self.presenters & !(PRESENTER_NATIVE_GRAPHICAL | PRESENTER_LINEAR_SERIAL) != 0
            || self.proof_instrumentation & !ALL_KNOWN_PROOF_INSTRUMENTATION != 0
        {
            return Err(FabricationError::UnknownInventory);
        }
        let native = self.includes(IMPL_NATIVE_PRESENTER);
        let linear = self.includes(IMPL_LINEAR_PRESENTER);
        if self.includes_facility(FACILITY_NATIVE_COMPOSITOR) != native
            || (self.resources & RESOURCE_PRESENTATION_SURFACE != 0) != native
            || (self.bases & BASE_DISPLAY_SCANOUT != 0) != native
            || (self.drivers & DRIVER_LINEAR_FRAMEBUFFER != 0) != native
            || (self.presenters & PRESENTER_NATIVE_GRAPHICAL != 0) != native
            || (self.presentation_surface_slots != 0) != native
            || (self.presentation_surface_bytes != 0) != native
        {
            return Err(FabricationError::UnknownInventory);
        }
        let expected_serial_driver = match self.target {
            "conduitos/ia32/pc" => DRIVER_IA32_DEBUGCON_SERIAL,
            "conduitos/aarch64/orange-pi-5-rk3588s" => DRIVER_DW_APB_UART2,
            _ => DRIVER_PL011_SERIAL,
        };
        let expected_serial_drivers = if linear { expected_serial_driver } else { 0 };
        if (self.bases & BASE_SERIAL_TEXT != 0) != linear
            || (self.drivers & expected_serial_driver != 0) != linear
            || self.drivers
                & (DRIVER_PL011_SERIAL | DRIVER_DW_APB_UART2 | DRIVER_IA32_DEBUGCON_SERIAL)
                != expected_serial_drivers
            || (self.presenters & PRESENTER_LINEAR_SERIAL != 0) != linear
            || (matches!(
                self.target,
                "conduitos/ia32/pc"
                    | "conduitos/aarch64/virt"
                    | "conduitos/aarch64/orange-pi-5-rk3588s"
            ) && (native || !linear))
            || (self.target == "conduitos/x86_64/pc" && linear)
        {
            return Err(FabricationError::UnknownInventory);
        }
        let http = self.includes(IMPL_HTTP_CLIENT);
        if self.includes_facility(FACILITY_HTTP_CLIENT) != http
            || (self.resources & RESOURCE_HTTP_CLIENT != 0) != http
            || (self.bases & BASE_HTTP_NETWORK != 0) != http
            || (self.drivers & DRIVER_HTTP_NETWORK != 0) != http
        {
            return Err(FabricationError::UnknownInventory);
        }
        if self.runtime_arena_ceiling == 0
            || self.operation_slot_ceiling == 0
            || self.timer_slot_ceiling == 0
            || self.evidence_item_ceiling == 0
        {
            return Err(FabricationError::InvalidCeiling);
        }
        if runtime_arena_bytes > self.runtime_arena_ceiling {
            return Err(FabricationError::RuntimeArenaExceeded);
        }
        Ok(())
    }

    pub const fn includes(&self, implementation: u16) -> bool {
        self.implementations & implementation == implementation
    }

    pub const fn includes_facility(&self, facility: u16) -> bool {
        self.facilities & facility == facility
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_' | b'/' | b'.')
        })
}

include!(concat!(env!("OUT_DIR"), "/fabrication_record.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> FabricationRecord {
        FabricationRecord {
            schema: FABRICATION_SCHEMA,
            profile_id: "sha256:profile",
            build_id: "build:sha256:build",
            image_binding: "image:sha256:binding",
            target: "conduitos/x86_64/pc",
            implementations: ALL_KNOWN_IMPLEMENTATIONS & !IMPL_LINEAR_PRESENTER & !IMPL_HTTP_CLIENT,
            facilities: FACILITY_NATIVE_COMPOSITOR,
            resources: RESOURCE_PRESENTATION_SURFACE,
            bases: BASE_DISPLAY_SCANOUT,
            drivers: DRIVER_LINEAR_FRAMEBUFFER,
            presenters: PRESENTER_NATIVE_GRAPHICAL,
            proof_instrumentation: 0,
            presentation_surface_slots: 2,
            presentation_surface_bytes: 4 * 1024 * 1024,
            runtime_arena_ceiling: 8 * 1024 * 1024,
            operation_slot_ceiling: 64,
            timer_slot_ceiling: 32,
            evidence_item_ceiling: 1024,
        }
    }

    #[test]
    fn exact_record_is_bounded_and_inventory_specific() {
        let record = record();
        assert_eq!(record.validate(4 * 1024 * 1024), Ok(()));
        assert!(record.includes(IMPL_KEYBOARD));
        assert!(
            !FabricationRecord {
                implementations: IMPL_TIME_TICK,
                ..record
            }
            .includes(IMPL_KEYBOARD)
        );
    }

    #[test]
    fn schema_identity_inventory_and_runtime_ceiling_fail_separately() {
        let record = record();
        assert_eq!(
            FabricationRecord {
                schema: "future",
                ..record
            }
            .validate(1),
            Err(FabricationError::UnsupportedSchema)
        );
        assert_eq!(
            FabricationRecord {
                profile_id: "bad value",
                ..record
            }
            .validate(1),
            Err(FabricationError::MalformedIdentity)
        );
        assert_eq!(
            FabricationRecord {
                implementations: 1 << 15,
                ..record
            }
            .validate(1),
            Err(FabricationError::UnknownInventory)
        );
        assert_eq!(
            record.validate(record.runtime_arena_ceiling + 1),
            Err(FabricationError::RuntimeArenaExceeded)
        );
    }

    #[test]
    fn linked_inventory_rejects_claimed_or_compiled_graphics_mismatches() {
        let record = record();
        for malformed in [
            FabricationRecord {
                presenters: 0,
                ..record
            },
            FabricationRecord {
                drivers: 0,
                ..record
            },
            FabricationRecord { bases: 0, ..record },
            FabricationRecord {
                resources: 0,
                ..record
            },
            FabricationRecord {
                facilities: 0,
                ..record
            },
            FabricationRecord {
                implementations: record.implementations & !IMPL_NATIVE_PRESENTER,
                ..record
            },
            FabricationRecord {
                presentation_surface_slots: 0,
                ..record
            },
            FabricationRecord {
                presentation_surface_bytes: 0,
                ..record
            },
            FabricationRecord {
                proof_instrumentation: 1 << 15,
                ..record
            },
        ] {
            assert_eq!(
                malformed.validate(1),
                Err(FabricationError::UnknownInventory)
            );
        }
    }

    #[test]
    fn aarch64_requires_only_the_linear_serial_closure() {
        let x86 = record();
        let record = FabricationRecord {
            target: "conduitos/aarch64/virt",
            implementations: (x86.implementations
                & !(IMPL_NATIVE_PRESENTER | IMPL_KEYBOARD | IMPL_PC_SPEAKER | IMPL_OPL2))
                | IMPL_LINEAR_PRESENTER,
            facilities: 0,
            resources: 0,
            bases: BASE_SERIAL_TEXT,
            drivers: DRIVER_PL011_SERIAL,
            presenters: PRESENTER_LINEAR_SERIAL,
            presentation_surface_slots: 0,
            presentation_surface_bytes: 0,
            ..x86
        };
        assert_eq!(record.validate(1), Ok(()));
        for malformed in [
            FabricationRecord {
                target: "conduitos/x86_64/pc",
                ..record
            },
            FabricationRecord {
                bases: BASE_DISPLAY_SCANOUT,
                ..record
            },
            FabricationRecord {
                drivers: DRIVER_LINEAR_FRAMEBUFFER,
                ..record
            },
            FabricationRecord {
                presenters: PRESENTER_NATIVE_GRAPHICAL,
                ..record
            },
        ] {
            assert_eq!(
                malformed.validate(1),
                Err(FabricationError::UnknownInventory)
            );
        }
    }
}
