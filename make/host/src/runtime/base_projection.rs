use conduit_core::{
    BaseLifecycle, BaseProviderEntry, BaseRegistry, BaseRegistryLimits, BaseRegistryRefusal,
    HostAdvertisement,
};

use super::{
    capability_is_ready, resource_is_ready, BuildManifest, MakeCatalog, RuntimeBindingDiagnostic,
    RuntimeFacts,
};

pub(super) fn project_ready_bases(
    manifest: &BuildManifest,
    catalog: &MakeCatalog,
    facts: &RuntimeFacts,
    candidates: Vec<BaseProviderEntry>,
    advertisement: &mut HostAdvertisement,
) -> Result<(), RuntimeBindingDiagnostic> {
    let maximum_bases = u16::try_from(candidates.len().max(1)).map_err(|_| {
        RuntimeBindingDiagnostic::InvalidBaseProvider(BaseRegistryRefusal::RegistryFull)
    })?;
    let maximum_capabilities = candidates
        .iter()
        .map(|entry| entry.capabilities.len())
        .max()
        .unwrap_or(0);
    let maximum_resources = candidates
        .iter()
        .map(|entry| entry.resources.len())
        .max()
        .unwrap_or(0);
    let advertised_capabilities = candidates
        .iter()
        .map(|entry| entry.capabilities.len())
        .sum::<usize>()
        .saturating_add(advertisement.capabilities.len());
    let advertised_resources = candidates
        .iter()
        .map(|entry| entry.resources.len())
        .sum::<usize>()
        .saturating_add(advertisement.resources.len());
    let limits = BaseRegistryLimits {
        maximum_bases,
        maximum_capabilities_per_base: u16::try_from(maximum_capabilities).map_err(|_| {
            RuntimeBindingDiagnostic::InvalidBaseProvider(BaseRegistryRefusal::TooManyCapabilities)
        })?,
        maximum_resources_per_base: u16::try_from(maximum_resources).map_err(|_| {
            RuntimeBindingDiagnostic::InvalidBaseProvider(BaseRegistryRefusal::TooManyResources)
        })?,
        maximum_advertised_capabilities: u16::try_from(advertised_capabilities.max(1)).map_err(
            |_| {
                RuntimeBindingDiagnostic::InvalidBaseProvider(
                    BaseRegistryRefusal::AdvertisementCapacity,
                )
            },
        )?,
        maximum_advertised_resources: u16::try_from(advertised_resources.max(1)).map_err(|_| {
            RuntimeBindingDiagnostic::InvalidBaseProvider(
                BaseRegistryRefusal::AdvertisementCapacity,
            )
        })?,
    };
    validate_candidates(manifest, &candidates, limits)?;

    let ready_resources = candidates
        .iter()
        .filter(|entry| entry.lifecycle == BaseLifecycle::Ready)
        .flat_map(|entry| entry.resources.iter())
        .filter(|resource| resource_is_ready(manifest, resource, facts))
        .cloned()
        .chain(advertisement.resources.iter().cloned())
        .collect::<Vec<_>>();
    let mut ready =
        BaseRegistry::new(limits).map_err(RuntimeBindingDiagnostic::InvalidBaseProvider)?;
    for candidate in candidates {
        if candidate.lifecycle != BaseLifecycle::Ready {
            continue;
        }
        let resources_ready = candidate
            .resources
            .iter()
            .all(|resource| resource_is_ready(manifest, resource, facts));
        let capabilities_ready = candidate
            .capabilities
            .iter()
            .all(|offer| capability_is_ready(manifest, catalog, offer, &ready_resources, facts));
        if resources_ready && capabilities_ready {
            ready
                .register(candidate)
                .map_err(RuntimeBindingDiagnostic::InvalidBaseProvider)?;
        }
    }
    ready
        .project_ready_into(advertisement)
        .map_err(RuntimeBindingDiagnostic::InvalidBaseProvider)
}

fn validate_candidates(
    manifest: &BuildManifest,
    candidates: &[BaseProviderEntry],
    limits: BaseRegistryLimits,
) -> Result<(), RuntimeBindingDiagnostic> {
    let mut validated =
        BaseRegistry::new(limits).map_err(RuntimeBindingDiagnostic::InvalidBaseProvider)?;
    for candidate in candidates {
        validated
            .register(candidate.clone())
            .map_err(RuntimeBindingDiagnostic::InvalidBaseProvider)?;
        if !base_is_built(manifest, candidate) {
            return Err(RuntimeBindingDiagnostic::UnexpectedBaseImplementation {
                kind: candidate.mechanism_family.as_str().into(),
                implementation: candidate.implementation_id.as_str().into(),
            });
        }
    }
    Ok(())
}

fn base_is_built(manifest: &BuildManifest, entry: &BaseProviderEntry) -> bool {
    manifest.base_selections.iter().any(|selection| {
        selection.kind == entry.mechanism_family.as_str()
            && selection.driver == entry.implementation_id.as_str()
    })
}
