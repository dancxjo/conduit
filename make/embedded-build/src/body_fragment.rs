//! Authoring-side exact Body containment and ordinary projection in one factory.
use crate::*;
use conduit_body::{BodyFragmentDocumentRefusal, PreparedBodyFragment};

#[derive(Clone, Copy)]
pub struct BodyFragmentProjectionLimits {
    pub image: EmbeddedImageBounds,
    pub computations: EmbeddedComputationBounds,
    pub assigned: conduit_core::AssignedPlanMaxima,
    pub maximum_fore_ports: usize,
    pub maximum_document_bytes: u32,
}
#[derive(Debug)]
pub enum BodyFragmentGenerationRefusal {
    Generation(GenerationError),
    Document(BodyFragmentDocumentRefusal),
    Resources,
    Scope,
    Workset(conduit_body::BodyWorksetDocumentRefusal),
    Resident(ResidentExpressionRefusal),
}

/// Canonical complete local workset derived from rich containment evidence.
/// Possession is not signing authority, receiver admission or permission to run.
pub struct GeneratedBodyWorksetDocument {
    bytes: Vec<u8>,
}
impl GeneratedBodyWorksetDocument {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Derive the complete local set rather than accepting a caller-selected list.
/// The resolver selects bounded resident implementations for each actual
/// fragment. Current receiver identity must be independently supplied; this
/// host-side projection never manufactures remote capabilities.
pub fn generate_prepared_body_workset<'a>(
    scope: PreparedBodyFragment<'a>,
    actual: &conduit_core::HostAdvertisement,
    maximum_document_bytes: u32,
    mut resolve: impl FnMut(
        PreparedBodyFragment<'a>,
    ) -> Result<
        (Vec<BoundResidentExpression>, BodyFragmentProjectionLimits),
        BodyFragmentGenerationRefusal,
    >,
) -> Result<GeneratedBodyWorksetDocument, BodyFragmentGenerationRefusal> {
    generate_body_workset(scope, actual, maximum_document_bytes, |local| {
        let (resident, limits) = resolve(local)?;
        generate_prepared_body_fragment(local, &resident, limits)
    })
}

/// Project every original local fragment, preserving exact selected Fore Lines.
/// Sessions select existing sealed routes; they grant no carrier possession.
pub fn generate_prepared_body_workset_with_line_fore<'a>(
    scope: PreparedBodyFragment<'a>,
    actual: &conduit_core::HostAdvertisement,
    maximum_document_bytes: u32,
    mut resolve: impl FnMut(
        PreparedBodyFragment<'a>,
    ) -> Result<
        (
            Vec<BoundResidentExpression>,
            BodyFragmentProjectionLimits,
            Vec<SelectedForeSession>,
        ),
        BodyFragmentGenerationRefusal,
    >,
) -> Result<GeneratedBodyWorksetDocument, BodyFragmentGenerationRefusal> {
    generate_body_workset(scope, actual, maximum_document_bytes, |local| {
        let (resident, limits, sessions) = resolve(local)?;
        generate_prepared_body_fragment_with_line_fore(local, &resident, limits, &sessions)
    })
}

