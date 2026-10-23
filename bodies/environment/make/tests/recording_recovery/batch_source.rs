//! Checked batch meaning only. No offered implementation or storage is invented.
use super::*;
use conduit_environment_make::RecordingBatchRecoverySource;
fn references(count: u8) -> Vec<c::BoundedResourceRef> {
    (1..=count)
        .map(|version| {
            let mut reference = reference();
            reference.lifetime.version = c::ResourceVersionIdentity::from_digest([version; 32]);
            reference
        })
        .collect()
}
#[test]
fn overnight_batch_has_exact_reads_and_ordered_finite_concat_in_checked_source() {
    let references = references(15);
    let source = RecordingBatchRecoverySource::compile(&references).unwrap();
    let expanded = source.expanded();
    assert_eq!(expanded.gears.len(), 60);
    assert_eq!(expanded.connections.len(), 59);
    for (kind, count) in [
        ("environment/recording-generation", 15),
        ("resource/read-json-snapshot", 15),
        ("flow/singleton", 15),
        ("flow/concat/finite", 14),
        ("environment/recover-recording", 1),
    ] {
        assert_eq!(
            expanded
                .gears
                .iter()
                .filter(|g| g.kind_id.as_str() == kind)
                .count(),
            count
        );
    }
    for reference in source.references() {
        let hex: String = reference
            .encode()
            .unwrap()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            expanded
                .gears
                .iter()
                .filter(|g| g.kind_id.as_str() == "resource/read-json-snapshot"
                    && g.configuration.iter().any(|entry| entry.key == "reference"
                        && entry.value == c::ConfigurationValue::Text(hex.clone())))
                .count(),
            1
        );
    }
    let same = RecordingBatchRecoverySource::compile(&references).unwrap();
    assert_eq!(expanded, same.expanded());
    let mut reversed = references.clone();
    reversed.reverse();
    assert_ne!(
        expanded,
        RecordingBatchRecoverySource::compile(&reversed)
            .unwrap()
            .expanded()
    );
    let singleton = RecordingBatchRecoverySource::compile(&references[..1]).unwrap();
    assert_eq!(singleton.expanded().gears.len(), 4);
    assert_eq!(singleton.expanded().connections.len(), 3);
    // The complete declared 16-generation profile remains inside the ordinary
    // checker bound, rather than being silently truncated to the tested night.
    assert_eq!(
        RecordingBatchRecoverySource::compile(&self::references(16))
            .unwrap()
            .expanded()
            .gears
            .len(),
        64
    );
}
#[test]
fn empty_duplicate_foreign_and_over_capacity_batch_meaning_refuses() {
    assert!(RecordingBatchRecoverySource::compile(&[]).is_err());
    let mut duplicate = references(2);
    duplicate[1] = duplicate[0].clone();
    assert!(RecordingBatchRecoverySource::compile(&duplicate).is_err());
    let mut foreign = references(1);
    foreign[0].content_profile = "foreign".into();
    assert!(RecordingBatchRecoverySource::compile(&foreign).is_err());
    assert!(RecordingBatchRecoverySource::compile(&references(17)).is_err());
}
