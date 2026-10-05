use crate::{CheckedFront, PlannedGear};
use alloc::vec::Vec;

impl PlannedGear {
    /// Reconstruct the checked port portion of this selected Gear's Fore.
    /// Startup parameters have already been consumed by planning and are not
    /// retained in a [`PlannedGear`].
    pub fn checked_port_front(&self) -> CheckedFront {
        CheckedFront::new(Vec::new(), self.inputs.clone(), self.outputs.clone(), None)
            .with_value_contracts(self.semantic_contract.value_contracts().to_vec())
    }

    /// Minimum structural validation for an explicitly assembled Plan record.
    /// Exact semantic content is subsequently covered by the fragment and
    /// Plan fingerprints; this boundary prevents anonymous/empty identities
    /// from entering those seals through fixtures or composition code.
    #[doc(hidden)]
    pub fn validate_constructed_identity(&self) -> bool {
        crate::valid_realization_properties(&self.realization_properties)
            && !self.placement_id.as_str().is_empty()
            && !self.gear_id.as_str().is_empty()
            && !self.kind_id.as_str().is_empty()
            && !self.kind_contract_revision.as_str().is_empty()
            && self.source_span.is_none_or(crate::SourceSpan::is_valid)
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
    (realization_properties: $properties:expr, $($fields:tt)*) => {{
        let gear = $crate::PlannedGear {
            source_span: None,
            realization_properties: $properties,
            $($fields)*
        };
        assert!(
            gear.validate_constructed_identity(),
            "PlannedGear requires complete semantic and realization identities"
        );
        gear
    }};
    ($($fields:tt)*) => {{
        $crate::planned_gear_from_parts! {
            realization_properties: ::core::default::Default::default(),
            $($fields)*
        }
    }};
}
