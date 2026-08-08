use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static BOOT_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(super) fn fresh_boot_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = BOOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("boot-{now:x}-{counter:x}")
}
