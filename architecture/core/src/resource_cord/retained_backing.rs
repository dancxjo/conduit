//! Read-only payload inspection of an actual accepted local resource owner.
use super::*;
impl AcceptedResourceCord {
    /// Actual heap payload for the closed unprotected, noncompute local owner.
    /// Other resource binding owners refuse rather than silently undercharge.
    /// The opaque handle is inline and has no heap allocation.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        if self.binding.protected.is_some()
            || self.binding.compute.is_some()
            || self.binding.content.is_some()
        {
            return None;
        }
        let mut bytes = 0usize;
        for text in [
            &self.plan_id.0,
            &self.active_play_id.0,
            &self.connection_id.0,
            &self.source_placement_id.0,
            &self.sink_placement_id.0,
            &self.sink_port_id.0,
            &self.contract.port_id.0,
            &self.contract.class_id.0,
            &self.binding.pool_id.0,
            &self.binding.class_id.0,
        ] {
            bytes = bytes.checked_add(text.capacity())?;
        }
        Some(bytes)
    }
}