fn generate_body_workset<'a>(
    scope: PreparedBodyFragment<'a>,
    actual: &conduit_core::HostAdvertisement,
    maximum_document_bytes: u32,
    mut project: impl FnMut(
        PreparedBodyFragment<'a>,
    ) -> Result<GeneratedBodyFragmentDocument, BodyFragmentGenerationRefusal>,
) -> Result<GeneratedBodyWorksetDocument, BodyFragmentGenerationRefusal> {
    use BodyFragmentGenerationRefusal as E;
    let count = scope.local_fragments().count();
    if count == 0
        || count > conduit_core::MAXIMUM_RESOURCE_ADMISSIONS
        || maximum_document_bytes == 0
    {
        return Err(E::Workset(conduit_body::BodyWorksetDocumentRefusal::Bounds));
    }
    // Validate the complete selected identity set before invoking a resolver.
    for local in scope.local_fragments() {
        let local = local.map_err(|_| E::Scope)?;
        let fragment = local.fragment();
        if fragment.host_id != actual.host_id
            || fragment.boot_id != actual.boot_id
            || fragment.offer_generation != actual.offer_generation
        {
            return Err(E::Scope);
        }
    }
    let mut documents = Vec::new();
    for local in scope.local_fragments() {
        let local = local.map_err(|_| E::Scope)?;
        documents.push(project(local)?);
    }
    let fragments: Vec<_> = documents.iter().map(|document| document.bytes()).collect();
    let bytes = conduit_body::encode_body_workset_document(&fragments, maximum_document_bytes)
        .map_err(E::Workset)?;
    Ok(GeneratedBodyWorksetDocument { bytes })
}
/// Only this exact factory constructs the sender's immutable document. It is
/// still not a signature, receiver admission or permission to start a Play.
pub struct GeneratedBodyFragmentDocument {
    bytes: Vec<u8>,
}
impl GeneratedBodyFragmentDocument {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn generate_prepared_body_fragment(
    scope: PreparedBodyFragment<'_>,
    resident: &[BoundResidentExpression],
    limits: BodyFragmentProjectionLimits,
) -> Result<GeneratedBodyFragmentDocument, BodyFragmentGenerationRefusal> {
    use BodyFragmentGenerationRefusal as E;
    let fragment = scope.fragment();
    let generated = generate_fore_native_plan(
        scope.plot_plan(),
        &fragment.fragment_id,
        limits.image,
        limits.computations,
        limits.maximum_fore_ports,
    )
    .map_err(E::Generation)?;
    let projection = if resident.is_empty() {
        generated.encode(limits.assigned)
    } else {
        generated.encode_with_resident_computations(limits.assigned, resident)
    }
    .map_err(E::Generation)?;
    finish_document(
        scope,
        generated.kernel(),
        generated.fore(),
        &projection,
        limits,
    )
}

/// An explicit schema-8 projection when sessions exist; ordinary fragments keep
/// schema 7. Missing sessions for a selected Line always refuse, never downgrade.
/// Returned bytes remain unsigned and confer no receiver admission or carrier.
pub fn generate_prepared_body_fragment_with_line_fore(
    scope: PreparedBodyFragment<'_>,
    resident: &[BoundResidentExpression],
    limits: BodyFragmentProjectionLimits,
    sessions: &[SelectedForeSession],
) -> Result<GeneratedBodyFragmentDocument, BodyFragmentGenerationRefusal> {
    if sessions.is_empty() {
        return generate_prepared_body_fragment(scope, resident, limits);
    }
    use BodyFragmentGenerationRefusal as E;
    let generated = generate_line_fore_native_plan(
        scope.plot_plan(),
        &scope.fragment().fragment_id,
        limits.image,
        limits.computations,
        limits.maximum_fore_ports,
        sessions,
    )
    .map_err(E::Generation)?;
    let projection = if resident.is_empty() {
        generated.encode(limits.assigned)
    } else {
        generated.encode_with_resident_computations(limits.assigned, resident)
    }
    .map_err(E::Generation)?;
    finish_document(
        scope,
        generated.kernel(),
        generated.fore(),
        &projection,
        limits,
    )
}

fn finish_document(
    scope: PreparedBodyFragment<'_>,
    kernel: &GeneratedEmbeddedPlan,
    inventory: &[conduit_core::AssignedForeRequirement],
    projection: &[u8],
    limits: BodyFragmentProjectionLimits,
) -> Result<GeneratedBodyFragmentDocument, BodyFragmentGenerationRefusal> {
    use BodyFragmentGenerationRefusal as E;
    let resources = projected_resources(scope.fragment(), kernel).ok_or(E::Resources)?;
    let fore: Vec<_> = inventory
        .iter()
        .flat_map(|p| p.payload().iter().copied())
        .collect();
    let metadata = conduit_body::BodyFragmentApprovalMetadata::from_prepared(
        scope, projection, &resources, &fore,
    )
    .map_err(E::Document)?;
    let bytes = conduit_body::encode_body_fragment_document(
        &metadata,
        projection,
        limits.maximum_document_bytes,
    )
    .map_err(E::Document)?;
    Ok(GeneratedBodyFragmentDocument { bytes })
}

// Retain the full signed content contract; plain-only documents keep their
// existing representation. Protected and compute bindings remain unsupported.
fn projected_resources(
    fragment: &conduit_core::PlanFragment,
    generated: &GeneratedEmbeddedPlan,
) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    if generated.resources.len() > conduit_core::MAXIMUM_BINDINGS_PER_ADMISSION {
        return None;
    }
    let versioned = fragment
        .placements
        .iter()
        .any(|p| p.resources.iter().any(|r| r.content.is_some()));
    for r in &generated.resources {
        let node = generated.nodes.iter().find(|n| n.node == r.node)?;
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.placement_id.as_str() == node.placement_id)?;
        let binding = placement.resources.get(usize::from(r.resource))?;
        if binding.units != r.units || binding.protected.is_some() || binding.compute.is_some() {
            return None;
        }
        let entry = conduit_body::PlainFragmentResource {
            node: r.node,
            resource: r.resource,
            placement: placement.placement_id.as_str(),
            pool: binding.pool_id.as_str(),
            class: binding.class_id.as_str(),
            units: r.units,
        };
        if versioned {
            let content = if let Some(content) = binding.content.as_ref() {
                if content.owner_host != fragment.host_id || content.owner_boot != fragment.boot_id
                {
                    return None;
                }
                Some(conduit_body::FragmentContent {
                    contract: conduit_body::FragmentContentContract::from_core(&content.contract),
                    base: content.base_id.as_str(),
                    residence_profile: content.residence_profile.as_str(),
                })
            } else {
                None
            };
            conduit_body::FragmentResource {
                binding: entry,
                content,
            }
            .append_to(&mut bytes)
            .ok()?;
        } else {
            entry.append_to(&mut bytes).ok()?;
        }
    }
    Some(bytes)
}

#[cfg(test)]
#[path = "body_fragment_tests.rs"]
mod tests;
