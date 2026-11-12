//! Exact trusted layout comparison after the shared complete Core decoder.
use super::*;
pub(super) fn validate(
    input: RecordInput<'_>,
    layout: &ResidentLayout<'_>,
    fore: Option<ForeProfile<'_>>,
) -> Result<(), AssignedKernelRefusal> {
    use AssignedKernelRefusal as E;
    if layout.records.len() > 256 {
        return Err(E::Capacity);
    }
    let mut seen = [false; 256];
    for record in input.records()? {
        let record = record?;
        if matches!(
            record.tag,
            c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
        ) {
            continue;
        }
        let expected = match fore {
            Some(ForeProfile::Line(profile)) => {
                c::AssignedRecordRequirement::from_kernel_record_with_line_fore(record, profile)
            }
            _ => {
                c::AssignedRecordRequirement::from_kernel_record(record, u16_at(input.header(), 8)?)
            }
        };
        let index = layout
            .records
            .iter()
            .enumerate()
            .position(|(i, r)| !seen[i] && *r == expected)
            .ok_or(E::Layout)?;
        seen[index] = true;
    }
    if seen[..layout.records.len()].contains(&false) {
        return Err(E::Layout);
    }
    Ok(())
}
