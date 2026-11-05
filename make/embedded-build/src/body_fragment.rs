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
    let metadata = conduit_body::BodyFragmentApprovalMetadata::from_prepared(scope, &projection)
        .map_err(E::Document)?;
    let bytes = conduit_body::encode_body_fragment_document(
        &metadata,
        &projection,
        limits.maximum_document_bytes,
    )
    .map_err(E::Document)?;
    Ok(GeneratedBodyFragmentDocument { bytes })
}
