use crate::PlannedGear;

impl PlannedGear {
    /// Minimum structural validation for an explicitly assembled Plan record.
    /// Exact semantic content is subsequently covered by the fragment and
    /// Plan fingerprints; this boundary prevents anonymous/empty identities
    /// from entering those seals through fixtures or composition code.
    #[doc(hidden)]
    pub fn validate_constructed_identity(&self) -> bool {
        !self.placement_id.as_str().is_empty()
            && !self.gear_id.as_str().is_empty()
            && !self.kind_id.as_str().is_empty()
            && !self.kind_contract_revision.as_str().is_empty()
            && !self.execution_profile_id.as_str().is_empty()
            && !self.host_id.as_str().is_empty()
            && !self.boot_id.as_str().is_empty()
            && !self.capability_id.as_str().is_empty()
            && !self.implementation_id.as_str().is_empty()
            && !self.artifact_id.as_str().is_empty()
    }
}

/// Reviewed named-field construction for a fully selected Gear.
///
/// The planner is the production owner of this record. Tests and bounded
/// composition code use this boundary so a new Plan field cannot silently be
/// omitted, and so every explicit record receives the same identity checks.
#[macro_export]
macro_rules! planned_gear_from_parts {
    ($($fields:tt)*) => {{
        let gear = $crate::PlannedGear { $($fields)* };
        assert!(
            gear.validate_constructed_identity(),
            "PlannedGear requires complete semantic and realization identities"
        );
        gear
    }};
}
