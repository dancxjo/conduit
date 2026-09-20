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
}

impl CheckedCanonicalForm {
    pub fn checked_front(&self) -> conduit_core::CheckedFront {
        self.runtime_front.clone()
    }
}
