use super::*;

fn long_list() -> Presentation {
    let (base, _) = face_with_action();
    let mut subjects = base.subjects;
    let mut disclosures = base.disclosures;
    for index in 0..8 {
        let identity = format!("item/{index}");
        subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Item,
            name: format!("Page item {index}"),
        });
        disclosures.push(PresentationDisclosure {
            subject: identity,
            level: PresentationDisclosureLevel::Primary,
        });
    }
    Presentation::new_with_semantics(
        base.revision,
        base.basis,
        subjects,
        base.relationships,
        base.properties,
        base.text,
        base.actions,
        disclosures,
    )
    .unwrap()
}

#[test]
fn explicit_pages_keep_exact_show_and_resume_without_duplicates() {
    let face = long_list();
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    assert_eq!(
        reader.command(&face, &show, ReaderCommand::MoreItems, 1),
        Err(SpokenFaceRefusal::InvalidValue)
    );
    for (sequence, command, first, last) in [
        (2, ReaderCommand::ReadItemPage, 0, 2),
        (3, ReaderCommand::MoreItems, 3, 5),
        (4, ReaderCommand::MoreItems, 6, 7),
    ] {
        reader.command(&face, &show, command, sequence).unwrap();
        let readout = reader.take_text_readout().unwrap().unwrap();
        assert_eq!(readout.face_id, face.identity.as_str());
        assert_eq!(readout.show_id, show.show_id.as_str());
        let text = readout.clauses.join(" ");
        for index in 0..8 {
            assert_eq!(
                text.contains(&format!("Page item {index},")),
                (first..=last).contains(&index)
            );
        }
        assert_eq!(text.contains("Type more items"), last < 7);
    }
    reader
        .command(&face, &show, ReaderCommand::MoreItems, 5)
        .unwrap();
    assert_eq!(
        reader.take_text_readout().unwrap().unwrap().clauses,
        ["No more current items."]
    );
}

#[test]
fn refresh_refuses_an_old_page_until_a_new_selection_is_requested() {
    let face = long_list();
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadItemPage, 1)
        .unwrap();
    reader.take_text_readout().unwrap();
    let next = Presentation::new_with_semantics(
        face.revision + 1,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
        face.actions.clone(),
        face.disclosures.clone(),
    )
    .unwrap();
    let next_show = common::available_mask_show(&next);
    reader.refresh(next.clone(), next_show.clone()).unwrap();
    assert_eq!(
        reader.command(&next, &next_show, ReaderCommand::MoreItems, 2),
        Err(SpokenFaceRefusal::StaleFace)
    );
    reader
        .command(&next, &next_show, ReaderCommand::ReadItemPage, 3)
        .unwrap();
    assert!(reader
        .take_text_readout()
        .unwrap()
        .unwrap()
        .clauses
        .join(" ")
        .contains("Page item 0,"));
}

#[test]
fn pending_audio_refuses_page_advancement() {
    let face = long_list();
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadItemPage, 1)
        .unwrap();
    let packet = reader.next_segment().unwrap().unwrap();
    assert!(!packet.segment.text.is_empty());
    let cursor = reader.item_cursor.clone();
    assert_eq!(
        reader.command(&face, &show, ReaderCommand::MoreItems, 2),
        Err(SpokenFaceRefusal::SpeechPressure)
    );
    assert_eq!(reader.item_cursor, cursor);
}

#[test]
fn unread_page_cannot_be_skipped_by_a_more_request() {
    let face = long_list();
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadItemPage, 1)
        .unwrap();
    let cursor = reader.item_cursor.clone();
    assert_eq!(
        reader.accepts_interruption(&face, &show, &ReaderCommand::MoreItems, 2),
        Err(SpokenFaceRefusal::SpeechPressure)
    );
    assert_eq!(
        reader.command(&face, &show, ReaderCommand::MoreItems, 2),
        Err(SpokenFaceRefusal::SpeechPressure)
    );
    assert_eq!(reader.item_cursor, cursor);
    let readout = reader.take_text_readout().unwrap().unwrap();
    assert!(readout.clauses.join(" ").contains("Page item 0,"));
}
