//! The same portable keymap semantics used by the ordinary Host implementation.
use crate::{frame::TextFrame, gate};
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeymapDisposition};
#[allow(dead_code)] // The shared owner also implements the separately admitted Submit mode.
#[path = "../../../semantics/catalog/src/text_state/retained.rs"]
mod retained;
use retained::{RetainedText, TextStateMode, TextStateRefusal};
struct State {
    keymap: ConduitIntlKeymap,
    editor: Option<RetainedText>,
    text: [u8; 256],
}

// The bottom page of this region's private stack allocation holds retained
// pure state. It never contains a Root pointer and is not a shared window.
#[cfg(not(target_arch = "x86"))]
const STATE: *mut State = 0x420000 as *mut State;
#[cfg(target_arch = "x86")]
const STATE: *mut State = 0x40020000 as *mut State;
const _: () = assert!(core::mem::size_of::<State>() <= 4096);

pub unsafe fn initialize(frame: &mut TextFrame) -> ! {
    unsafe {
        STATE.write(State {
            keymap: ConduitIntlKeymap::new(),
            editor: None,
            text: [0; 256],
        });
    }
    frame.output_length = 0;
    frame.status = 0;
    gate::finish(0)
}
pub unsafe fn apply(frame: &mut TextFrame) -> ! {
    frame.status = unsafe { transform(frame) };
    gate::finish(frame.status)
}

pub unsafe fn chain(frame: &mut TextFrame) -> ! {
    frame.intermediate_length = 0;
    frame.status = unsafe { transform(frame) };
    if frame.status == 0 && frame.output_length != 0 {
        let length = frame.output_length as usize;
        if length > frame.intermediate.len() {
            gate::finish(3);
        }
        frame.intermediate[..length].copy_from_slice(&frame.output[..length]);
        frame.intermediate_length = length as u32;
        frame.status = match crate::text_transform::uppercase_into(
            &frame.intermediate[..length],
            &mut frame.output,
        ) {
            Ok(length) => {
                frame.output_length = length as u32;
                0
            }
            Err(_) => 2,
        };
    }
    gate::finish(frame.status)
}

unsafe fn transform(frame: &mut TextFrame) -> u32 {
    let length = frame.input_length as usize;
    if length > frame.input.len() {
        return 1;
    }
    let event = match KeyEvent::decode(&frame.input[..length]) {
        Ok(event) => event,
        Err(_) => return 1,
    };
    frame.output_length = 0;
    match unsafe { (&mut *STATE).keymap.apply(event) } {
        KeymapDisposition::Text(text) => {
            let mut utf8 = [0; 4];
            let bytes = text.encode_utf8(&mut utf8);
            frame.output[..bytes.len()].copy_from_slice(bytes);
            frame.output_length = bytes.len() as u32;
            0
        }
        KeymapDisposition::NoText | KeymapDisposition::Cancelled => 0,
        KeymapDisposition::Refused(_) => 1,
    }
}

/// Root supplies the selected text/edit configuration before any input runs.
pub unsafe fn initialize_editor(frame: &mut TextFrame) -> ! {
    if frame.input_length != 8 {
        gate::finish(3);
    }
    let maximum = u64::from_le_bytes(frame.input[..8].try_into().unwrap());
    let editor = usize::try_from(maximum)
        .ok()
        .and_then(|maximum| RetainedText::new(TextStateMode::Edit, maximum).ok());
    let Some(editor) = editor else {
        gate::finish(3)
    };
    unsafe {
        STATE.write(State {
            keymap: ConduitIntlKeymap::new(),
            editor: Some(editor),
            text: [0; 256],
        });
    }
    frame.output_length = 0;
    frame.status = 0;
    gate::finish(0)
}

/// Both pure operations run in one entry; no Root call performs editing.
pub unsafe fn edit_chain(frame: &mut TextFrame) -> ! {
    frame.intermediate_length = 0;
    frame.status = unsafe { transform(frame) };
    if frame.status == 0 && frame.output_length != 0 {
        let length = frame.output_length as usize;
        if length > frame.intermediate.len() {
            gate::finish(3);
        }
        frame.intermediate[..length].copy_from_slice(&frame.output[..length]);
        frame.intermediate_length = length as u32;
        let state = unsafe { &mut *STATE };
        let Some(editor) = &mut state.editor else {
            gate::finish(3)
        };
        frame.output_length = 0;
        frame.status = match editor.apply(&mut state.text, &frame.intermediate[..length]) {
            Ok(Some(output)) => {
                frame.output[..output.len()].copy_from_slice(output);
                frame.output_length = output.len() as u32;
                0
            }
            Ok(None) => 0,
            Err(TextStateRefusal::InvalidUtf8) => 1,
            Err(TextStateRefusal::CapacityExhausted) => 2,
            Err(TextStateRefusal::InvalidCapacity) => 3,
        };
    }
    gate::finish(frame.status)
}
