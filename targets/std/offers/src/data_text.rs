//! Finite Play-lived Text data-generation offers.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, ResourceClassId,
    ResourceRequirement,
};

pub const DATA_SAVE_TEXT_STD_PROFILE: &str = "std/data-save-text-play@1";
pub const DATA_LOAD_TEXT_STD_PROFILE: &str = "std/data-load-text-play@1";
pub const DATA_SAVE_TEXT_STD_IMPLEMENTATION: &str = "std/host-data-save-text@1";
pub const DATA_LOAD_TEXT_STD_IMPLEMENTATION: &str = "std/host-data-load-text@1";
pub const DATA_TEXT_STD_ARTIFACT: &str = "conduit-std-host/text-data-generations@1";
pub const DATA_SAVE_TEXT_HOST_CALL: &str = "conduit.host/data-save-text@1";
pub const DATA_LOAD_TEXT_HOST_CALL: &str = "conduit.host/data-load-text@1";
pub const DATA_TEXT_GENERATION_RESOURCE_CLASS: &str =
    "conduit.resource/play-text-data-generations@1";
pub const DATA_TEXT_MAXIMUM_GENERATIONS: usize = 4;
pub const DATA_TEXT_MAXIMUM_RETAINED_BYTES: usize =
    DATA_TEXT_MAXIMUM_GENERATIONS * conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize;
pub const DATA_TEXT_RESOURCE_BINDING_CAPACITY: u32 = 16;

pub fn data_save_text_std_offer() -> CapabilityOffer {
    data_text_offer(
        conduit_data::data_save_text_contract(),
        "data-save-text-play",
        DATA_SAVE_TEXT_STD_PROFILE,
        DATA_SAVE_TEXT_STD_IMPLEMENTATION,
        DATA_SAVE_TEXT_HOST_CALL,
        conduit_data::MAXIMUM_DATA_TEXT_BYTES,
        text_reference_bytes(),
    )
}

pub fn data_load_text_std_offer() -> CapabilityOffer {
    data_text_offer(
        conduit_data::data_load_text_contract(),
        "data-load-text-play",
        DATA_LOAD_TEXT_STD_PROFILE,
        DATA_LOAD_TEXT_STD_IMPLEMENTATION,
        DATA_LOAD_TEXT_HOST_CALL,
        text_reference_bytes(),
        conduit_data::MAXIMUM_DATA_TEXT_BYTES,
    )
}

fn text_reference_bytes() -> u32 {
    conduit_data::maximum_data_reference_encoded_bytes("value/text")
        .expect("canonical Text reference has a finite exact envelope") as u32
}

fn data_text_offer(
    contract: conduit_core::Kind,
    capability: &str,
    profile: &str,
    implementation: &str,
    host_call: &str,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
) -> CapabilityOffer {
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(DATA_TEXT_STD_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(host_call),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes,
                maximum_output_bytes,
            }],
            resource_requirements: vec![ResourceRequirement {
                class_id: ResourceClassId::from(DATA_TEXT_GENERATION_RESOURCE_CLASS),
                units: 1,
                protected_role: None,
                compute: None,
                content: None,
            }],
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, FrontValueLocation};

    #[test]
    fn save_and_load_preserve_exact_v2_fronts_and_require_one_shared_slot() {
        let save = data_save_text_std_offer();
        let load = data_load_text_std_offer();
        for offer in [&save, &load] {
            assert_eq!(
                offer.kind_contract_revision.as_str(),
                conduit_data::DATA_TEXT_CONTRACT_REVISION
            );
            assert_eq!(offer.resource_requirements.len(), 1);
            assert_eq!(offer.resource_requirements[0].units, 1);
            assert_eq!(
                offer.resource_requirements[0].class_id.as_str(),
                DATA_TEXT_GENERATION_RESOURCE_CLASS
            );
            assert!(offer.authority_requirements.is_empty());
            assert_eq!(offer.host_calls.len(), 1);
            assert_eq!(offer.host_calls[0].maximum_in_flight, 1);
        }
        assert_eq!(
            save.outputs[0].abnormal_kind,
            Some(kind_id(conduit_data::DATA_SAVE_TEXT_TERMINAL_INFO_ID))
        );
        assert_eq!(
            load.outputs[0].abnormal_kind,
            Some(kind_id(conduit_data::DATA_LOAD_TEXT_TERMINAL_INFO_ID))
        );
        assert_eq!(
            save.checked_front()
                .value_contract(&FrontValueLocation::Input(conduit_core::port_id("value")))
                .unwrap()
                .maximum_bytes,
            conduit_data::MAXIMUM_DATA_TEXT_BYTES
        );
        assert_eq!(
            save.host_calls[0].maximum_output_bytes,
            text_reference_bytes()
        );
        assert_eq!(
            load.host_calls[0].maximum_input_bytes,
            text_reference_bytes()
        );
        assert_eq!(
            load.host_calls[0].maximum_output_bytes,
            conduit_data::MAXIMUM_DATA_TEXT_BYTES
        );
    }
}
