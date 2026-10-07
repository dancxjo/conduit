//! The same portable keymap semantics used by the ordinary Host implementation.
use crate::{frame::TextFrame, gate};
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeymapDisposition};

// The bottom page of this region's private stack allocation holds retained
// pure state. It never contains a Root pointer and is not a shared window.
const STATE: *mut ConduitIntlKeymap = 0x420000 as *mut ConduitIntlKeymap;
const _: () = assert!(core::mem::size_of::<ConduitIntlKeymap>() <= 4096);

pub unsafe fn initialize(frame: &mut TextFrame) -> ! {
    unsafe {
        STATE.write(ConduitIntlKeymap::new());
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
    match unsafe { (&mut *STATE).apply(event) } {
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
