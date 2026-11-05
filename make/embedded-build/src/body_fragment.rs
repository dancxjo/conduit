//! Authoring-side exact Body containment and ordinary projection in one factory.
use crate::*;
use conduit_body::{BodyFragmentDocumentRefusal, PreparedBodyFragment};

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
    let resources = plain_resources(fragment, generated.kernel()).ok_or(E::Resources)?;
    let metadata =
        conduit_body::BodyFragmentApprovalMetadata::from_prepared(scope, &projection, &resources)
            .map_err(E::Document)?;
    let bytes = conduit_body::encode_body_fragment_document(
        &metadata,
        &projection,
        limits.maximum_document_bytes,
    )
    .map_err(E::Document)?;
    Ok(GeneratedBodyFragmentDocument { bytes })
}

// A deliberately plain profile: preserve original placement/pool/class/units.
// Rich protected/content/compute bindings must retain their full contracts.
fn plain_resources(
    fragment: &conduit_core::PlanFragment,
    generated: &GeneratedEmbeddedPlan,
) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    if generated.resources.len() > conduit_core::MAXIMUM_BINDINGS_PER_ADMISSION {
        return None;
    }
    for r in &generated.resources {
        let node = generated.nodes.iter().find(|n| n.node == r.node)?;
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.placement_id.as_str() == node.placement_id)?;
        let binding = placement.resources.get(usize::from(r.resource))?;
        if binding.units != r.units
            || binding.protected.is_some()
            || binding.compute.is_some()
            || binding.content.is_some()
        {
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
        entry.append_to(&mut bytes).ok()?;
    }
    Some(bytes)
}
