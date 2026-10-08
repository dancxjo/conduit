//! Exact synthetic proof selection; ordinary product defaults remain separate.
use crate::make::*;
pub const MAKE_PROFILE: &str = "profile:conduitos-synthetic-numeric-topology-v1";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrongProofProfile;
pub fn validate_make(make: &MakeRecord) -> Result<(), WrongProofProfile> {
    make.validate(super::ARENA_BYTES as u64)
        .map_err(|_| WrongProofProfile)?;
    if make.profile_id != MAKE_PROFILE
        || make.target != "conduitos/x86_64/pc"
        || make.proof_instrumentation != PROOF_NUMERIC_TOPOLOGY
        || make.runtime_arena_ceiling != super::ARENA_BYTES as u64
        || make.operation_slot_ceiling != super::MAXIMUM_NODES as u32
        || make.timer_slot_ceiling != 1
        || make.evidence_item_ceiling != 32768
        || make.implementations != IMPL_TEXT_LITERAL
        || make.facilities != 0
        || make.resources != 0
        || make.bases != 0
        || make.drivers != 0
        || make.presenters != 0
        || make.presentation_surface_slots != 0
        || make.presentation_surface_bytes != 0
    {
        return Err(WrongProofProfile);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    mod selected {
        use super::*;
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proof/fargan/synthetic_guest_make.rs"
        ));
    }
    #[test]
    fn exact_proof_profile_refuses_default_capacity_and_inventory_drift() {
        let selected = selected::EMBEDDED_MAKE;
        validate_make(&selected).unwrap();
        let ordinary = MakeRecord {
            profile_id: "profile:ordinary",
            proof_instrumentation: 0,
            runtime_arena_ceiling: 16 * 1024 * 1024,
            operation_slot_ceiling: 64,
            ..selected
        };
        ordinary.validate(16 * 1024 * 1024).unwrap();
        assert!(validate_make(&ordinary).is_err());
        let mut drift = selected;
        drift.operation_slot_ceiling -= 1;
        assert!(validate_make(&drift).is_err());
        let mut drift = selected;
        drift.runtime_arena_ceiling -= 32;
        assert!(validate_make(&drift).is_err());
        let mut drift = selected;
        drift.implementations |= IMPL_TEXT_UPPER;
        assert!(validate_make(&drift).is_err());
        let mut drift = selected;
        drift.proof_instrumentation |= PROOF_HOTPLUG;
        assert!(validate_make(&drift).is_err());
    }
}
