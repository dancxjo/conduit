#![cfg(feature = "face-projection")]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedSourceFrameDecoder},
    publication::{PublicationChannel as Channel, PublicationSnapshot},
    publication_face_custody::{CustodyRefusal as E, LocalPublicationFace},
    retained_publication_face::RetainedPublicationFace,
};
use conduit_face::{FaceBasis, FacePropertyValue};
fn owner() -> LocalPublicationFace {
    LocalPublicationFace::new(
        RetainedPublicationFace::prepare(
            FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: Some("source/CPU-fixture".into()),
                checked_plot_id: Some("checked/CPU-fixture".into()),
                expanded_plot_id: Some("expanded/CPU-fixture".into()),
                plan_id: Some("plan/CPU-fixture".into()),
                active_play_id: Some("play/CPU-fixture".into()),
                sign_ids: vec![],
            },
            3,
            100,
        )
        .unwrap(),
    )
}
fn reading() -> Vec<u8> {
    PreparedSourceFrameDecoder::prepare(&ObservationContract::prepare().unwrap())
        .unwrap()
        .decode(5, [53, 0, 20, 2, 75])
        .unwrap()
        .to_vec()
}
fn snapshot(bytes: &[u8], sequence: u64) -> PublicationSnapshot<'_> {
    PublicationSnapshot {
        channel: Channel::Reading,
        sequence,
        canonical: bytes,
    }
}
#[test]
fn one_producer_and_reader_preserve_quantity_revisions_freshness_and_terminal_custody() {
    let owner = owner();
    let bytes = reading();
    let (mut publisher, reader) = owner.claim().unwrap();
    assert!(matches!(owner.claim(), Err(E::AlreadyClaimed)));
    let ((), memory) = allocation_probe::observe(|| {
        publisher.publish(snapshot(&bytes, 1), 1000).unwrap();
        reader
            .with_current(1001, |face, status| {
                assert_eq!(
                    (
                        face.revision,
                        status.source_sequence,
                        status.publisher_retained
                    ),
                    (1, 1, true)
                );
                assert_eq!(status.refusal, None);
                for name in ["temperature", "relative-humidity"] {
                    assert!(face.properties.iter().any(|p| p.name == name
                        && matches!(p.value, FacePropertyValue::TypedValue { .. })));
                }
            })
            .unwrap();
        reader
            .with_current(1101, |face, status| {
                assert_eq!((face.revision, status.source_sequence), (2, 1));
                assert!(face
                    .properties
                    .iter()
                    .any(|p| p.name == "fresh" && p.value == FacePropertyValue::Flag(false)));
            })
            .unwrap();
        publisher.publish(snapshot(&bytes, 2), 1200).unwrap();
        drop(publisher);
        reader
            .with_current(1201, |face, status| {
                assert_eq!(
                    (
                        face.revision,
                        status.source_sequence,
                        status.publisher_retained
                    ),
                    (3, 2, false)
                );
                assert_eq!(status.refusal, None);
            })
            .unwrap();
        assert!(matches!(owner.claim(), Err(E::AlreadyClaimed)));
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn reentrant_pressure_and_rejected_clock_or_publication_preserve_committed_values() {
    let owner = owner();
    let bytes = reading();
    let (mut publisher, reader) = owner.claim().unwrap();
    reader
        .with_current(0, |face, status| {
            assert_eq!((face.revision, status.source_sequence), (0, 0));
            assert!(matches!(reader.with_current(0, |_, _| ()), Err(E::Busy)));
            assert_eq!(publisher.publish(snapshot(&bytes, 1), 1000), Err(E::Busy));
            assert_eq!(face.revision, 0);
        })
        .unwrap();
    reader
        .with_current(0, |_, status| assert_eq!(status.refusal, Some("face-busy")))
        .unwrap();
    publisher.publish(snapshot(&bytes, 1), 1000).unwrap();
    let before = reader.with_current(1000, |face, _| face.clone()).unwrap();
    assert!(matches!(
        publisher.publish(snapshot(&[], 2), 1001),
        Err(E::Publication(_))
    ));
    reader
        .with_current(1001, |face, status| {
            assert_eq!(face, &before);
            assert_eq!(status.refusal, Some("structured"));
        })
        .unwrap();
    assert!(matches!(
        reader.with_current(999, |_, _| panic!("refused consumer ran")),
        Err(E::Clock(_))
    ));
    reader
        .with_current(1001, |face, status| {
            assert_eq!(face, &before);
            assert_eq!(status.refusal, Some("clock-reversed"));
        })
        .unwrap();
    publisher.publish(snapshot(&bytes, 2), 1002).unwrap();
    reader
        .with_current(1003, |face, status| {
            assert_eq!(face.revision, 2);
            assert_eq!(status.refusal, None);
        })
        .unwrap();
}
#[test]
fn dropping_a_publisher_while_reading_never_leaves_false_retained_custody() {
    let owner = owner();
    let (publisher, reader) = owner.claim().unwrap();
    reader
        .with_current(0, |face, status| {
            assert!(status.publisher_retained);
            drop(publisher);
            assert_eq!(face.revision, 0);
        })
        .unwrap();
    reader
        .with_current(0, |_, status| assert!(!status.publisher_retained))
        .unwrap();
    assert!(matches!(owner.claim(), Err(E::AlreadyClaimed)));
}
#[cfg(feature = "display-mask")]
#[test]
fn reader_can_compose_owned_mask_frames_without_allocations_or_escaping_face_borrows() {
    use conduit_environment_runtime::display_mask::PreparedDisplayMask;
    use conduit_seven_segment_mask::*;
    let owner = owner();
    let bytes = reading();
    let (mut publisher, reader) = owner.claim().unwrap();
    let mut mask = PreparedDisplayMask::prepare(
        Tm1637Profile {
            digits: core::array::from_fn(|n| DigitWiring {
                address: n as u8,
                segment_bits: [0, 1, 2, 3, 4, 5, 6],
            }),
            decimal_points: [None; 4],
            colon: Some(WireBit { address: 1, bit: 7 }),
        },
        SevenSegmentLook::fahrenheit().with_colon_fraction(),
        3000,
        6000,
        3,
    )
    .unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        publisher.publish(snapshot(&bytes, 1), 1000).unwrap();
        for elapsed in [0, 3000] {
            let (owned, length) = reader
                .with_current(1001, |face, status| {
                    assert!(status.publisher_retained);
                    let frame = mask.render(face, elapsed).unwrap();
                    assert_eq!(frame.rendered.face_content_id, face.identity.as_str());
                    let mut owned = [0; 1024];
                    owned[..frame.canonical.len()].copy_from_slice(frame.canonical);
                    (owned, frame.canonical.len())
                })
                .unwrap();
            conduit_environment_runtime::display_packets::validate_display_frame(&owned[..length])
                .unwrap();
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
