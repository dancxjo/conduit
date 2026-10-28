//! Conservative refusal policy for an OPTIONAL early recording handoff.
//! This does not enlarge checkpoint capacity or predict future encoded bytes.
pub(crate) const MAXIMUM_EARLY_CHECKPOINT_BYTES: usize = 26_000;
pub(crate) fn early_checkpoint_safe(extent: Option<(u64, usize)>) -> bool {
    matches!(extent, Some((generation, bytes)) if generation > 0 && bytes > 0 && bytes <= MAXIMUM_EARLY_CHECKPOINT_BYTES)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_and_actual_full_checkpoints_refuse_before_optional_cancellation() {
        assert!(!early_checkpoint_safe(None));
        assert!(!early_checkpoint_safe(Some((0, 1000))));
        assert!(!early_checkpoint_safe(Some((620, 0))));
        assert!(!early_checkpoint_safe(Some((620, 26402))));
        assert!(!early_checkpoint_safe(Some((620, 26001))));
        assert!(early_checkpoint_safe(Some((620, 26000))));
        assert!(early_checkpoint_safe(Some((621, 3817))));
    }
}
