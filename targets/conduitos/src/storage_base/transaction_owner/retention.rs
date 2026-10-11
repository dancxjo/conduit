//! Original preparation allocations, charged before storage enables USB.
use super::{StorageScopeAdmission, StorageTransactionRefusal};

pub(super) fn metadata(
    admission: &StorageScopeAdmission<'_>,
) -> Result<usize, StorageTransactionRefusal> {
    let table = &admission.table;
    if table.maximum_capabilities() != 1 || table.reserved_capability_slots() != 1 {
        return Err(StorageTransactionRefusal::Retention);
    }
    let bytes = table
        .retained_storage_bytes(32)
        .and_then(|bytes| bytes.checked_add(admission.claim.retained_storage_bytes(32)?))
        .ok_or(StorageTransactionRefusal::Retention)?;
    if bytes > 16384 {
        return Err(StorageTransactionRefusal::Retention);
    }
    Ok(bytes)
}

pub(crate) fn charge(
    owner: usize,
    inline: usize,
    body_capacity: usize,
    admitted: u64,
) -> Result<usize, StorageTransactionRefusal> {
    let body = body_capacity
        .checked_add(31)
        .map(|bytes| bytes / 32 * 32)
        .ok_or(StorageTransactionRefusal::Retention)?;
    let retained = owner
        .checked_add(inline)
        .and_then(|bytes| bytes.checked_add(body))
        .ok_or(StorageTransactionRefusal::Retention)?;
    if crate::allocation::BOOT_ARENA.capacity() != 0 {
        use core::fmt::Write;
        let mut line = crate::sign_format::FixedText::new();
        writeln!(
            &mut line,
            "CONDUIT_STORAGE_PREPARE owner={} inline={} body={} retained={} admitted={}",
            owner, inline, body, retained, admitted,
        )
        .map_err(|_| StorageTransactionRefusal::Retention)?;
        crate::arch::early_write(line.as_bytes());
    }
    if retained as u64 > admitted {
        return Err(StorageTransactionRefusal::Retention);
    }
    Ok(retained)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_output_and_body_allocation_are_charged_before_admission() {
        assert_eq!(charge(16384, 4096, 33, 20544), Ok(20544));
        assert_eq!(
            charge(16384, 4096, 33, 20543),
            Err(StorageTransactionRefusal::Retention)
        );
        assert_eq!(
            charge(usize::MAX, 1, 0, u64::MAX),
            Err(StorageTransactionRefusal::Retention)
        );
        assert_eq!(
            charge(0, 0, usize::MAX, u64::MAX),
            Err(StorageTransactionRefusal::Retention)
        );
        assert_eq!(
            charge(1, 1, 1, 0),
            Err(StorageTransactionRefusal::Retention)
        );
    }
}
