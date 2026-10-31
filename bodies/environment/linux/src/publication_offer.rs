//! Descriptor only: Root retains the initialized generation and real directory,
//! admits capacity and supplies current authority before advertising readiness.
use conduit_core as c;
pub const RECORDING_PUBLICATION_PROFILE: &str = "environment/linux-recording-publication@1";
pub const RECORDING_PUBLICATION_IMPLEMENTATION: &str = "environment/linux-recording-publication@1";
pub const RECORDING_PUBLICATION_ARTIFACT: &str =
    "conduit-environment-linux/recording-publication@1";
pub fn recording_publication_offer(
    resource: &c::ResourceOffer,
) -> Result<c::CapabilityOffer, String> {
    let content = resource
        .content
        .as_ref()
        .ok_or("recording content contract absent")?;
    content
        .validate()
        .map_err(|error| format!("recording content: {error:?}"))?;
    if resource.class_id.as_str() != "resource/snapshot@1"
        || resource.capacity_units == 0
        || content.contract.content_profile.as_str() != "text/json-utf8@1"
        || content.contract.retention != c::ResourceRetention::ExternalDurable
        || content.contract.access != c::ResourceAccessMode::WriteCandidatePublish
        || content.contract.maximum_bytes > 4096
        || content.contract.maximum_items != 1
    {
        return Err("recording publication resource contract".into());
    }
    let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(true);
    let subject = kind.kind_id.clone();
    let operation = conduit_linux_snapshot_store::PUBLISH_OPERATION;
    Ok(c::BackOfferBuilder::new(
        kind,
        c::Back {
            capability_id: RECORDING_PUBLICATION_IMPLEMENTATION.into(),
            execution_profile_id: RECORDING_PUBLICATION_PROFILE.into(),
            implementation_id: RECORDING_PUBLICATION_IMPLEMENTATION.into(),
            artifact_id: RECORDING_PUBLICATION_ARTIFACT.into(),
            host_calls: vec![c::HostCallRequirement {
                contract_id: operation.into(),
                target_kind: Some(subject.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: 4096,
                maximum_output_bytes: 512,
            }],
            resource_requirements: vec![c::ResourceRequirement {
                class_id: resource.class_id.clone(),
                units: 1,
                compute: None,
                protected_role: None,
                content: Some(content.contract.clone()),
            }],
            authority_requirements: vec![c::AuthorityRequirement {
                contract_id: conduit_linux_snapshot_store::AUTHORITY_CONTRACT.into(),
                host_call_contract_id: operation.into(),
                subject_kind: subject,
            }],
        },
    )
    .build())
}
