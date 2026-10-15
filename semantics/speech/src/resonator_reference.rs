//! Frozen pre-extraction scalar reference for whole-frame voice parity.
//! This intentionally does not call the generated/shared recurrence under test.
//! Ordering and I32 overflow match the equation accepted at 560916ec4f36.
pub(super) fn filter_limit(drive: i32, b: i32, c: i32, y1: i32, y2: i32) -> Option<i32> {
    let first = drive.checked_add(b.checked_mul(y1)?)?;
    let sample = first.checked_add(c.checked_mul(y2)?)?.checked_div(16384)?;
    Some(sample.clamp(-32767, 32767))
}
