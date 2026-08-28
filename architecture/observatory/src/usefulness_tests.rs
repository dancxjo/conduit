use super::*;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    BootId, CapabilityId, HostId, ImplementationId, KindId, OfferGeneration, ResourceClassId,
    ResourcePoolId, SignId,
};

fn report() -> CapabilityUsefulnessReport {
    CapabilityUsefulnessReport {
        host_id: HostId::from("host/laptop"),
        boot_id: BootId::from("boot/laptop/7"),
        offer_generation: OfferGeneration(11),
        observed_at_millis: 1_000,
        expires_at_millis: 2_000,
        observation_sign_id: SignId::from("sign/usefulness/11"),
        entries: vec![
            CapabilityUsefulnessEntry {
                subject_id: "profile/local-build".into(),
                label: "local build profile".into(),
                kind_id: None,
                capability_id: None,
                implementation_id: None,
                disposition: CapabilityDisposition::CapacityInsufficient {
                    pool_id: ResourcePoolId::from("storage/main"),
                    required_units: 900,
                    reservable_units: 640,
                },
                resources: vec![ResourceCeiling {
                    pool_id: ResourcePoolId::from("storage/main"),
                    class_id: ResourceClassId::from("base:storage"),
                    capacity_units: 1_024,
                    reservable_units: 640,
                }],
            },
            CapabilityUsefulnessEntry {
                subject_id: "capability/browser".into(),
                label: "browser presentation".into(),
                kind_id: Some(KindId::from("presentation/browser")),
                capability_id: Some(CapabilityId::from("cap/browser")),
                implementation_id: Some(ImplementationId::from("impl/browser")),
                disposition: CapabilityDisposition::Offered,
                resources: Vec::new(),
            },
            CapabilityUsefulnessEntry {
                subject_id: "capability/keyboard".into(),
                label: "keyboard input".into(),
                kind_id: Some(KindId::from("input/keyboard")),
                capability_id: Some(CapabilityId::from("cap/keyboard")),
                implementation_id: Some(ImplementationId::from("impl/keyboard")),
                disposition: CapabilityDisposition::Offered,
                resources: Vec::new(),
            },
        ],
    }
}

#[test]
fn useful_roles_survive_specific_failure_and_render_first() {
    let report = report();
    let lines = report.render_text(1_500).unwrap();
    assert!(lines[0].contains("freshness=CURRENT"));
    assert!(lines[0].contains("BOOT boot/laptop/7 generation=11"));
    assert!(lines[0].contains("sign=sign/usefulness/11"));
    assert!(lines[1].contains("browser presentation AVAILABLE"));
    assert!(lines[2].contains("keyboard input AVAILABLE"));
    assert!(lines[3].contains("local build profile DOES NOT FIT"));
    assert!(lines[3].contains("required=900 reservable=640 short-by=260"));
    assert_eq!(
        report.entries.len(),
        3,
        "presentation cannot suppress truth"
    );
}

#[test]
fn structured_and_human_views_share_exact_refusal_and_provenance() {
    let report = report();
    let decoded = report.clone();
    assert_eq!(decoded, report);
    assert_eq!(
        decoded.entries[0].disposition.refusal_class(),
        Some("capacity-insufficient")
    );
    let text = decoded.render_text(2_001).unwrap().join("\n");
    assert!(text.contains("freshness=STALE"));
    assert!(text.contains("pool=storage/main"));
    assert!(text.contains("class=base:storage ceiling=1024 reservable=640"));
    assert!(!text.contains("obsolete"));
    assert!(!text.contains("legacy"));
}

#[test]
fn all_required_distinctions_have_stable_nonaggregate_classes() {
    let classes = [
        CapabilityDisposition::Offered,
        CapabilityDisposition::Unsupported,
        CapabilityDisposition::ImplementationMissing,
        CapabilityDisposition::ResourceMissing {
            class_id: ResourceClassId::from("base:storage"),
        },
        CapabilityDisposition::BaseMissing {
            kind_id: conduit_core::HostBaseKindId::from("base:network"),
        },
        CapabilityDisposition::CapacityInsufficient {
            pool_id: ResourcePoolId::from("memory/main"),
            required_units: 2,
            reservable_units: 1,
        },
        CapabilityDisposition::PolicyAuthorityRefusal {
            requirement_id: "authority/network".into(),
        },
        CapabilityDisposition::LineUnavailable {
            line_id: "line/remote".into(),
        },
    ];
    assert_eq!(classes.iter().filter(|class| class.is_useful()).count(), 1);
    assert_eq!(
        classes
            .iter()
            .filter_map(CapabilityDisposition::refusal_class)
            .count(),
        7
    );
}

#[test]
fn malformed_unbounded_and_false_capacity_claims_refuse() {
    let mut invalid = report();
    invalid.entries[1].implementation_id = None;
    assert_eq!(
        invalid.validate(),
        Err(CapabilityUsefulnessError::OfferedWithoutExactImplementation)
    );

    let mut invalid = report();
    invalid.entries[0].disposition = CapabilityDisposition::CapacityInsufficient {
        pool_id: ResourcePoolId::from("storage/main"),
        required_units: 640,
        reservable_units: 640,
    };
    assert_eq!(
        invalid.validate(),
        Err(CapabilityUsefulnessError::InvalidCapacityRelation)
    );

    let mut invalid = report();
    invalid.entries = vec![invalid.entries[1].clone(); MAX_USEFULNESS_ENTRIES + 1];
    assert_eq!(
        invalid.validate(),
        Err(CapabilityUsefulnessError::TooManyEntries)
    );

    let mut invalid = report();
    invalid.entries[0].label = "x".repeat(MAX_USEFULNESS_FIELD_BYTES + 1);
    assert_eq!(
        invalid.validate(),
        Err(CapabilityUsefulnessError::RefusalWithoutSubject)
    );
}
