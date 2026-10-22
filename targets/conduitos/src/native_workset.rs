//! Exact initial native workset preparation, before ordinary kernel admission.
mod application_delivery;
mod catalog;
mod eligibility;
mod keyboard_delivery;
mod planning;
mod patchbay_ingress;
mod play;
mod preparation;
mod text_state;
mod document_state;
mod editor_delivery;
mod tutorial_application;

pub use application_delivery::NativeApplicationRequest;
pub use catalog::{
    NATIVE_CANDIDATE_CAPACITY, NATIVE_PLOT_CAPACITY, NativePlot, NativePlotProfile,
    candidate_inventory, checked, inventory, profile, resident, resolve,
};
pub use eligibility::eligibility;
pub use planning::{AdmittedPlotInput, PreparedNativeWorkset, prepare, review};
pub(crate) use planning::{prepare_exact, propose_partitions};
pub use play::{NativeFace, NativeWorksetPlay, PlayRefusal};
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
    Application(conduitos_application_core::data::Refusal),
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
            Self::Application(error) => match error {
                conduitos_application_core::data::Refusal::Bounds => {
                    "native-body-application-bounds-refused"
                }
                conduitos_application_core::data::Refusal::Malformed => {
                    "native-body-application-message-malformed"
                }
                conduitos_application_core::data::Refusal::Order => {
                    "native-body-application-order-refused"
                }
                conduitos_application_core::data::Refusal::Capacity => {
                    "native-body-application-capacity-exceeded"
                }
                conduitos_application_core::data::Refusal::Application => {
                    "native-body-application-initialization-refused"
                }
            },
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
