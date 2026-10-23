//! Own actual Source-compiled portable software before any climate assignment.
//! This starts no Body, kernel, sensor or timer operation.
use conduit_environment_runtime::appliance::assigned::{
    PreparedResidentSoftware, ReadyResidentOffers,
};
mod material {
    include!(concat!(env!("OUT_DIR"), "/appliance-software.rs"));
}
pub type Software = PreparedResidentSoftware<conduit_kernel::HostedValueStore>;
#[cold]
#[inline(never)]
pub fn prepare() -> Result<Software, ()> {
    let values = conduit_environment_runtime::appliance::prepare_value_storage().map_err(|_| ())?;
    Software::prepare_source(&material::spec(), material::computations(), values).map_err(|_| ())
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
