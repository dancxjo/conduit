//! Address-owned native capacity. Shared session/execution storage is charged once.
use super::profile::{NativeBackingGrant, NativeBackingRefusal};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackingRole {
    SharedNativePool,
    FixedIo,
    ExecutionStack,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackingSpan {
    pub start: u64,
    pub end: u64,
    pub role: BackingRole,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedPoolUse {
    /// Both roles refer to the same already charged pool; not two reservations.
    pub pool_identity: [u8; 32],
    pub capacity_bytes: u64,
    pub persistent_session_and_metadata: bool,
    pub execution_scratch_and_metadata: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackingLedger {
    pub profile: String,
    pub private_bytes: u64,
    /// Disjoint image address reservations, including potential relocated private pages.
    pub image_mapping_bytes: u64,
    /// Deduplicated source file extent inventory; already covered by image mappings.
    pub immutable_file_bytes: u64,
    pub shared_pool: SharedPoolUse,
    pub fixed_io_bytes: u64,
    pub execution_stack_bytes: u64,
    pub fixed_io_span: BackingSpan,
    pub execution_stack_span: BackingSpan,
    pub private_mapping_identity: [u8; 32],
    pub immutable_mapping_identity: [u8; 32],
    pub fixed_mapping_identity: [u8; 32],
    /// Read-only kernel/vDSO/guard mappings, separate from native writable backing.
    pub other_mapping_bytes: u64,
}
impl BackingLedger {
    pub fn validate(&self, grant: NativeBackingGrant) -> Result<(), NativeBackingRefusal> {
        grant.validate()?;
        let (_, io_bytes, stack_bytes) =
            charge_private_spans(&[self.fixed_io_span, self.execution_stack_span])?;
        if self.fixed_io_span.role != BackingRole::FixedIo
            || self.execution_stack_span.role != BackingRole::ExecutionStack
            || io_bytes != self.fixed_io_bytes
            || stack_bytes != self.execution_stack_bytes
            || io_bytes != 4096
            || stack_bytes != super::profile::STACK_BYTES as u64
        {
            return Err(NativeBackingRefusal::InvalidLedger);
        }

        if self.profile != super::profile::PROFILE
            || self.private_bytes == 0
            || self.shared_pool.capacity_bytes == 0
            || !self.shared_pool.persistent_session_and_metadata
            || !self.shared_pool.execution_scratch_and_metadata
            || self.shared_pool.pool_identity == [0; 32]
            || self.private_mapping_identity == [0; 32]
            || self
                .shared_pool
                .capacity_bytes
                .checked_add(self.fixed_io_bytes)
                .and_then(|v| v.checked_add(self.execution_stack_bytes))
                != Some(self.private_bytes)
        {
            return Err(NativeBackingRefusal::InvalidLedger);
        }
        if self.private_bytes > grant.maximum_private_bytes
            || self
                .image_mapping_bytes
                .checked_add(self.other_mapping_bytes)
                .ok_or(NativeBackingRefusal::InvalidLedger)?
                > grant.maximum_fixed_mapping_bytes
        {
            return Err(NativeBackingRefusal::InsufficientBacking {
                private_required: self.private_bytes,
                fixed_mapping_required: self
                    .image_mapping_bytes
                    .checked_add(self.other_mapping_bytes)
                    .ok_or(NativeBackingRefusal::InvalidLedger)?,
            });
        }
        Ok(())
    }
}
/// Exact disjoint span accounting; no measured-per-run or RSS estimate enters this sum.
pub fn charge_private_spans(
    spans: &[BackingSpan],
) -> Result<(u64, u64, u64), NativeBackingRefusal> {
    let mut total = (0u64, 0u64, 0u64);
    for (index, span) in spans.iter().enumerate() {
        if span.start % 4096 != 0
            || span.end % 4096 != 0
            || span.start >= span.end
            || spans[..index]
                .iter()
                .any(|p| p.start < span.end && span.start < p.end)
        {
            return Err(NativeBackingRefusal::InvalidLedger);
        }
        let slot = match span.role {
            BackingRole::SharedNativePool => &mut total.0,
            BackingRole::FixedIo => &mut total.1,
            BackingRole::ExecutionStack => &mut total.2,
        };
        *slot = slot
            .checked_add(span.end - span.start)
            .ok_or(NativeBackingRefusal::InvalidLedger)?;
    }
    Ok(total)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_pool_is_charged_once_and_overlapping_domains_refuse() {
        let spans = [
            BackingSpan {
                start: 4096,
                end: 8192,
                role: BackingRole::SharedNativePool,
            },
            BackingSpan {
                start: 8192,
                end: 12288,
                role: BackingRole::FixedIo,
            },
        ];
        assert_eq!(charge_private_spans(&spans), Ok((4096, 4096, 0)));
        let mut bad = spans;
        bad[1].start = 4096;
        assert_eq!(
            charge_private_spans(&bad),
            Err(NativeBackingRefusal::InvalidLedger)
        );
    }
}
