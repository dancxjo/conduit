use conduit_core::*;
use conduit_planner::proof::resource_frame::*;
use conduit_std_host::hosted_resource::HostedResourceGeneration;

pub fn provider(
    binding: &ResourceBinding,
    read: &AuthorityGrantId,
    other: Option<&AuthorityGrantId>,
    writer: Option<&AuthorityGrantId>,
) -> HostedResourceGeneration {
    provider_with_handle(
        binding,
        read,
        other,
        writer,
        ResourceHandleId::from(format!("handle/{}", binding.pool_id.as_str())),
    )
}

pub fn provider_with_handle(
    binding: &ResourceBinding,
    read: &AuthorityGrantId,
    other: Option<&AuthorityGrantId>,
    writer: Option<&AuthorityGrantId>,
    handle: ResourceHandleId,
) -> HostedResourceGeneration {
    let c = &binding.content.as_ref().unwrap().contract;
    let reference = BoundedResourceRef {
        identity: c.identity,
        content_profile: c.content_profile.clone(),
        access_class: binding.class_id.clone(),
        extent: ResourceExtent {
            bytes: u64::from(FRAME_BYTES),
            items: Some(65536),
        },
        lifetime: ResourceLifetime {
            version: c.version,
            expires_at: None,
        },
    };
    let requirement = ResourceDereferenceRequirement {
        content_profile: c.content_profile.clone(),
        access_class: binding.class_id.clone(),
        authority_contract: AuthorityContractId::from(FRAME_AUTHORITY),
        maximum_bytes: u64::from(FRAME_BYTES),
        maximum_items: Some(65536),
    };
    let access = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: c.content_profile.clone(),
        access_class: binding.class_id.clone(),
        handle,
        authority_contract: requirement.authority_contract.clone(),
        authority_grant: read.clone(),
        maximum_bytes: u64::from(FRAME_BYTES),
        maximum_items: Some(65536),
        availability: ResourceReferenceAvailability::Available,
    };
    let additional = other
        .map(|grant| {
            let mut a = access.clone();
            a.authority_grant = grant.clone();
            a
        })
        .into_iter()
        .collect();
    HostedResourceGeneration::new(binding, reference, requirement, access, writer.cloned())
        .unwrap()
        .with_readers(additional)
        .unwrap()
}
