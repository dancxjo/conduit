//! Exact Source-bound installed Make expressions. Digests describe data, not origin.
use super::*;
#[derive(Clone, Copy)]
pub struct ResidentExpressionMaterial {
    pub source: ResidentSourceIdentity,
    pub node: u16,
    pub key: c::AssignedIdentity,
    pub text_digest: [u8; 32],
    pub program_digest: [u8; 32],
    pub material_digest: [u8; 32],
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
    pub bytes: &'static [u8],
}
impl<V: ValueStorage> PreparedResidentSoftware<V> {
    /// Initialize from one independently authenticated complete checked Make
    /// artifact, preserving the actual affine value backing on every refusal.
    /// # Safety
    /// Root must authenticate ALL linked material against its checked Make
    /// origin and actual Source/configuration independently of received data.
    /// Matching caller-supplied digests or a claimed Source cannot establish
    /// this origin. No execution, membership or native possession is granted.
    pub unsafe fn prepare_source_with_linked_make_retained(
        resident: &ResidentSoftwareSpec<'_>,
        texts: &[ResidentComputation<'_>],
        schemas: &[ResidentSelectorSchemas],
        materials: &[ResidentExpressionMaterial],
        values: V,
    ) -> Result<Self, (NativePreparationRefusal, V)> {
        Self::prepare_source_inner(resident, texts, Some(schemas), Some(materials), values)
    }
}
pub(super) fn validate(
    construction: &Construction,
    texts: &[ResidentComputation<'_>],
    materials: &[ResidentExpressionMaterial],
) -> Result<(), NativePreparationRefusal> {
    use NativePreparationRefusal as E;
    let key = c::AssignedIdentity::from_text("program");
    let expected = construction.computations.iter().filter(|c| c.key == key);
    if materials.len() != expected.clone().count() {
        return Err(E::Configuration);
    }
    for computation in expected {
        let mut matching = materials
            .iter()
            .filter(|m| m.node == computation.node && m.key == computation.key);
        let material = matching.next().ok_or(E::Configuration)?;
        let text = texts
            .iter()
            .find(|t| t.node == computation.node && t.key == computation.key)
            .ok_or(E::Configuration)?;
        let canonical = text.canonical_bytes.ok_or(E::Configuration)?;
        if matching.next().is_some()
            || material.source != construction.source
            || material.text_digest != computation.text_digest
            || c::assigned_plan_payload_digest(canonical) != material.program_digest
            || c::assigned_plan_payload_digest(material.bytes) != material.material_digest
        {
            return Err(E::Configuration);
        }
    }
    Ok(())
}
