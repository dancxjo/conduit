use super::StdHostComposition;

impl StdHostComposition {
    pub const fn with_experience_evidence(mut self) -> Self {
        self.experience_evidence = true;
        self
    }
}

pub(super) fn append(
    composition: StdHostComposition,
    offers: &mut Vec<conduit_core::CapabilityOffer>,
) {
    if composition.experience_evidence {
        offers.push(conduit_std_offers::experience_evidence_offer());
    }
}
