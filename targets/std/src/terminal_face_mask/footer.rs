//! Keep admitted keyboard instructions visible within the finite viewport.
pub(super) fn help(interactive: bool, columns: usize) -> (&'static str, Option<&'static str>) {
    let full = if interactive {
        "Tab focus · ↑↓ read · PgUp/PgDn · F2 inspect · Enter apply · Esc cancel"
    } else {
        "Read only · ↑↓ read · PgUp/PgDn · F2 inspect · Esc close"
    };
    if full.chars().count() <= columns {
        return (full, None);
    }
    if interactive {
        (
            "Tab focus · ↑↓ read · PgUp/PgDn",
            Some("F2 inspect · Enter apply · Esc cancel"),
        )
    } else {
        (
            "Read only · ↑↓ read · PgUp/PgDn",
            Some("F2 inspect · Esc close"),
        )
    }
}
pub(super) fn rows(interactive: bool, columns: usize) -> usize {
    1 + usize::from(help(interactive, columns).1.is_some())
}
