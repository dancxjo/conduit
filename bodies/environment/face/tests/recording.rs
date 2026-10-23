#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_environment_face::{
    PreparedRecordingFace, RecordingFaceRefusal as E, RecordingRecoveryFailure as F,
    RecoveredRecordingRange as Range,
};
use conduit_face::{FaceBasis, FacePropertyValue as V};
fn basis() -> FaceBasis {
    FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some("source/recording".into()),
        checked_plot_id: Some("checked/recording".into()),
        expanded_plot_id: Some("expanded/recording".into()),
        plan_id: Some("plan/recording".into()),
        active_play_id: Some("play/recording".into()),
        sign_ids: vec![],
    }
}
fn range() -> Range {
    Range {
        chunks: 1,
        samples: 2,
        first_sequence: 1,
        last_sequence: 3,
    }
}
#[test]
fn recovered_ranges_and_refusals_publish_without_allocating_or_claiming_durability() {
    let mut face = PreparedRecordingFace::prepare(basis(), 2, 128, 4).unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        face.current().validate().unwrap();
        face.publish_recovered(range()).unwrap();
        assert_eq!(face.current().revision, 1);
        face.publish_recovered(range()).unwrap();
        assert_eq!(face.current().revision, 1);
        face.publish_refusal(F::InvalidContent).unwrap();
        assert_eq!(face.current().revision, 2);
        face.publish_refusal(F::InvalidContent).unwrap();
        assert_eq!(face.current().revision, 2);
        let next = Range {
            chunks: 2,
            samples: 3,
            first_sequence: 1,
            last_sequence: 7,
        };
        face.publish_recovered(next).unwrap();
        face.publish_refusal(F::Cancelled).unwrap();
        face.current().validate().unwrap();
        assert!(face
            .current()
            .properties
            .iter()
            .any(|p| p.name == "missing-sequences" && p.value == V::Count(4)));
        assert!(face
            .current()
            .properties
            .iter()
            .any(|p| p.name == "samples" && p.value == V::Count(3)));
        assert!(face
            .current()
            .properties
            .iter()
            .any(|p| p.name == "last-sequence" && p.value == V::Count(7)));
        assert!(face
            .current()
            .properties
            .iter()
            .all(|p| !matches!(p.name.as_str(), "durable" | "fresh" | "temperature")));
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn invalid_reversed_and_exhausted_updates_preserve_the_prior_face() {
    assert!(matches!(
        PreparedRecordingFace::prepare(basis(), 0, 128, 1),
        Err(E::Configuration)
    ));
    let mut face = PreparedRecordingFace::prepare(basis(), 2, 128, 1).unwrap();
    face.publish_recovered(range()).unwrap();
    let previous = face.current().clone();
    for invalid in [
        Range {
            chunks: 0,
            ..range()
        },
        Range {
            samples: 4,
            ..range()
        },
        Range {
            first_sequence: 4,
            ..range()
        },
    ] {
        assert_eq!(face.publish_recovered(invalid), Err(E::Range));
        assert_eq!(face.current(), &previous);
    }
    for reversed in [
        Range {
            samples: 1,
            ..range()
        },
        Range {
            first_sequence: 2,
            ..range()
        },
        Range {
            last_sequence: 2,
            ..range()
        },
    ] {
        assert_eq!(face.publish_recovered(reversed), Err(E::Reversed));
        assert_eq!(face.current(), &previous);
    }
    assert_eq!(face.publish_refusal(F::Capacity), Err(E::Exhausted));
    assert_eq!(face.current(), &previous);
    face.publish_recovered(range()).unwrap();
    assert_eq!(face.current(), &previous);
}
