//! Exact initial native workset preparation, before ordinary kernel admission.
mod catalog;
mod keyboard_delivery;
mod planning;
mod play;
mod text_state;

pub use catalog::{NativeForm, checked, inventory, resident, resolve};
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

impl WorksetRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownForm => "native-body-form-unavailable",
            Self::WorksetBound => "native-body-form-capacity-exceeded",
            Self::Catalog => "native-body-form-check-refused",
            Self::Host => "native-body-current-host-unavailable",
            Self::Plan => "native-body-plan-refused",
            Self::Resource => "native-body-resource-capacity-exceeded",
            Self::Capability => "native-body-capability-capacity-exceeded",
            Self::Lowering => "native-body-kernel-tables-exceeded",
            Self::Kernel => "native-body-kernel-preparation-refused",
        }
    }
}

#[cfg(test)]
mod tests;
