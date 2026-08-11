use super::execute_browser_nucleus;
use conduit_presentation::{MAX_GRAPHICS_SCENE_BYTES, MAX_LAYOUT_FRAME_BYTES};
use std::cell::RefCell;

struct BrowserNucleusBuffers {
    graphics: [u8; MAX_GRAPHICS_SCENE_BYTES],
    graphics_len: usize,
    layout: [u8; MAX_LAYOUT_FRAME_BYTES],
    layout_len: usize,
    text: [u8; conduit_std_catalog::MAX_TEXT_BYTES as usize],
    text_len: usize,
}

thread_local! {
    static BROWSER_NUCLEUS: RefCell<Option<BrowserNucleusBuffers>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_run() -> i32 {
    match execute_browser_nucleus() {
        Ok(proof) => {
            let mut text = [0; conduit_std_catalog::MAX_TEXT_BYTES as usize];
            text[..proof.text.len()].copy_from_slice(proof.text.as_bytes());
            BROWSER_NUCLEUS.with(|slot| {
                *slot.borrow_mut() = Some(BrowserNucleusBuffers {
                    graphics: proof.graphics.encode(),
                    graphics_len: proof.graphics.encoded_len(),
                    layout: proof.layout.encode(),
                    layout_len: proof.layout.encoded_len(),
                    text,
                    text_len: proof.text.len(),
                });
            });
            0
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_graphics_ptr() -> usize {
    BROWSER_NUCLEUS.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |proof| proof.graphics.as_ptr() as usize)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_graphics_len() -> usize {
    BROWSER_NUCLEUS.with(|slot| slot.borrow().as_ref().map_or(0, |proof| proof.graphics_len))
}

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_layout_ptr() -> usize {
    BROWSER_NUCLEUS.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |proof| proof.layout.as_ptr() as usize)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_layout_len() -> usize {
    BROWSER_NUCLEUS.with(|slot| slot.borrow().as_ref().map_or(0, |proof| proof.layout_len))
}

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_text_ptr() -> usize {
    BROWSER_NUCLEUS.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |proof| proof.text.as_ptr() as usize)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_text_len() -> usize {
    BROWSER_NUCLEUS.with(|slot| slot.borrow().as_ref().map_or(0, |proof| proof.text_len))
}
