use super::*;

fn edit(base_revision: u64, start: u32, end: u32, text: &str) -> TextEdit<'_> {
    TextEdit {
        base_revision,
        start,
        end,
        insert_text: text.as_bytes(),
    }
}

#[test]
fn physical_fragments_and_committed_replacements_share_document_and_revision() {
    let mut physical = BoundedTextState::new(TextStateMode::Edit, 256).unwrap();
    let mut semantic = BoundedTextState::new(TextStateMode::Edit, 256).unwrap();
    for (index, fragment) in ["c", "a", "f", "é"].into_iter().enumerate() {
        let expected = physical
            .apply(fragment.as_bytes())
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(
            semantic
                .replace(edit(index as u64, index as u32, index as u32, fragment))
                .unwrap(),
            expected
        );
        assert_eq!(physical.revision(), semantic.revision());
    }
    assert_eq!(
        semantic.replace(edit(4, 1, 3, "😀")).unwrap(),
        "c😀é".as_bytes()
    );
    assert_eq!(
        semantic.replace(edit(5, 1, 2, "")).unwrap(),
        "cé".as_bytes()
    );
    assert_eq!(semantic.apply(b"\x08").unwrap().unwrap(), b"c");
    assert_eq!(semantic.revision(), 7);
}

#[test]
fn refusals_preserve_all_document_bytes_and_revision() {
    let mut state = BoundedTextState::new(TextStateMode::Edit, 5).unwrap();
    state.replace(edit(0, 0, 0, "café")).unwrap();
    for (proposal, expected) in [
        (edit(0, 0, 0, "x"), TextEditRefusal::StaleRevision),
        (edit(1, 3, 2, ""), TextEditRefusal::InvalidRange),
        (edit(1, 0, 5, ""), TextEditRefusal::InvalidRange),
        (edit(1, 4, 4, "x"), TextEditRefusal::CapacityExhausted),
        (
            TextEdit {
                insert_text: &[0xff],
                ..edit(1, 0, 1, "")
            },
            TextEditRefusal::InvalidUtf8,
        ),
    ] {
        assert_eq!(state.replace(proposal), Err(expected));
        assert_eq!(state.revision(), 1);
        assert_eq!(state.apply(b"\n").unwrap().unwrap(), "café".as_bytes());
    }
}

#[test]
fn insertion_and_replacement_use_exact_utf8_capacity() {
    let mut state = BoundedTextState::new(TextStateMode::Edit, 4).unwrap();
    state.replace(edit(0, 0, 0, "abcd")).unwrap();
    assert_eq!(state.replace(edit(1, 1, 3, "é")).unwrap(), "aéd".as_bytes());
    assert_eq!(state.replace(edit(2, 0, 3, "😀")).unwrap(), "😀".as_bytes());
    assert_eq!(state.replace(edit(3, 0, 1, "")).unwrap(), b"");
}

#[test]
fn submit_mode_cannot_accept_document_replacements() {
    let mut state = BoundedTextState::new(TextStateMode::Submit, 4).unwrap();
    assert_eq!(
        state.replace(edit(0, 0, 0, "a")),
        Err(TextEditRefusal::UnsupportedMode)
    );
    assert_eq!(state.revision(), 0);
}

#[test]
fn canonical_edit_and_document_forms_round_trip_without_allocation() {
    let proposal = edit(9, 1, 3, "é😀");
    let mut bytes = [0; MAXIMUM_TEXT_EDIT_BYTES as usize];
    let length = proposal.encode_into(&mut bytes).unwrap();
    assert_eq!(TextEdit::decode(&bytes[..length]), Ok(proposal));
    assert_eq!(&bytes[..8], &9u64.to_le_bytes());
    let document = TextDocument {
        revision: 10,
        text: "café".as_bytes(),
    };
    let length = document.encode_into(&mut bytes).unwrap();
    assert_eq!(TextDocument::decode(&bytes[..length]), Ok(document));
    assert_eq!(
        TextEdit::decode(&bytes[..15]),
        Err(TextEditRefusal::InvalidEncoding)
    );
    assert_eq!(
        TextDocument::decode(&bytes[..7]),
        Err(TextEditRefusal::InvalidEncoding)
    );
    assert_eq!(
        proposal.encode_into(&mut []),
        Err(TextEditRefusal::InvalidStorage)
    );
    assert_eq!(
        TextDocument {
            revision: 0,
            text: &[0xff]
        }
        .encode_into(&mut bytes),
        Err(TextEditRefusal::InvalidUtf8)
    );
}

#[cfg(feature = "plot-catalog")]
#[test]
fn semantic_editor_loop_checks_with_exact_ports_and_required_label() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_text_state_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot editor {\n state: text/edit-document(maximum-bytes = 256)\n surface: face/editor(\"Message\")\n state.current >> surface.document\n surface.edit >> state.edit\n}\n";
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded = conduit_plot::expand_canonical_plot(&checked, "editor", &profile).unwrap();
    assert_eq!(expanded.gears.len(), 2);
    assert_eq!(expanded.connections.len(), 2);
    assert!(conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            &source.replace("face/editor(\"Message\")", "face/editor")
        ),
        &startup
    )
    .is_err());
}
