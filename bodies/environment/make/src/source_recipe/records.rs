//! Source-only canonical computation records for eventual borrowed admission.
//! These are trusted compiler material, never native readiness or assignment
//! acceptance. Graph, current scope, integrity and authority remain separate.
use super::*;
pub(super) fn configuration_payloads(
    texts: &[(u16, c::AssignedIdentity, String, Vec<u8>)],
) -> Vec<Vec<u8>> {
    texts
        .iter()
        .map(|(node, key, text, _)| {
            let mut payload = Vec::with_capacity(19 + text.len());
            payload.extend_from_slice(&node.to_le_bytes());
            payload.extend_from_slice(&key.0);
            payload.push(3); // Existing assigned configuration Text representation.
            payload.extend_from_slice(text.as_bytes());
            payload
        })
        .collect()
}
impl NativeSoftwareSourceRecipe {
    /// Exact standard configuration payloads derived from checked Source,
    /// before any Host/Boot/Plan exists. Borrowing these initializes no owner.
    pub fn resident_computation_records(
        &self,
    ) -> impl ExactSizeIterator<Item = c::AssignedRecord<'_>> {
        self.configuration_records
            .iter()
            .map(|payload| c::AssignedRecord {
                tag: c::ASSIGNED_CONFIGURATION,
                payload,
            })
    }
    pub(super) fn write_computation_records(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(
            out,
            "static COMPUTATION_RECORDS: [c::AssignedRecord<'static>; {}] = [",
            self.configuration_records.len()
        )?;
        for record in self.resident_computation_records() {
            writeln!(
                out,
                "c::AssignedRecord {{ tag: {}, payload: &{:?} }},",
                record.tag, record.payload
            )?;
        }
        writeln!(out, "];")?;
        writeln!(out, "pub fn computation_records() -> &'static [c::AssignedRecord<'static>] {{ &COMPUTATION_RECORDS }}")
    }
}
