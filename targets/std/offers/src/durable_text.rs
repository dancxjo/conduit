//! Explicitly selected immutable Text publication on a durable filesystem Base.
//! These offers are never part of the ambient std inventory.
use conduit_core::*;

pub const DURABLE_TEXT_SAVE_IMPLEMENTATION: &str = "std/file-data-save-text@1";
pub const DURABLE_TEXT_LOAD_IMPLEMENTATION: &str = "std/file-data-load-text@1";
pub const DURABLE_TEXT_ARTIFACT: &str = "conduit-std-host/file-text-generations@1";
pub const DURABLE_TEXT_PROFILE: &str = "std/selected-file-text@1";
pub const DURABLE_TEXT_BASE: &str = "std/selected-file-snapshot";
pub const DURABLE_TEXT_RESIDENCE: &str = "std/selected-file-snapshot@1";
pub const DURABLE_TEXT_PUBLISH_CALL: &str = "conduit.host/resource-snapshot-publish@1";
pub const DURABLE_TEXT_READ_CALL: &str = "conduit.host/resource-snapshot-read@1";
pub const DURABLE_TEXT_AUTHORITY: &str = "authority/resource-snapshot@1";

/// The caller selects one exact content identity and generation before Plan.
/// The reference is descriptive; publication still requires current authority.
pub fn durable_text_offer(
    content: ResourceContentRequirement,
) -> Result<CapabilityOffer, &'static str> {
    content
        .validate()
        .map_err(|_| "invalid Text residence contract")?;
    if content.content_profile != kind_id("value/text")
        || content.maximum_bytes > conduit_data::MAXIMUM_DATA_TEXT_BYTES
        || content.maximum_items != 1
        || content.generation_slots != 1
        || content.reader_leases != 1
        || content.retention != ResourceRetention::ExternalDurable
        || content.sharing != ResourceSharing::SingleWriterPublished
    {
        return Err("durable Text requires one finite immutable generation");
    }
    let write = content.access == ResourceAccessMode::WriteCandidatePublish;
    let (contract, implementation, call) = if write {
        (
            conduit_data::data_save_text_contract(),
            DURABLE_TEXT_SAVE_IMPLEMENTATION,
            DURABLE_TEXT_PUBLISH_CALL,
        )
    } else {
        (
            conduit_data::data_load_text_contract(),
            DURABLE_TEXT_LOAD_IMPLEMENTATION,
            DURABLE_TEXT_READ_CALL,
        )
    };
    let kind = contract.kind_id.clone();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: implementation.into(),
            execution_profile_id: DURABLE_TEXT_PROFILE.into(),
            implementation_id: implementation.into(),
            artifact_id: DURABLE_TEXT_ARTIFACT.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: call.into(),
                target_kind: Some(kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: if write {
                    4096
                } else {
                    MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
                },
                maximum_output_bytes: if write {
                    MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
                } else {
                    4096
                },
            }],
            resource_requirements: vec![ResourceRequirement {
                class_id: conduit_data::DATA_GENERATION_ACCESS_CLASS.into(),
                units: 1,
                protected_role: None,
                compute: None,
                content: Some(content),
            }],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: DURABLE_TEXT_AUTHORITY.into(),
                host_call_contract_id: call.into(),
                subject_kind: kind,
            }],
        },
    )
    .build())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn content(write: bool) -> ResourceContentRequirement {
        ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            version: ResourceVersionIdentity::from_digest([2; 32]),
            content_profile: kind_id("value/text"),
            maximum_bytes: 256,
            maximum_items: 1,
            retention: ResourceRetention::ExternalDurable,
            sharing: ResourceSharing::SingleWriterPublished,
            access: if write {
                ResourceAccessMode::WriteCandidatePublish
            } else {
                ResourceAccessMode::ReadPublished
            },
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: u16::from(write),
            sensitive: false,
        }
    }
    #[test]
    fn durable_text_preserves_the_portable_data_contract_and_requires_authority() {
        for write in [true, false] {
            let offer = durable_text_offer(content(write)).unwrap();
            let contract = if write {
                conduit_data::data_save_text_contract()
            } else {
                conduit_data::data_load_text_contract()
            };
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                contract.kind_contract_revision
            );
            assert_eq!(offer.semantic_contract, contract.semantic_contract());
            assert_eq!(offer.authority_requirements.len(), 1);
            assert_eq!(offer.resource_requirements[0].content, Some(content(write)));
        }
        let mut invalid = content(true);
        invalid.retention = ResourceRetention::Play;
        assert!(durable_text_offer(invalid).is_err());
        let mut invalid = content(false);
        invalid.maximum_bytes = 4097;
        assert!(durable_text_offer(invalid).is_err());
    }
}
