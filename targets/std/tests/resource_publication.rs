mod resource_common;
use conduit_core::ResourceSharing;
use conduit_std_host::hosted_resource::ResourcePublicationRefusal as Refusal;
use resource_common::*;

#[test]
fn published_frame_is_one_residency_for_three_consumers_and_cannot_be_rewritten() {
    let bytes = vec![42; FRAME_BYTES];
    let mut frame = prepared(ResourceSharing::SingleWriterPublished, 2);
    assert_eq!(
        frame.write_candidate(&reader(), &bytes),
        Err(Refusal::AuthorityDenied)
    );
    assert_eq!(frame.acquire(&reader()), Err(Refusal::NotPublished));
    frame.write_candidate(&writer(), &bytes).unwrap();
    assert_eq!(
        frame.write_candidate(&writer(), &bytes),
        Err(Refusal::CandidateOccupied)
    );
    assert_eq!(frame.acquire(&reader()), Err(Refusal::NotPublished));
    frame.publish(&writer()).unwrap();
    let leases = [
        frame.acquire(&reader()).unwrap(),
        frame.acquire(&reader()).unwrap(),
        frame.acquire(&reader()).unwrap(),
    ];
    assert_eq!(frame.acquire(&reader()), Err(Refusal::LeaseExhausted));
    assert_eq!(frame.payload_residencies(), 1);
    assert_eq!(frame.resident_bytes(), FRAME_BYTES as u32);
    for lease in leases {
        assert_eq!(frame.read(lease).unwrap(), bytes);
        assert!(std::ptr::eq(
            frame.read(lease).unwrap().as_ptr(),
            frame.read(leases[0]).unwrap().as_ptr()
        ));
    }
    assert_eq!(
        frame.write_candidate(&writer(), &bytes),
        Err(Refusal::PublishedImmutable)
    );
    assert_eq!(frame.retire(&writer()), Err(Refusal::ReadersPresent));
    for lease in leases {
        frame.release(lease).unwrap();
        assert_eq!(frame.release(lease), Err(Refusal::StaleLease));
    }
    let replacement = frame.acquire(&reader()).unwrap();
    assert_eq!(frame.read(leases[0]), Err(Refusal::StaleLease));
    frame.release(replacement).unwrap();
    frame.retire(&writer()).unwrap();
    assert_eq!(frame.resident_bytes(), 0);
    assert_eq!(frame.acquire(&reader()), Err(Refusal::Lost));
    assert_eq!(frame.write_candidate(&writer(), &bytes), Err(Refusal::Lost));
}

#[test]
fn compositor_can_publish_next_exact_generation_while_readers_keep_old_content() {
    let input = vec![40; FRAME_BYTES];
    let mut previous = prepared(ResourceSharing::ImmutableReadMany, 2)
        .initialize(&input)
        .unwrap();
    let old = previous.acquire(&reader()).unwrap();
    let mut output = vec![0; FRAME_BYTES];
    for (pixel, result) in previous.read(old).unwrap().iter().zip(&mut output) {
        *result = pixel + 2;
    }
    let mut next = prepared(ResourceSharing::SingleWriterPublished, 3);
    next.write_candidate(&writer(), &output).unwrap();
    next.publish(&writer()).unwrap();
    let display = next.acquire(&reader()).unwrap();
    let encoder = next.acquire(&reader()).unwrap();
    assert_ne!(
        previous.reference().lifetime.version,
        next.reference().lifetime.version
    );
    assert_eq!(next.read(old), Err(Refusal::StaleLease));
    assert_eq!(previous.read(old).unwrap(), input);
    assert_eq!(next.read(display).unwrap(), output);
    assert_eq!(next.read(encoder).unwrap(), output);
    assert_eq!(next.payload_residencies(), 1);
    next.release(display).unwrap();
    next.release(encoder).unwrap();
    previous.release(old).unwrap();
    next.retire(&writer()).unwrap();
}

#[test]
fn candidate_cancellation_releases_payload_and_bad_extent_does_not_consume_storage() {
    let mut frame = prepared(ResourceSharing::SingleWriterPublished, 2);
    assert_eq!(
        frame.write_candidate(&writer(), &[0]),
        Err(Refusal::ReferenceRefused)
    );
    assert_eq!(frame.resident_bytes(), 0);
    frame
        .write_candidate(&writer(), &vec![0; FRAME_BYTES])
        .unwrap();
    frame.retire(&writer()).unwrap();
    assert_eq!(frame.payload_residencies(), 0);
    assert_eq!(frame.publish(&writer()), Err(Refusal::Lost));
}
