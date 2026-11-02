//! Explicit operation selection. Construction describes inclusion, never readiness.
use crate::*;

#[derive(Default)]
pub(super) struct ResolvedOperations {
    pub capabilities: Vec<CapabilitySelection>,
    pub host_calls: Vec<String>,
    pub facilities: Vec<String>,
}

pub(super) fn resolve(
    configuration: &HostConfiguration,
    target: &str,
    bases: &[(String, String)],
    catalog: &MakeCatalog,
    packages: &MakePackageSet,
) -> Result<ResolvedOperations, Vec<ConfigurationDiagnostic>> {
    if configuration.capabilities.is_empty() {
        return Ok(ResolvedOperations::default());
    }
    let refuse = |detail| vec![ConfigurationDiagnostic::Capability { detail }];
    let review = resolve_host_capability_intent(
        &HostCapabilityIntent {
            target: target.into(),
            capabilities: configuration.capabilities.clone(),
        },
        catalog,
        packages,
    )
    .map_err(|error| refuse(format!("{error:?}")))?;
    let mut selections = Vec::new();
    let mut calls = Vec::new();
    let mut facilities = Vec::new();
    for operation in review.capabilities {
        let mut compatible = operation.candidates.into_iter().filter(|candidate| {
            let requirements = &candidate.prerequisites;
            requirements.bases.iter().all(|required| {
                bases.iter().any(|(kind, implementation)| {
                    kind == &required.base_kind
                        && required
                            .compatible_implementations
                            .iter()
                            .any(|item| &item.implementation_id == implementation)
                })
            }) && requirements.drivers.iter().all(|driver| {
                bases
                    .iter()
                    .any(|(_, implementation)| implementation == driver)
            }) && requirements.resources.iter().all(|class| {
                configuration
                    .resources
                    .iter()
                    .any(|budget| &budget.class == class && budget.slots > 0 && budget.bytes > 0)
            })
        });
        let candidate = compatible.next().ok_or_else(|| {
            refuse(format!(
                "{} has no implementation with explicitly selected Base/driver/resource budgets",
                operation.kind
            ))
        })?;
        if compatible.next().is_some() {
            return Err(refuse(format!(
                "{} is ambiguous; explicitly pin its implementation",
                operation.kind
            )));
        }
        selections.push(CapabilitySelection {
            kind: operation.kind,
            contract_revision: candidate.contract_revision,
            implementation: candidate.implementation_id,
        });
        calls.extend(candidate.prerequisites.host_calls);
        facilities.extend(candidate.prerequisites.facilities);
    }
    selections.sort_by(|a, b| a.kind.cmp(&b.kind));
    Ok(ResolvedOperations {
        capabilities: selections,
        host_calls: calls,
        facilities,
    })
}
