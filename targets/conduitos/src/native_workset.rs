//! Exact initial native workset preparation, before ordinary kernel admission.
mod application_delivery;
mod catalog;
mod keyboard_delivery;
mod planning;
mod play;
mod text_state;
mod tutorial_application;

pub use application_delivery::NativeApplicationRequest;
pub use catalog::{
    NATIVE_PLOT_CAPACITY, NativePlot, NativePlotProfile, checked, inventory, profile, resident,
    resolve,
};
pub use planning::{AdmittedPlotInput, PreparedNativeWorkset, prepare, review};
pub(crate) use planning::{prepare_exact, propose_partitions};
pub use play::{NativePresentation, NativeWorksetPlay, PlayRefusal};
pub use tutorial_application::TutorialAction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorksetRefusal {
    UnknownPlot,
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
            Self::UnknownPlot => "native-body-plot-unavailable",
            Self::WorksetBound => "native-body-plot-capacity-exceeded",
            Self::Catalog => "native-body-plot-check-refused",
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
pub(crate) mod tests;
