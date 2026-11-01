//! Optional cooperative Linux snapshot realization. Root retains the actual
//! directory, exact resource residence, independent grants and complete capacity
//! admission. Constructing this descriptor creates none of those facts.
use conduit_core as c;
pub use conduit_linux_snapshot_store::SnapshotDirectory;
pub const PUBLISH: &str = "std/linux-snapshot-publish@1";
pub const READ: &str = "std/linux-snapshot-read@1";
pub const PROFILE: &str = "std/linux-snapshot@1";
pub const ARTIFACT: &str = "conduit-std-host/linux-snapshot@1";
pub(crate) fn offer(publish: bool) -> c::CapabilityOffer {
    let contract = conduit_semantic_catalog::resource_snapshot_semantic_contract(publish);
    let kind = contract.kind_id.clone();
    let implementation = if publish { PUBLISH } else { READ };
    let operation = if publish {
        conduit_linux_snapshot_store::PUBLISH_OPERATION
    } else {
        conduit_linux_snapshot_store::READ_OPERATION
    };
    c::BackOfferBuilder::new(
        contract,
        c::Back {
            capability_id: implementation.into(),
            execution_profile_id: PROFILE.into(),
            implementation_id: implementation.into(),
            artifact_id: ARTIFACT.into(),
            host_calls: vec![c::HostCallRequirement {
                contract_id: operation.into(),
                target_kind: Some(kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: if publish { 4096 } else { 512 },
                maximum_output_bytes: if publish { 512 } else { 4096 },
            }],
            resource_requirements: vec![],
            authority_requirements: vec![c::AuthorityRequirement {
                contract_id: conduit_linux_snapshot_store::AUTHORITY_CONTRACT.into(),
                host_call_contract_id: operation.into(),
                subject_kind: kind,
            }],
        },
    )
    .build()
}

/// Describe one independently selected resource. Root must retain its actual
/// directory and admit capacity before exposing this offer; this issues no grant.
pub fn offer_for_resource(resource: &c::ResourceOffer) -> Result<c::CapabilityOffer, String> {
    let content = resource
        .content
        .as_ref()
        .ok_or("snapshot content contract absent")?;
    content
        .validate()
        .map_err(|error| format!("snapshot content: {error:?}"))?;
    if content.contract.retention != c::ResourceRetention::ExternalDurable {
        return Err("snapshot resource residence contract".into());
    }
    let mut offer = offer(content.contract.access == c::ResourceAccessMode::WriteCandidatePublish);
    offer.resource_requirements.push(c::ResourceRequirement {
        class_id: resource.class_id.clone(),
        units: 1,
        compute: None,
        protected_role: None,
        content: Some(content.contract.clone()),
    });
    Ok(offer)
}
