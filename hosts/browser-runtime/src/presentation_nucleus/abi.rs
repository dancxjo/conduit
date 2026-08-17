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
    structured: [u8; 512],
    structured_len: usize,
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
            let (structured, structured_len) = encode_structured(&proof.structured);
            BROWSER_NUCLEUS.with(|slot| {
                *slot.borrow_mut() = Some(BrowserNucleusBuffers {
                    graphics: proof.graphics.encode(),
                    graphics_len: proof.graphics.encoded_len(),
                    layout: proof.layout.encode(),
                    layout_len: proof.layout.encoded_len(),
                    text,
                    text_len: proof.text.len(),
                    structured,
                    structured_len,
                });
            });
            0
        }
        Err(_) => -1,
    }
}

fn encode_structured(
    artifact: &conduit_presentation::StructuredSignPresentation,
) -> ([u8; 512], usize) {
    use conduit_presentation::PresentationPropertyValue;
    let mut schema = "";
    let mut variant = "";
    let mut unit = "";
    let mut quantity = 0_i64;
    for property in &artifact.presentation.properties {
        match (property.name.as_str(), &property.value) {
            ("record-schema", PresentationPropertyValue::Identity(value))
                if value == "education/feedback@1" =>
            {
                schema = value
            }
            ("active-variant-tag", PresentationPropertyValue::Identity(value)) => variant = value,
            ("quantity-unit", PresentationPropertyValue::Identity(value)) => unit = value,
            ("quantity-value", PresentationPropertyValue::Signed(value)) => quantity = *value,
            _ => {}
        }
    }
    let mut encoded = [0_u8; 512];
    encoded[0] = 1;
    let mut offset = 1;
    for value in [schema, variant, unit] {
        encoded[offset] = value.len() as u8;
        offset += 1;
        encoded[offset..offset + value.len()].copy_from_slice(value.as_bytes());
        offset += value.len();
    }
    encoded[offset..offset + 8].copy_from_slice(&quantity.to_le_bytes());
    offset += 8;
    (encoded, offset)
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

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_structured_ptr() -> usize {
    BROWSER_NUCLEUS.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |proof| proof.structured.as_ptr() as usize)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_presentation_nucleus_structured_len() -> usize {
    BROWSER_NUCLEUS.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |proof| proof.structured_len)
    })
}
