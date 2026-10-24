//! Own actual Source-compiled portable software before any climate assignment.
//! This starts no Body, kernel, sensor or timer operation.
use alloc::boxed::Box;
use conduit_environment_runtime::appliance::assigned::{
    PreparedResidentSoftware, ReadyResidentOffers,
};
mod material {
    include!(concat!(env!("OUT_DIR"), "/appliance-software.rs"));
}
pub type Software = PreparedResidentSoftware<conduit_kernel::HostedValueStore>;
#[cold]
#[inline(never)]
pub fn prepare() -> Result<Box<Software>, ()> {
    let values = conduit_environment_runtime::appliance::prepare_value_storage().map_err(|_| ())?;
    Software::prepare_source(&material::spec(), material::computations(), values)
        .map(Box::new)
        .map_err(|_| ())
}
pub fn offers(
    software: &Software,
) -> Result<ReadyResidentOffers<'_, conduit_kernel::HostedValueStore>, ()> {
    software
        .source_offers(&material::spec(), material::offers())
        .map_err(|_| ())
}

pub fn source_facts() -> ([u8; 32], conduit_time::TickConfiguration) {
    let spec = material::spec();
    (spec.source.expanded_plot_digest, spec.cadence)
}
pub fn resident(
) -> Result<conduit_environment_runtime::appliance::assigned::ResidentAppliance<'static>, ()> {
    let spec = material::spec();
    Ok(
        conduit_environment_runtime::appliance::assigned::ResidentAppliance {
            layout: crate::appliance_layout::layout(spec.source)?,
            cadence: spec.cadence,
            source: spec.source,
            computations: spec.computations,
            roles: spec.roles,
        },
    )
}
pub fn computation_records() -> &'static [conduit_core::AssignedRecord<'static>] {
    material::computation_records()
}

#[cfg(feature = "native-face-root")]
pub fn source_identity() -> conduit_core::PlotIdentity {
    material::source_identity()
}
