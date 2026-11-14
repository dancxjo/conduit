//! Derive descriptive compiler metadata; independent Root authority is required.
use conduit_core::*;
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use conduit_plot::SourceCheckNativeSchema;
use conduit_std_host::{
    source_check::{PreparedSourceChecker, SourceCheckPreparationBounds},
    NativeSourceDefinition, StdHostConfig,
};
use conduit_std_offers::*;

/// Current independently issued provider identity, never inferred from IMAGE.
pub struct SourceCheckProviderIdentity {
    pub instance: BaseInstanceId,
    pub generation: u64,
    pub resource_generation: ResourceGenerationId,
}

/// Derive the selected compiler's finite metadata before acquiring Source bytes.
/// This validates, but never issues, the supplied current Host authorization.
/// The result is construction intent until funded native initialization succeeds.
pub fn derive_image_definition(
    config: &StdHostConfig,
    image: &HostImage,
    checked: &CheckedHostConfiguration,
    catalog: &MakeCatalog,
    schema: &SourceCheckNativeSchema,
    grant: AuthorityGrant,
    provider: SourceCheckProviderIdentity,
) -> Result<NativeSourceDefinition, String> {
    conduit_host_make::verify_configuration_image(checked, image, catalog)?;
    if config.host_id.as_str().is_empty()
        || config.boot_id.as_str().is_empty()
        || config.offer_generation.0 == 0
        || provider.instance.as_str().is_empty()
        || provider.generation == 0
        || provider.resource_generation.0.is_empty()
        || grant.grant_id.as_str().is_empty()
        || grant.host_id != config.host_id
        || grant.boot_id != config.boot_id
        || grant.capability_id.as_str() != SOURCE_CHECK_IMPLEMENTATION
        || grant.contract_id.as_str() != SOURCE_CHECK_AUTHORITY
        || grant.host_call_contract_id.as_str() != SOURCE_CHECK_CALL
        || grant.subject_kind.as_str() != SOURCE_CHECK_KIND
    {
        return Err(
            "selected Source compiler requires independent current provider and grant".into(),
        );
    }
    let selected =
        conduit_workspace_make::CheckedSourceCheckInitialization::from_configuration(checked)?;
    let mut bases = image
        .manifest
        .base_selections
        .iter()
        .filter(|b| b.kind == SOURCE_CHECK_BASE_KIND);
    let base = bases.next().ok_or("IMAGE lacks selected Source compiler")?;
    if bases.next().is_some() || base.driver != SOURCE_CHECK_IMPLEMENTATION {
        return Err("IMAGE requires one exact Source compiler Base".into());
    }
    let mut capabilities = checked
        .profile()
        .capabilities
        .iter()
        .filter(|i| i.kind == SOURCE_CHECK_KIND);
    let capability = capabilities
        .next()
        .ok_or("IMAGE lacks Source-check capability")?;
    if capabilities.next().is_some()
        || capability.implementation != SOURCE_CHECK_IMPLEMENTATION
        || capability.contract_revision != SOURCE_CHECK_CALL
        || image
            .manifest
            .implementations
            .iter()
            .filter(|i| i.as_str() == SOURCE_CHECK_IMPLEMENTATION)
            .count()
            != 1
    {
        return Err("IMAGE requires one exact Source-check capability".into());
    }
    let pool = |class: &str| -> Result<ResourceOffer, String> {
        let mut pools = image
            .manifest
            .resource_budgets
            .iter()
            .filter(|p| p.class == class);
        let pool = pools.next().ok_or("IMAGE lacks compiler resource pool")?;
        if pools.next().is_some()
            || pool.id.is_empty()
            || image
                .manifest
                .resource_budgets
                .iter()
                .filter(|p| p.id == pool.id)
                .count()
                != 1
            || pool.bytes
                < if class == SOURCE_CHECK_RESOURCE {
                    selected.retained_bytes
                } else {
                    selected.preparation_bytes
                }
            || (class == SOURCE_CHECK_RESOURCE && pool.slots != 1)
            || (class == RUNTIME_MEMORY_RESOURCE_CLASS
                && (u64::from(pool.slots) < selected.preparation_bytes
                    || u64::from(pool.slots) > image.manifest.bounds.heap_arena_bytes))
        {
            return Err("IMAGE compiler resource pool is ambiguous or insufficient".into());
        }
        Ok(ResourceOffer {
            pool_id: pool.id.as_str().into(),
            class_id: class.into(),
            capacity_units: pool.slots,
            compute: None,
            content: None,
        })
    };
    let resources = vec![
        pool(SOURCE_CHECK_RESOURCE)?,
        pool(RUNTIME_MEMORY_RESOURCE_CLASS)?,
    ];
    let bounds = SourceCheckPreparationBounds {
        request_bytes: selected
            .request_bytes
            .try_into()
            .map_err(|_| "request bound overflow")?,
        report_bytes: selected
            .reply_bytes
            .try_into()
            .map_err(|_| "reply bound overflow")?,
        retained_bytes: selected
            .retained_bytes
            .try_into()
            .map_err(|_| "retained bound overflow")?,
    };
    let kind = PreparedSourceChecker::kind_for_bounds(schema, bounds)?;
    let offer = conduit_core::capability_offer_from_parts! {
        startup_parameters: kind.startup_parameters.clone(), shorthand: kind.shorthand.clone(),
        capability_id: SOURCE_CHECK_IMPLEMENTATION.into(), kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(), semantic_contract: kind.semantic_contract(),
        implementation: ImplementationOffer {
            execution_profile_id: SOURCE_CHECK_IMPLEMENTATION.into(),
            implementation_id: SOURCE_CHECK_IMPLEMENTATION.into(),
            artifact_id: image.manifest.image_id.as_str().into(),
        },
        host_calls: vec![HostCallRequirement { contract_id: SOURCE_CHECK_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: bounds.request_bytes, maximum_output_bytes: bounds.report_bytes }],
        resource_requirements: vec![ResourceRequirement { class_id: SOURCE_CHECK_RESOURCE.into(), units: 1, protected_role: None, compute: None, content: None }],
        authority_requirements: vec![AuthorityRequirement { contract_id: SOURCE_CHECK_AUTHORITY.into(), host_call_contract_id: SOURCE_CHECK_CALL.into(), subject_kind: kind.kind_id.clone() }],
        limits: kind.limits.clone(),
    };
    Ok(NativeSourceDefinition {
        kind,
        grant,
        base: BaseProviderEntry {
            base_id: base.id.as_str().into(),
            provider_instance_id: provider.instance,
            provider_generation: provider.generation,
            implementation_id: base.driver.as_str().into(),
            mechanism_family: base.kind.as_str().into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources,
        },
        resource_generation: provider.resource_generation,
        configuration: vec![],
        input_bytes: bounds.request_bytes,
        output_bytes: bounds.report_bytes,
        resource_bytes: selected.retained_bytes,
    })
}
