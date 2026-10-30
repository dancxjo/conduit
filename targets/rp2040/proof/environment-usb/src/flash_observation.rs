//! Read-only chip evidence. Neither configured capacity nor JEDEC identity
//! establishes exclusive storage custody, write timing or an execution grant.
use core::{
    fmt::Write,
    sync::atomic::{AtomicU32, Ordering},
};
// Zero represents unavailable evidence; it is not a replacement chip identity.
static JEDEC: AtomicU32 = AtomicU32::new(0);
pub fn record(id: Option<u32>) {
    JEDEC.store(id.unwrap_or(0), Ordering::Relaxed);
}
pub fn append(out: &mut impl Write) -> core::fmt::Result {
    let id = JEDEC.load(Ordering::Relaxed);
    write!(out, ",\"flash_jedec_id\":")?;
    if id == 0 {
        out.write_str("null")?;
    } else {
        write!(out, "{id}")?;
    }
    write!(
        out,
        ",\"configured_flash_bytes\":{},\"flash_write_timing_qualified\":false",
        conduit_rp2040_flash_layout::FLASH_BYTES
    )
}
