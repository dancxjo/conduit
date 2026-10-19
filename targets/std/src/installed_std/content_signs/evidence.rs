//! Post-Play export of bytes actually retained at the routing seam.
use super::*;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedRoutedValue {
    pub binding: ContentSignBinding,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContentEvidenceReport {
    pub admission: Option<ContentSignAdmission>,
    pub bindings: Vec<ContentSignBinding>,
    pub retained_values: Vec<RetainedRoutedValue>,
}
pub(in crate::installed_std) fn capture_report(
    signs: &StdSignLog,
) -> Result<ContentEvidenceReport, String> {
    let admission = signs.admission().copied();
    let bindings: Vec<_> = signs.bindings().collect();
    let mut retained_values = Vec::new();
    if let Some(admission) = admission.filter(|a| a.value_storage_bytes.is_some()) {
        retained_values.reserve_exact(bindings.len());
        for binding in &bindings {
            let bytes = signs
                .retained_value(binding.event_sequence)
                .ok_or("retained content binding lacks its admitted value bytes")?;
            if bytes.len() != binding.byte_length as usize
                || conduit_core::semantic_digest(admission.digest_domain, bytes) != binding.digest
            {
                return Err("retained value differs from its routing content binding".into());
            }
            retained_values.push(RetainedRoutedValue {
                binding: *binding,
                bytes: bytes.to_vec(),
            });
        }
    }
    Ok(ContentEvidenceReport {
        admission,
        bindings,
        retained_values,
    })
}
