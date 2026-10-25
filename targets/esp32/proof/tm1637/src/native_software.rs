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

#[cfg(feature = "body-image-root")]
pub fn planning_source(
) -> Result<conduit_plot::flat_image::indexed::IndexedSourceImage<'static>, &'static str> {
    let bytes = material::planning_source_image();
    if conduit_core::assigned_plan_payload_digest(bytes) != material::planning_source_sha256() {
        return Err("body-source-image-digest");
    }
    // SAFETY: these exact bytes and the independent expected digest are linked
    // from this trusted Make build, which checked the complete expansion. No
    // network/UART input or claimed Source identity enters this installation.
    unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(bytes) }
        .map_err(|_| "body-source-image-representation")
}
