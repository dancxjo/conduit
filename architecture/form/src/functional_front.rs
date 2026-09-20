use crate::{CheckedCanonicalForm, CheckedGear};

impl CheckedGear {
    pub fn checked_front(&self) -> conduit_core::CheckedFront {
        conduit_core::CheckedFront::new(
            self.startup_parameters.clone(),
            self.inputs.clone(),
            self.outputs.clone(),
            self.shorthand.clone(),
        )
    }

    /// Tests both callable fit and semantic realization eligibility.
    ///
    /// Ordinary gears require the same semantic contract identity. A semantic
    /// owner may deliberately request structural polymorphism with the
    /// reviewed marker; that is an authored meaning, not a planner fallback.
    pub fn accepts_realization(&self, offer: &conduit_core::CapabilityOffer) -> bool {
        self.checked_front() == offer.checked_front()
            && (self.kind_contract_revision == offer.kind_contract_revision
                || self.kind_contract_revision.as_str()
                    == conduit_core::STRUCTURAL_POLYMORPHIC_CONTRACT)
    }
}

impl CheckedCanonicalForm {
    pub fn checked_front(&self) -> conduit_core::CheckedFront {
        self.runtime_front.clone()
    }
}
