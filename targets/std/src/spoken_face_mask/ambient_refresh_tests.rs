//! Passive observations must not occupy the speech channel (#6916).
use super::*;

#[test]
fn repeated_accepted_updates_are_silent_and_explicit_reading_uses_current_truth() {
    let (mut face, mut show) = face_with_action();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    for revision in 2..12 {
        face.revision = revision;
        face.subjects
            .iter_mut()
            .find(|s| s.identity == "arrival")
            .unwrap()
            .name = format!("Current observation {revision}");
        // Reconstruct to obtain the identity of this exact accepted revision.
        face = Face::new_with_semantics(
            face.revision,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            face.actions,
            face.disclosures,
        )
        .unwrap();
        show = common::available_mask_show(&face);
        reader.refresh(face.clone(), show.clone()).unwrap();
        assert!(reader.next_segment().unwrap().is_none());
    }
    reader
        .command(
            &face,
            &show,
            ReaderCommand::FocusSubject("arrival".into()),
            1,
        )
        .unwrap();
    let packet = reader.next_segment().unwrap().unwrap();
    assert!(packet.segment.text.contains("Current observation 11"));
    reader
        .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
        .unwrap();
}

#[test]
fn passive_refresh_preserves_requested_reading_and_exact_delivery_pressure() {
    let (face, show) = face_with_action();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    // Even before the first packet, a requested turn must not be discarded.
    assert_eq!(
        reader.refresh(face.clone(), show.clone()),
        Err(SpokenFaceRefusal::SpeechPressure)
    );
    let packet = reader.next_segment().unwrap().unwrap();
    assert_eq!(
        reader.refresh(face.clone(), show.clone()),
        Err(SpokenFaceRefusal::SpeechPressure)
    );
    reader
        .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
        .unwrap();
    reader
        .command(&face, &show, ReaderCommand::Stop, 2)
        .unwrap();
    reader.refresh(face, show).unwrap();
    assert!(reader.next_segment().unwrap().is_none());
}
