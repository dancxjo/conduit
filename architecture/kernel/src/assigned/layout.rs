//! Exact trusted layout comparison after the shared complete Core decoder.
use super::*;
// Preserve the original 256-byte preparation workspace. Each canonical tag has
// at most255 records; eight tag-local bitmaps fit in one pass, and two passes
// cover all13 tags without retaining a second inventory or allocating storage.
const SEEN_BYTES: usize = 256;
const TAG_BYTES: usize = (u8::MAX as usize).div_ceil(8);
const TAGS_PER_PASS: usize = SEEN_BYTES / TAG_BYTES;
#[cfg(test)]
#[path = "layout_bitmap_tests.rs"]
mod tests;
fn ignored(tag: u8) -> bool {
    matches!(
        tag,
        c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
    )
}
pub(super) fn validate(
    input: RecordInput<'_>,
    layout: &ResidentLayout<'_>,
    fore: Option<ForeProfile<'_>>,
) -> Result<(), AssignedKernelRefusal> {
    use AssignedKernelRefusal as E;
    // The separately trusted layout must remain within the SAME canonical
    // per-tag count domain. Ignored or foreign requirements cannot disappear
    // merely because actual configuration/Sign/terminal records are skipped.
    for tag in 1..=c::ASSIGNED_PLAN_COUNT_KINDS as u8 {
        if layout.records.iter().filter(|r| r.tag == tag).count() > u8::MAX as usize {
            return Err(E::Capacity);
        }
    }
    if layout
        .records
        .iter()
        .any(|r| r.tag == 0 || usize::from(r.tag) > c::ASSIGNED_PLAN_COUNT_KINDS || ignored(r.tag))
    {
        return Err(E::Layout);
    }
    let mut seen = [0u8; SEEN_BYTES];
    for group in 0..c::ASSIGNED_PLAN_COUNT_KINDS.div_ceil(TAGS_PER_PASS) {
        seen.fill(0);
        for record in input.records()? {
            let record = record?;
            if ignored(record.tag) {
                continue;
            }
            if record.tag == 0 || usize::from(record.tag) > c::ASSIGNED_PLAN_COUNT_KINDS {
                return Err(E::Layout);
            }
            let tag = usize::from(record.tag - 1);
            if tag / TAGS_PER_PASS != group {
                continue;
            }
            // Hash only in the record's one selected group. The existing
            // Fore/native normalization and full digest comparison are intact.
            let expected = match fore {
                Some(ForeProfile::Line(profile)) => {
                    c::AssignedRecordRequirement::from_kernel_record_with_line_fore(record, profile)
                }
                _ => c::AssignedRecordRequirement::from_kernel_record(
                    record,
                    u16_at(input.header(), 8)?,
                ),
            };
            let start = (tag % TAGS_PER_PASS) * TAG_BYTES;
            let ordinal = layout
                .records
                .iter()
                .filter(|r| r.tag == record.tag)
                .enumerate()
                .position(|(i, r)| seen[start + i / 8] & (1 << (i % 8)) == 0 && *r == expected)
                .ok_or(E::Layout)?;
            seen[start + ordinal / 8] |= 1 << (ordinal % 8);
        }
        for tag in (group * TAGS_PER_PASS + 1)
            ..=((group + 1) * TAGS_PER_PASS).min(c::ASSIGNED_PLAN_COUNT_KINDS)
        {
            let count = layout
                .records
                .iter()
                .filter(|r| usize::from(r.tag) == tag)
                .count();
            let start = ((tag - 1) % TAGS_PER_PASS) * TAG_BYTES;
            if (0..count).any(|i| seen[start + i / 8] & (1 << (i % 8)) == 0) {
                return Err(E::Layout);
            }
        }
    }
    Ok(())
}
