//! Selected execution diagnostics consume admitted output backing.
use std::fmt::{Arguments, Write};
pub(super) const CAPACITY: usize = 512;
pub(super) struct Diagnostic {
    buffer: Option<String>,
}
impl Diagnostic {
    pub(super) fn prepare() -> Self {
        Self {
            buffer: Some(String::with_capacity(CAPACITY)),
        }
    }
    pub(super) fn backing_bytes(&self) -> usize {
        self.buffer.as_ref().map_or(0, String::capacity)
    }
}
struct Bounded<'a>(&'a mut String);
impl Write for Bounded<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if text.len() > self.0.capacity() - self.0.len() {
            return Err(std::fmt::Error);
        }
        self.0.push_str(text);
        Ok(())
    }
}
pub(super) fn error(slot: &mut Option<Diagnostic>, arguments: Arguments<'_>) -> String {
    let Some(slot) = slot else {
        return arguments.to_string();
    };
    let Some(mut text) = slot.buffer.take() else {
        return String::new();
    };
    if Bounded(&mut text).write_fmt(arguments).is_err() {
        text.clear();
        text.push_str(
            "selected execution diagnostic exceeded admitted output; typed failure retained",
        );
    }
    text
}

/// Provider errors are independently owned, but retaining one in the selected
/// result must consume our preadmitted diagnostic backing. A diagnostic already
/// produced here has consumed its buffer and needs no second allocation/copy.
pub(super) fn retained_error(slot: &mut Option<Diagnostic>, value: String) -> String {
    if slot.as_ref().is_some_and(|slot| slot.buffer.is_some()) {
        error(slot, format_args!("{value}"))
    } else {
        value
    }
}
