//! Exact source selection for the prepared instruction, without current input grants.
use super::parsec_source_plan::{expanded, Producer};
use conduit_core::{
    CapabilityOffer, ConfigurationValue, HostAdvertisement, PreparationHostIdentity,
};
use conduit_plot::{CheckedSyntaxDocument, PortableExpressionProgram};
use conduit_presentation::{
    checked_parsec_input_instruction, prepare_checked_parsec_input_instruction,
    PreparedParsecInputInstruction,
};

pub fn required_parsec_input_instruction_offer() -> Result<CapabilityOffer, String> {
    // Reuse only the immutable source-derived offer, never a Host or Plan.
    static OFFER: std::sync::OnceLock<Result<CapabilityOffer, String>> = std::sync::OnceLock::new();
    OFFER
        .get_or_init(prepare_parsec_input_instruction_offer)
        .clone()
}

fn prepare_parsec_input_instruction_offer() -> Result<CapabilityOffer, String> {
    let checked =
        checked_parsec_input_instruction().map_err(|e| format!("Parsec instruction: {e:?}"))?;
    let expanded = expanded(&checked, "parsec/input-instruction")?;
    let gear = &expanded.expanded.gears[0];
    let program = gear
        .configuration
        .iter()
        .find(|field| field.key == "program")
        .ok_or("Parsec instruction has no checked program")?;
    let ConfigurationValue::Text(encoded) = &program.value else {
        return Err("Parsec instruction program has a different representation".into());
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|e| format!("Parsec instruction program: {e:?}"))?;
    conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
        .map_err(|e| format!("Parsec instruction offer: {e:?}"))
}

/// Holds the independently reproduced instruction Plan selected on the current Host.
/// This value establishes computational custody only, never input availability.
pub struct ParsecInputInstructionSourcePlan {
    checked: CheckedSyntaxDocument,
    hosts: [HostAdvertisement; 1],
    producer: Producer,
}

impl ParsecInputInstructionSourcePlan {
    pub fn prepare(host: &HostAdvertisement) -> Result<Self, String> {
        let offer = required_parsec_input_instruction_offer()?;
        if host
            .capabilities
            .iter()
            .find(|candidate| candidate.capability_id == offer.capability_id)
            != Some(&offer)
        {
            return Err(
                "current Host does not offer the exact checked Parsec input instruction".into(),
            );
        }
        let checked =
            checked_parsec_input_instruction().map_err(|e| format!("Parsec instruction: {e:?}"))?;
        let hosts = [host.clone()];
        let producer = Producer::prepare_bounded(
            &checked,
            &hosts,
            "parsec/input-instruction",
            conduit_presentation::PARSEC_INPUT_INSTRUCTION_FRAME_BYTES as u32,
        )?;
        Ok(Self {
            checked,
            hosts,
            producer,
        })
    }

    pub fn evaluator(&self) -> Result<PreparedParsecInputInstruction, String> {
        let custody = self.producer.custody(&self.checked, &self.hosts)?;
        prepare_checked_parsec_input_instruction(&custody)
            .map_err(|e| format!("prepared Parsec instruction: {e:?}"))
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
    use crate::{StdHost, StdHostConfig};
    #[test]
    fn instruction_source_requires_exact_selected_constructor_and_retains_host_custody() {
        let absent = StdHost::new_with_config(StdHostConfig {
            host_id: "proof/instruction-source".into(),
            boot_id: "proof/instruction-source/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        });
        assert!(ParsecInputInstructionSourcePlan::prepare(absent.advertisement()).is_err());
        let offer = required_parsec_input_instruction_offer().unwrap();
        let mut advertisement = absent.advertisement().clone();
        advertisement.capabilities.push(offer.clone());
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let host = StdHost::from_advertisement(advertisement).unwrap();
        let selected = ParsecInputInstructionSourcePlan::prepare(host.advertisement()).unwrap();
        assert_eq!(
            selected.host_identity().host_id,
            host.advertisement().host_id
        );
        assert_eq!(
            selected.host_identity().boot_id,
            host.advertisement().boot_id
        );
        assert_eq!(
            selected.host_identity().offer_generation,
            host.advertisement().offer_generation
        );
        assert!(selected.evaluator().unwrap().prepared_buffer_bytes() > 0);
        let mut changed = host.advertisement().clone();
        changed
            .capabilities
            .iter_mut()
            .find(|x| x.capability_id == offer.capability_id)
            .unwrap()
            .implementation
            .artifact_id = "forged/instruction-source".into();
        assert!(ParsecInputInstructionSourcePlan::prepare(&changed).is_err());
    }
}
