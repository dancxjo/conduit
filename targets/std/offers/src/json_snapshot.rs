//! Explicitly selected immutable JSON snapshot effects; never ambient inventory.
//! Descriptive content does not establish directory custody, disk reservation,
//! current authority, publication commitment or producer acknowledgement.
use conduit_core::*;

pub const JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION: &str = "std/file-json-snapshot-publish@1";
pub const JSON_SNAPSHOT_READ_IMPLEMENTATION: &str = "std/file-json-snapshot-read@1";
pub const JSON_SNAPSHOT_ARTIFACT: &str = "conduit-std-host/file-json-snapshot@1";
pub const JSON_SNAPSHOT_PROFILE: &str = "std/selected-file-json-snapshot@1";
pub const JSON_SNAPSHOT_BASE: &str = "std/selected-json-snapshot";
pub const JSON_SNAPSHOT_RESIDENCE: &str = "std/selected-json-snapshot@1";
pub const JSON_SNAPSHOT_ACCESS_CLASS: &str = "resource/snapshot@1";
pub const JSON_SNAPSHOT_PUBLISH_CALL: &str = "conduit.host/resource-snapshot-publish@1";
pub const JSON_SNAPSHOT_READ_CALL: &str = "conduit.host/resource-snapshot-read@1";
pub const JSON_SNAPSHOT_AUTHORITY: &str = "authority/resource-snapshot@1";
pub const JSON_SNAPSHOT_MAXIMUM_BYTES: u32 = 4096;
/// All selected metadata cloned during operation preparation is fenced before
/// cloning. This is a byte extent, not a trust or authority classification.
pub const JSON_SNAPSHOT_MAXIMUM_ID_BYTES: usize = 128;
/// Finite ordinary preparation reservation, separate from kernel value storage.
/// Two record buffers use the existing record envelope. Sixty-four bounded text
/// slots cover the simultaneous provider/Host authority clones, temporary offer
/// and reference-admission metadata. Eight reference envelopes cover decode,
/// encode and returned-reference scratch. Sixteen complete inline Kind/authority/
/// content envelopes conservatively cover the provider and temporary containers;
/// key derivation and geometric String growth receive their own 512-byte envelope.
/// Caller-owned selections, original input and scheduler storage are separate.
pub const JSON_SNAPSHOT_PREPARATION_BYTES: u32 = 2
    * (8 + 2
        + 128
        + 2
        + MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
        + 32
        + JSON_SNAPSHOT_MAXIMUM_BYTES)
    + 64 * JSON_SNAPSHOT_MAXIMUM_ID_BYTES as u32
    + 8 * MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
    + 16 * (core::mem::size_of::<Kind>()
        + core::mem::size_of::<AuthorityBinding>()
        + core::mem::size_of::<ResourceContentRequirement>()) as u32
    + (8 + 2 + 128 + 2 + MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32)
    + 512;

