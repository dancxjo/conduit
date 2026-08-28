//! Portable catalog meaning for bounded bitmap presentation.

#[cfg(feature = "form-catalog")]
use alloc::{string::ToString, vec, vec::Vec};
#[cfg(feature = "form-catalog")]
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};
#[cfg(feature = "form-catalog")]
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

#[cfg(feature = "form-catalog")]
use crate::GRAY8_BITMAP_INFO_KIND;

pub const BITMAP_PRESENTATION_KIND: &str = "presentation/bitmap";
pub const BITMAP_PRESENTATION_REVISION: &str = "conduit.presentation/bitmap@1";

#[cfg(feature = "form-catalog")]
pub fn bitmap_presentation_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(BITMAP_PRESENTATION_KIND),
        kind_contract_revision: KindContractRevision::from(BITMAP_PRESENTATION_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("bitmap"),
            value_kind: kind_id(GRAY8_BITMAP_INFO_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_bitmap_presentation_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(KindSignature {
        kind: BITMAP_PRESENTATION_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(bitmap_presentation_definition())
        .map_err(|error| error.to_string())
}
