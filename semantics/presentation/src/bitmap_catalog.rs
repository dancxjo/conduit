//! Portable catalog meaning for bounded bitmap presentation.

#[cfg(feature = "plot-catalog")]
use alloc::{string::ToString, vec, vec::Vec};
#[cfg(feature = "plot-catalog")]
use conduit_core::{kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal};
#[cfg(feature = "plot-catalog")]
use conduit_plot::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

#[cfg(feature = "plot-catalog")]
use crate::GRAY8_BITMAP_INFO_KIND;

pub const BITMAP_PRESENTATION_KIND: &str = "presentation/bitmap";
pub const BITMAP_PRESENTATION_REVISION: &str = "conduit.presentation/bitmap@1";

#[cfg(feature = "plot-catalog")]
pub fn bitmap_presentation_definition() -> KindProjection {
    KindProjection {
        kind_id: kind_id(BITMAP_PRESENTATION_KIND),
        kind_contract_revision: KindIdentity::from(BITMAP_PRESENTATION_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("bitmap"),
            value_kind: kind_id(GRAY8_BITMAP_INFO_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: Default::default(),
    }
}

#[cfg(feature = "plot-catalog")]
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
