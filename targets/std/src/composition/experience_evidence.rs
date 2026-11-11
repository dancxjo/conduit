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
        offers.push(conduit_std_offers::experience_evidence_flow_offer());
        offers.push(conduit_std_offers::experience_integration_offer());
        offers.push(conduit_std_offers::experience_prompt_offer());
        offers.push(conduit_std_offers::experience_situation_offer());
        offers.push(conduit_std_offers::experience_autobiography_retain_offer());
        offers.push(conduit_std_offers::experience_autobiography_recovery_offer());
        offers.push(conduit_std_offers::experience_autobiography_recall_offer());
    }
}
