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
pub const ALL_KNOWN_IMPLEMENTATIONS: u16 = (1 << 9) - 1;
pub const FACILITY_NATIVE_COMPOSITOR: u16 = 1;
pub const RESOURCE_PRESENTATION_SURFACE: u16 = 1;
pub const BASE_DISPLAY_SCANOUT: u16 = 1;
pub const DRIVER_LINEAR_FRAMEBUFFER: u16 = 1;
pub const PRESENTER_NATIVE_GRAPHICAL: u16 = 1;
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
        if self.target != "conduitos/x86_64/pc" {
            return Err(FabricationError::WrongTarget);
        }
        if self.implementations == 0
            || self.implementations & !ALL_KNOWN_IMPLEMENTATIONS != 0
            || self.facilities & !FACILITY_NATIVE_COMPOSITOR != 0
            || self.resources & !RESOURCE_PRESENTATION_SURFACE != 0
            || self.bases & !BASE_DISPLAY_SCANOUT != 0
            || self.drivers & !DRIVER_LINEAR_FRAMEBUFFER != 0
            || self.presenters & !PRESENTER_NATIVE_GRAPHICAL != 0
            || self.proof_instrumentation & !ALL_KNOWN_PROOF_INSTRUMENTATION != 0
        {
            return Err(FabricationError::UnknownInventory);
        }
        let native = self.includes(IMPL_NATIVE_PRESENTER);
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
            implementations: ALL_KNOWN_IMPLEMENTATIONS,
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
}
