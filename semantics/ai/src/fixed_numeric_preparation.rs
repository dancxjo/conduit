//! Exact selected affine Back identity and preparation-time resource access.
use crate::fixed_numeric_catalog::fixed_numeric_contracts;
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_data::{TensorBacking, TensorValue};

pub const AFFINE_IMPLEMENTATION: &str = "conduit.numeric/scalar-resource-affine@1";
pub const AFFINE_PROFILE: &str = "conduit.numeric/finite-ieee754-f32-reference@1";
pub const TENSOR_READ_AUTHORITY: &str = "conduit.numeric/read-immutable-tensor@1";
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedPlannedRefusal {
    UnsupportedShape,
    Identity,
    Resource(ResourceReferenceAccessRefusal),
    ResourceRequired,
}

pub fn fixed_affine_offer<const INPUT: usize, const OUTPUT: usize>(
) -> Result<CapabilityOffer, String> {
    let id = format!("numeric/dense{INPUT}x{OUTPUT}");
    let kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == id)
        .ok_or_else(|| format!("unsupported fixed affine shape {INPUT}x{OUTPUT}"))?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!("{AFFINE_IMPLEMENTATION}/{INPUT}x{OUTPUT}")),
            execution_profile_id: ExecutionProfileId::from(AFFINE_PROFILE),
            implementation_id: ImplementationId::from(AFFINE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(AFFINE_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
/// Pure computation reads only borrowed bytes already adopted under these
/// exact preparation access grants. It never performs resource dereferencing
/// during Step. The model-selection owner retains those adoption receipts.
pub fn admit_tensor_access(
    tensor: &TensorValue,
    binding: &ResourceReferenceBinding,
) -> Result<AdmittedResourceAccess, FixedPlannedRefusal> {
    let TensorBacking::Resource(reference) = &tensor.backing else {
        return Err(FixedPlannedRefusal::ResourceRequired);
    };
    ResourceDereferenceRequirement {
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        authority_contract: AuthorityContractId::from(TENSOR_READ_AUTHORITY),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
    }
    .admit(reference, binding)
    .map_err(FixedPlannedRefusal::Resource)
}
pub fn verify_affine_placement<const INPUT: usize, const OUTPUT: usize>(
    placement: &PlannedGear,
) -> Result<(), FixedPlannedRefusal> {
    let expected =
        fixed_affine_offer::<INPUT, OUTPUT>().map_err(|_| FixedPlannedRefusal::UnsupportedShape)?;
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.capability_id != expected.capability_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.limits != expected.limits
        || placement.semantic_contract != expected.semantic_contract
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.pool_references.is_empty()
        || !placement.terminal_transductions.is_empty()
    {
        return Err(FixedPlannedRefusal::Identity);
    }
    Ok(())
}
