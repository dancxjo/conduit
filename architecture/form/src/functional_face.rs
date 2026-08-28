use crate::{CheckedCanonicalForm, CheckedGear};

impl CheckedGear {
    pub fn checked_face(&self) -> conduit_core::CheckedFace {
        conduit_core::CheckedFace::new(
            self.startup_parameters.clone(),
            self.inputs.clone(),
            self.outputs.clone(),
            self.shorthand.clone(),
        )
    }
}

impl CheckedCanonicalForm {
    pub fn checked_face(&self) -> conduit_core::CheckedFace {
        self.runtime_face.clone()
    }
}
