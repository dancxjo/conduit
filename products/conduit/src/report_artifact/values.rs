//! Actual retained values require one exact Core routing Sign before export.
use conduit_observatory::{RoutedValueArtifact, SignArtifact};
pub(super) fn build(
    signs: &[SignArtifact],
    values: &[crate::product_execution::RuntimeRoutedValue],
) -> Result<Vec<RoutedValueArtifact>, String> {
    let mut artifacts = Vec::with_capacity(values.len());
    for value in values {
        let mut matching = signs.iter().filter(|sign| {
            sign.sign.active_play_id.as_ref() == Some(&value.active_play_id)
                && matches!(&sign.sign.kind, conduit_core::ObservationKind::RoutedContent { evidence }
                    if evidence.native_event_sequence == value.value.binding.event_sequence
                        && evidence.digest == value.value.binding.digest
                        && evidence.byte_length == value.value.binding.byte_length)
        });
        let sign = matching.next().ok_or("captured value lacks a retained Core routing Sign; artifact export does not support this routing context")?;
        if matching.next().is_some() {
            return Err("captured value has ambiguous routing Sign context".into());
        }
        if artifacts
            .iter()
            .any(|a: &RoutedValueArtifact| a.sign.identity.sign_id == sign.identity.sign_id)
        {
            return Err("captured value duplicates an execution Sign".into());
        }
        artifacts.push(RoutedValueArtifact::new(
            sign.clone(),
            value.value.bytes.clone(),
        )?);
    }
    Ok(artifacts)
}
