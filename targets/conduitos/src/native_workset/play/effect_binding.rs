//! Exact prepared effect routing; no application or document state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Effect {
    Keyboard,
    Keymap,
    Upper,
    Edit,
    Document,
    Editor,
    Face,
    ApplicationEvent,
    Application,
    ApplicationFace,
    Patchbay,
}
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) plot: u8,
    pub(super) effect: Effect,
}
