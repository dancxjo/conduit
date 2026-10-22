//! Exact prepared effect routing; no application or document state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Effect {
    Keyboard,
    Keymap,
    Upper,
    Edit,
    Document,
    Face,
    ApplicationEvent,
    Application,
    ApplicationFace,
}
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) plot: u8,
    pub(super) effect: Effect,
}
