//! Exact initial native workset preparation, before ordinary kernel admission.
mod catalog;
mod keyboard_delivery;
mod planning;
mod play;
mod text_state;

pub use catalog::{NativeForm, checked, inventory, resident};
pub use planning::{PreparedNativeWorkset, prepare};
pub use play::{NativePresentation, NativeWorksetPlay, PlayRefusal};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorksetRefusal {
    UnknownForm,
    WorksetBound,
    Catalog,
    Host,
    Plan,
    Resource,
    Capability,
    Lowering,
    Kernel,
}

#[cfg(test)]
mod tests;
