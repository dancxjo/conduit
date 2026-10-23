//! Bounded provisional preview; the owned argument bytes remain unchanged.
use super::*;

impl InputState {
    pub fn preview(
        &self,
        focus: Option<FaceControl>,
        chrome: &super::super::chrome::Chrome,
        scale: u16,
    ) -> Result<Option<String>, FaceSceneError> {
        let Some(control) = focus else {
            return Ok(None);
        };
        if self.action != Some(control.action) {
            return Ok(None);
        }
        let Some(bytes) = control
            .argument
            .and_then(|argument| self.values.get(argument))
            .and_then(Option::as_ref)
        else {
            return Ok(None);
        };
        let value = match bytes.as_slice() {
            [0] => "false",
            [1] => "true",
            _ => match core::str::from_utf8(bytes) {
                Ok(value) => value,
                Err(_) => return Ok(None),
            },
        };
        chrome.draft_preview(value, scale).map(Some)
    }
}
