//! Exact source selection for the prepared policy, without current input grants.
use super::{expanded, Producer};
use conduit_core::{
    CapabilityOffer, ConfigurationValue, HostAdvertisement, PreparationHostIdentity,
};
use conduit_plot::{CheckedSyntaxDocument, PortableExpressionProgram};
use conduit_presentation::{
    checked_parsec_input_policy, prepare_checked_parsec_input_policy, PreparedParsecInputPolicy,
};

pub fn required_parsec_input_policy_offer() -> Result<CapabilityOffer, String> {
    let checked = checked_parsec_input_policy().map_err(|e| format!("Parsec policy: {e:?}"))?;
    let expanded = expanded(&checked, "parsec/input-policy")?;
    let gear = &expanded.expanded.gears[0];
    let program = gear
        .configuration
        .iter()
        .find(|field| field.key == "program")
        .ok_or("Parsec policy has no checked program")?;
    let ConfigurationValue::Text(encoded) = &program.value else {
        return Err("Parsec policy program has a different representation".into());
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|e| format!("Parsec policy program: {e:?}"))?;
    conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
        .map_err(|e| format!("Parsec policy offer: {e:?}"))
}

/// Holds the independently reproduced policy Plan selected on the current Host.
/// This value establishes computational custody only, never input availability.
pub struct ParsecInputPolicySourcePlan {
    checked: CheckedSyntaxDocument,
    hosts: [HostAdvertisement; 1],
    producer: Producer,
}

impl ParsecInputPolicySourcePlan {
    pub fn prepare(host: &HostAdvertisement) -> Result<Self, String> {
        let offer = required_parsec_input_policy_offer()?;
        if host
            .capabilities
            .iter()
            .find(|candidate| candidate.capability_id == offer.capability_id)
            != Some(&offer)
        {
            return Err("current Host does not offer the exact checked Parsec input policy".into());
        }
        let checked = checked_parsec_input_policy().map_err(|e| format!("Parsec policy: {e:?}"))?;
        let hosts = [host.clone()];
        let producer = Producer::prepare(&checked, &hosts, "parsec/input-policy")?;
        Ok(Self {
            checked,
            hosts,
            producer,
        })
    }

    pub fn evaluator(&self) -> Result<PreparedParsecInputPolicy, String> {
        let custody = self.producer.custody(&self.checked, &self.hosts)?;
        prepare_checked_parsec_input_policy(&custody)
            .map_err(|e| format!("prepared Parsec policy: {e:?}"))
    }

    pub fn host_identity(&self) -> PreparationHostIdentity {
        PreparationHostIdentity {
            host_id: self.hosts[0].host_id.clone(),
            boot_id: self.hosts[0].boot_id.clone(),
            offer_generation: self.hosts[0].offer_generation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BodyLiveForeQueue, StdHost, StdHostConfig};
    fn config() -> StdHostConfig {
        StdHostConfig {
            host_id: "std/policy-source-proof".into(),
            boot_id: "std/policy-source-proof/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        }
    }

    #[test]
    fn policy_source_plan_requires_exact_current_constructor_offer() {
        let absent = StdHost::new_for_parsec_scan(config(), 32).unwrap();
        assert!(ParsecInputPolicySourcePlan::prepare(absent.advertisement()).is_err());
        let host = BodyLiveForeQueue::parsec_input_test_host(config());
        let selected = ParsecInputPolicySourcePlan::prepare(host.advertisement()).unwrap();
        let identity = selected.host_identity();
        assert_eq!(identity.host_id, host.advertisement().host_id);
        assert_eq!(identity.boot_id, host.advertisement().boot_id);
        assert_eq!(
            identity.offer_generation,
            host.advertisement().offer_generation
        );
        assert!(selected.evaluator().unwrap().prepared_buffer_bytes() > 0);
        let mut forged = host.advertisement().clone();
        let expected = required_parsec_input_policy_offer().unwrap();
        let offer = forged
            .capabilities
            .iter_mut()
            .find(|offer| offer.capability_id == expected.capability_id)
            .unwrap();
        offer.implementation.artifact_id = "forged/policy-artifact".into();
        assert!(ParsecInputPolicySourcePlan::prepare(&forged).is_err());
    }
}
