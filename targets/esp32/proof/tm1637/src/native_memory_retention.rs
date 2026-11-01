//! Allocation-free storage observations. Counts describe retained inline
//! backing only; they grant no capacity and expose no admission contents.
use conduit_body::{SealedBodyLifecycleSession, VecStorageUsage};

fn row(storage: VecStorageUsage) -> [usize; 3] {
    [storage.len, storage.capacity, storage.element_bytes]
}

pub(super) fn report(body: Option<&SealedBodyLifecycleSession>) {
    #[derive(serde::Serialize)]
    struct Report {
        event: &'static str,
        // Rows: pending, invitations, continuity keys, pending returns, receipts.
        // Each row is [length, capacity, inline element bytes].
        admissions: Option<[[usize; 3]; 5]>,
        pending_archives: Option<[usize; 3]>,
        // The exact completed signed response's [length, capacity].
        join_response: Option<[usize; 2]>,
        // Cached authenticated [journal generation, encoded payload bytes].
        // None while the actual Store is leased, stale or unavailable.
        checkpoint: Option<(u64, usize)>,
    }
    let admissions = crate::native_restart::with_admissions(|manager| {
        let usage = manager.storage_usage();
        Ok([
            row(usage.pending_admissions),
            row(usage.invitations),
            row(usage.continuity_keys),
            row(usage.pending_returns),
            row(usage.receipts),
        ])
    })
    .ok();
    let report = Report {
        event: "root-retained-storage",
        admissions,
        pending_archives: body.map(|owner| row(owner.pending_archive_storage_usage())),
        join_response: crate::native_body_join_http::completed_storage_usage(),
        checkpoint: crate::native_restart::checkpoint_extent().ok(),
    };
    let mut bytes = [0; 512];
    if let Ok(length) = serde_json_core::to_slice(&report, &mut bytes) {
        if let Ok(text) = core::str::from_utf8(&bytes[..length]) {
            esp_println::println!("{}", text);
        }
    }
}
