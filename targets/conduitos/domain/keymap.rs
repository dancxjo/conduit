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
    let length = frame.input_length as usize;
    if length > frame.input.len() {
        gate::finish(1);
    }
    let event = match KeyEvent::decode(&frame.input[..length]) {
        Ok(event) => event,
        Err(_) => gate::finish(1),
    };
    frame.output_length = 0;
    frame.status = match unsafe { (&mut *STATE).apply(event) } {
        KeymapDisposition::Text(text) => {
            let mut utf8 = [0; 4];
            let bytes = text.encode_utf8(&mut utf8);
            frame.output[..bytes.len()].copy_from_slice(bytes);
            frame.output_length = bytes.len() as u32;
            0
        }
        KeymapDisposition::NoText | KeymapDisposition::Cancelled => 0,
        KeymapDisposition::Refused(_) => 1,
    };
    gate::finish(frame.status)
}
