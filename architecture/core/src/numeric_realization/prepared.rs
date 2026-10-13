//! Preparation turns a checked numerical witness into one bounded typed output.
use super::*;
use alloc::vec::Vec;

pub struct PreparedNumericOutput {
    witness: VerifiedNumericWitness,
    canonical: Vec<u8>,
}
impl PreparedNumericOutput {
    pub fn new(
        graph: &crate::NumericComputation,
        witness: VerifiedNumericWitness,
    ) -> Result<Self, NumericEngineRefusal> {
        if graph
            .identity()
            .map_err(|_| NumericEngineRefusal::InvalidComputation)?
            != *witness.graph_identity()
        {
            return Err(NumericEngineRefusal::ClaimMismatch);
        }
        let canonical = match &witness.claim().result {
            NumericResultEvidence::ExactMathematical(value) => value.canonical_bytes()?,
            NumericResultEvidence::ExactPhysical(value) => value.encoded().to_vec(),
            NumericResultEvidence::ExactBoolean(value) => alloc::vec![u8::from(*value)],
            NumericResultEvidence::IeeeBits(value) => value.bits().to_le_bytes().to_vec(),
            NumericResultEvidence::Enclosure(interval) => {
                if interval.lower() != interval.upper() {
                    return Err(NumericEngineRefusal::UnsupportedRepresentation);
                }
                interval.lower().canonical_bytes()?
            }
        };
        crate::validate_primitive_info(
            &graph.nodes()[usize::from(graph.root())].semantic_kind,
            &canonical,
        )
        .map_err(|_| NumericEngineRefusal::UnsupportedRepresentation)?;
        if canonical.len() > witness.requirement().budget.storage_bytes as usize {
            return Err(crate::NumericRefusal::StorageBudget.into());
        }
        Ok(Self { witness, canonical })
    }
    pub fn witness(&self) -> &VerifiedNumericWitness {
        &self.witness
    }
    pub fn canonical(&self) -> &[u8] {
        &self.canonical
    }
}
