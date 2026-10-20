//! Diagnostic reconstruction stays outside Play and preserves exact event text.
use conduit_kernel::KernelEvent;
use std::fmt::Write;

pub(super) fn render(events: &[KernelEvent]) -> String {
    let mut text = String::new();
    for event in events {
        // A String writer cannot refuse formatting. Reuse its growing buffer
        // instead of allocating and growing one temporary String per Sign.
        writeln!(&mut text, "{event:?}").expect("String formatting is infallible");
    }
    text
}