/// One caller-selected exact immutable generation. Root independently admits
/// its actual durable residence and current operation-specific authority.
/// This offer describes JSON snapshot semantics, not Text data/save or data/load.
pub fn json_snapshot_offer(
    content: ResourceContentRequirement,
) -> Result<CapabilityOffer, &'static str> {
    content
        .validate()
        .map_err(|_| "invalid JSON snapshot residence contract")?;
    if content.content_profile != kind_id("text/json-utf8@1")
        || !(1..=JSON_SNAPSHOT_MAXIMUM_BYTES).contains(&content.maximum_bytes)
        || content.maximum_items != 1
        || content.generation_slots != 1
        || content.reader_leases != 1
        || content.retention != ResourceRetention::ExternalDurable
        || content.sharing != ResourceSharing::SingleWriterPublished
    {
        return Err("JSON snapshot requires one finite immutable generation");
    }
    // Exhaustive operation selection: a future access mode is not implicitly a read.
    let (write, implementation, call) = match content.access {
        ResourceAccessMode::WriteCandidatePublish => (
            true,
            JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION,
            JSON_SNAPSHOT_PUBLISH_CALL,
        ),
        ResourceAccessMode::ReadPublished => (
            false,
            JSON_SNAPSHOT_READ_IMPLEMENTATION,
            JSON_SNAPSHOT_READ_CALL,
        ),
    };
    let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(write);
    let subject = kind.kind_id.clone();
    let mut offer = BackOfferBuilder::new(
        kind,
        Back {
            capability_id: implementation.into(),
            implementation_id: implementation.into(),
            execution_profile_id: JSON_SNAPSHOT_PROFILE.into(),
            artifact_id: JSON_SNAPSHOT_ARTIFACT.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: call.into(),
                target_kind: Some(subject.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: if write {
                    JSON_SNAPSHOT_MAXIMUM_BYTES
                } else {
                    MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
                },
                maximum_output_bytes: if write {
                    MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32
                } else {
                    JSON_SNAPSHOT_MAXIMUM_BYTES
                },
            }],
            resource_requirements: vec![
                ResourceRequirement {
                    class_id: JSON_SNAPSHOT_ACCESS_CLASS.into(),
                    units: 1,
                    protected_role: None,
                    compute: None,
                    content: Some(content),
                },
                resource_requirement(
                    RUNTIME_MEMORY_RESOURCE_CLASS,
                    JSON_SNAPSHOT_PREPARATION_BYTES,
                ),
            ],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: JSON_SNAPSHOT_AUTHORITY.into(),
                host_call_contract_id: call.into(),
                subject_kind: subject,
            }],
        },
    )
    .build();
    offer.resource_requirements.sort();
    Ok(offer)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn content(write: bool) -> ResourceContentRequirement {
        ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            version: ResourceVersionIdentity::from_digest([2; 32]),
            content_profile: kind_id("text/json-utf8@1"),
            maximum_bytes: 4096,
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
            sensitive: true,
        }
    }
    #[test]
    fn selected_json_offers_preserve_exact_kind_ports_calls_content_and_authority() {
        for write in [false, true] {
            let offer = json_snapshot_offer(content(write)).unwrap();
            let expected = conduit_semantic_catalog::resource_snapshot_semantic_contract(write);
            assert_eq!(offer.semantic_contract, expected.semantic_contract());
            assert_eq!(offer.kind_id, expected.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                expected.kind_contract_revision
            );
            assert_eq!(offer.inputs, expected.inputs);
            assert_eq!(offer.outputs, expected.outputs);
            assert_eq!(offer.resource_requirements.len(), 2);
            assert!(offer
                .resource_requirements
                .windows(2)
                .all(|pair| pair[0] < pair[1]));
            let memory = offer
                .resource_requirements
                .iter()
                .find(|requirement| requirement.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
                .unwrap();
            let snapshot = offer
                .resource_requirements
                .iter()
                .find(|requirement| requirement.class_id.as_str() == JSON_SNAPSHOT_ACCESS_CLASS)
                .unwrap();
            assert_eq!(
                *memory,
                resource_requirement(
                    RUNTIME_MEMORY_RESOURCE_CLASS,
                    JSON_SNAPSHOT_PREPARATION_BYTES
                )
            );
            assert_eq!(snapshot.content, Some(content(write)));
            assert_eq!(
                offer.implementation.execution_profile_id.as_str(),
                JSON_SNAPSHOT_PROFILE
            );
            assert_eq!(
                offer.implementation.artifact_id.as_str(),
                JSON_SNAPSHOT_ARTIFACT
            );
            let (implementation, call, input, output) = if write {
                (
                    JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION,
                    JSON_SNAPSHOT_PUBLISH_CALL,
                    4096,
                    MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
                )
            } else {
                (
                    JSON_SNAPSHOT_READ_IMPLEMENTATION,
                    JSON_SNAPSHOT_READ_CALL,
                    MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
                    4096,
                )
            };
            assert_eq!(
                offer.implementation.implementation_id.as_str(),
                implementation
            );
            assert_eq!(offer.capability_id.as_str(), implementation);
            assert_eq!(
                offer.host_calls,
                vec![HostCallRequirement {
                    contract_id: call.into(),
                    target_kind: Some(offer.kind_id.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: input,
                    maximum_output_bytes: output
                }]
            );
            assert_eq!(
                offer.authority_requirements,
                vec![AuthorityRequirement {
                    contract_id: JSON_SNAPSHOT_AUTHORITY.into(),
                    host_call_contract_id: call.into(),
                    subject_kind: offer.kind_id.clone()
                }]
            );
        }
    }
    #[test]
    fn foreign_profiles_lifetimes_multiplicity_and_operation_slot_mismatches_refuse() {
        for write in [false, true] {
            for mutation in 0..11 {
                let mut invalid = content(write);
                match mutation {
                    0 => invalid.content_profile = kind_id("value/text"),
                    1 => invalid.maximum_bytes = 4097,
                    2 => invalid.maximum_bytes = 0,
                    3 => invalid.maximum_items = 2,
                    4 => invalid.generation_slots = 2,
                    5 => invalid.reader_leases = 2,
                    6 => invalid.retention = ResourceRetention::Boot,
                    7 => invalid.sharing = ResourceSharing::ImmutableReadMany,
                    8 => invalid.publication_slots = u16::from(!write),
                    9 => invalid.identity = ResourceSemanticIdentity::from_digest([0; 32]),
                    10 => invalid.version = ResourceVersionIdentity::from_digest([0; 32]),
                    _ => unreachable!(),
                }
                assert!(
                    json_snapshot_offer(invalid).is_err(),
                    "write={write}, invalid contract case={mutation}"
                );
            }
            let mut smallest = content(write);
            smallest.maximum_bytes = 1;
            assert!(json_snapshot_offer(smallest).is_ok());
        }
    }
}
