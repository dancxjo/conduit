//! Canonical bounded document operations inside the existing protected owner.
//! This prepares an implementation boundary; no editor Host offer is advertised.
use crate::{frame::TextFrame, keymap::retained};
use retained::{MAXIMUM_EDITED_TEXT_BYTES, RetainedText, TextEdit, TextEditRefusal};
#[allow(dead_code)] // The shared codec also exposes the receiving-side decoder.
#[path = "../../../semantics/catalog/src/text_state/edit_codec.rs"]
mod codec;

/// Command 14: operation 0 reads; operation 1 applies one canonical replacement.
/// Status 1 carries these exact typed refusal codes. Status 3 is a malformed
/// mechanism request, not a new service/privileged gate operation.
pub(super) fn execute(
    frame: &mut TextFrame,
    editor: Option<&mut RetainedText>,
    text: &mut [u8; crate::frame::TEXT_CAPACITY],
) -> u32 {
    frame.output_length = 0;
    frame.document_output_length = 0;
    frame.document_refusal = 0;
    frame.document_output.fill(0);
    let length = frame.document_input_length as usize;
    if length > frame.document_input.len()
        || !matches!(frame.operation, 0 | 1)
        || (frame.operation == 0 && length != 0)
    {
        return 3;
    }
    let Some(editor) = editor else {
        return refuse(frame, TextEditRefusal::UnsupportedMode);
    };
    if frame.operation == 1 {
        let edit = match TextEdit::decode(&frame.document_input[..length]) {
            Ok(edit) => edit,
            Err(error) => return refuse(frame, error),
        };
        if let Err(error) = editor.replace(text, edit) {
            return refuse(frame, error);
        }
    }
    let current = match editor.current(text) {
        Ok(current) => current,
        Err(error) => return refuse(frame, error),
    };
    let document = codec::TextDocument {
        revision: editor.revision(),
        text: current,
    };
    match document.encode_into(&mut frame.document_output) {
        Ok(length) => {
            frame.document_output_length = length as u32;
            0
        }
        Err(error) => refuse(frame, error),
    }
}

fn refuse(frame: &mut TextFrame, error: TextEditRefusal) -> u32 {
    frame.document_refusal = match error {
        TextEditRefusal::StaleRevision => 1,
        TextEditRefusal::InvalidEncoding => 2,
        TextEditRefusal::InvalidRange => 3,
        TextEditRefusal::InvalidUtf8 => 4,
        TextEditRefusal::CapacityExhausted => 5,
        TextEditRefusal::RevisionExhausted => 6,
        TextEditRefusal::InvalidStorage => 7,
        TextEditRefusal::UnsupportedMode => 8,
    };
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use retained::TextStateMode;

    fn frame() -> TextFrame {
        // Every field is an integer or integer array: zero is a valid value.
        unsafe { core::mem::zeroed() }
    }

    #[test]
    fn canonical_replacement_and_physical_edits_share_one_revisioned_owner() {
        let mut editor = RetainedText::new(TextStateMode::Edit, 256).unwrap();
        let mut text = [0; 256];
        editor.apply(&mut text, "é".as_bytes()).unwrap();
        let mut frame = frame();
        frame.operation = 1;
        let edit = TextEdit {
            base_revision: 1,
            start: 1,
            end: 1,
            insert_text: b"x",
        };
        frame.document_input_length = edit.encode_into(&mut frame.document_input).unwrap() as u32;
        assert_eq!(execute(&mut frame, Some(&mut editor), &mut text), 0);
        let observed = codec::TextDocument::decode(
            &frame.document_output[..frame.document_output_length as usize],
        )
        .unwrap();
        assert_eq!(observed.revision, 2);
        assert_eq!(observed.text, "éx".as_bytes());
        assert_eq!(
            editor.apply(&mut text, b"\x08").unwrap(),
            Some("é".as_bytes())
        );
        frame.operation = 0;
        frame.document_input_length = 0;
        assert_eq!(execute(&mut frame, Some(&mut editor), &mut text), 0);
        let observed = codec::TextDocument::decode(
            &frame.document_output[..frame.document_output_length as usize],
        )
        .unwrap();
        assert_eq!(observed.revision, 3);
        assert_eq!(observed.text, "é".as_bytes());
    }

    #[test]
    fn stale_and_malformed_edits_leave_owner_and_output_unchanged() {
        let mut editor = RetainedText::new(TextStateMode::Edit, 4).unwrap();
        let mut text = [0; 256];
        editor.apply(&mut text, b"ok").unwrap();
        let mut frame = frame();
        frame.operation = 1;
        let edit = TextEdit {
            base_revision: 0,
            start: 0,
            end: 0,
            insert_text: b"x",
        };
        frame.document_input_length = edit.encode_into(&mut frame.document_input).unwrap() as u32;
        frame.document_output_length = 100;
        assert_eq!(execute(&mut frame, Some(&mut editor), &mut text), 1);
        assert_eq!(frame.document_refusal, 1);
        assert_eq!(frame.document_output_length, 0);
        assert_eq!(editor.revision(), 1);
        assert_eq!(editor.current(&text).unwrap(), b"ok");
        frame.document_input_length = 1;
        assert_eq!(execute(&mut frame, Some(&mut editor), &mut text), 1);
        assert_eq!(frame.document_refusal, 2);
        frame.operation = 2;
        assert_eq!(execute(&mut frame, Some(&mut editor), &mut text), 3);
        assert_eq!(frame.document_refusal, 0);
        assert_eq!(execute(&mut frame, None, &mut text), 3);
        frame.operation = 0;
        frame.document_input_length = 0;
        assert_eq!(execute(&mut frame, None, &mut text), 1);
        assert_eq!(frame.document_refusal, 8);
    }
}
