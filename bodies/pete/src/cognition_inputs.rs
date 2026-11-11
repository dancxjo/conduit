//! Prepare one generic sourced batch from Pete's selected Situation.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub const PETE_COGNITION_INPUTS_KIND: &str = "pete/situate-experience-inputs";
pub fn pete_cognition_inputs_contract() -> Kind {
    let mut contract = pete_cognition_situate_contract();
    contract.kind_id = kind_id(PETE_COGNITION_INPUTS_KIND);
    contract.kind_contract_revision = "conduit.pete/situate-experience-inputs@1".into();
    contract.outputs[0].port_id = port_id("inputs");
    contract.outputs[0].value_kind = conduit_chat::episode_experience_input_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    contract
}
pub fn pete_cognition_inputs_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        pete_cognition_inputs_contract(),
        Back {
            capability_id: "pete-situate-experience-inputs".into(),
            execution_profile_id: "conduit.std/pete-situate-experience-inputs-kernel@1".into(),
            implementation_id: "std/pete-situate-experience-inputs@1".into(),
            artifact_id: "conduit-std-host/pete-situate-experience-inputs@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
pub fn prepare_pete_experience_input_bytes(bytes: &[u8]) -> Result<Vec<u8>, u16> {
    if bytes.len() > MAXIMUM_PETE_COGNITION_FRAME_BYTES {
        return Err(1);
    }
    let frame = PeteCognitionFrame::decode(bytes).map_err(|_| 1_u16)?;
    let input = prepare_pete_experience_input(&frame).map_err(|refusal| match refusal {
        PeteCognitionRefusal::InvalidObservation => 2_u16,
        PeteCognitionRefusal::Situation => 3,
        PeteCognitionRefusal::Experience => 4,
        PeteCognitionRefusal::Evidence => 5,
        PeteCognitionRefusal::Homeostasis => 6,
    })?;
    input.encode().map_err(|_| 5)
}
