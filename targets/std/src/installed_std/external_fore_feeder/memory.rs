//! Newly owned ingress metadata; supplied value bytes remain caller-owned.
use super::*;
impl PreparedForeInputs<'_> {
    /// Actual one-shot Vec capacities, including fan-out targets and spare
    /// slots. Borrowed input payloads are excluded. This is not admission or a
    /// validation-workspace receipt; streaming/live profiles refuse explicitly.
    pub(in crate::installed_std) fn one_shot_backing_bytes(&self) -> Option<usize> {
        let Self::Single(entries) = self else {
            return None;
        };
        let mut bytes = entries
            .capacity()
            .checked_mul(core::mem::size_of::<SingleInput<'_>>())?;
        for entry in entries {
            bytes = bytes.checked_add(
                entry
                    .targets
                    .capacity()
                    .checked_mul(core::mem::size_of::<(RemoteEndpointId, CordId)>())?,
            )?;
        }
        Some(bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_ingress_counts_owned_spare_targets_and_excludes_borrowed_payload() {
        let payload = vec![7; 4096];
        let mut targets = Vec::with_capacity(11);
        targets.push((RemoteEndpointId(0), CordId(0)));
        let mut entries = Vec::with_capacity(5);
        let expected = entries.capacity() * core::mem::size_of::<SingleInput<'_>>()
            + targets.capacity() * core::mem::size_of::<(RemoteEndpointId, CordId)>();
        entries.push(SingleInput {
            bytes: &payload,
            targets,
        });
        let prepared = PreparedForeInputs::Single(entries);
        assert_eq!(prepared.one_shot_backing_bytes(), Some(expected));
        assert!(prepared.empty_backing_bytes().is_none());
        let PreparedForeInputs::Single(entries) = prepared else {
            unreachable!()
        };
        assert_eq!(entries[0].bytes.as_ptr(), payload.as_ptr());
        assert_eq!(payload.len(), 4096);
    }
}
