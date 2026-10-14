//! Portable catalog meaning for bounded bitmap face.

#[cfg(feature = "plot-catalog")]
use alloc::{string::ToString, vec, vec::Vec};
#[cfg(feature = "plot-catalog")]
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindConfigurationField, KindConfigurationRule,
    KindIdentity, PortDescriptor, PortDirection, PortTemporal,
};
#[cfg(feature = "plot-catalog")]
use conduit_plot::{
    KindProjection, KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};

#[cfg(feature = "plot-catalog")]
use crate::GRAY8_BITMAP_INFO_KIND;

pub const BITMAP_FACE_KIND: &str = "face/bitmap";
pub const BITMAP_FACE_REVISION: &str = "conduit.face/bitmap@1";

#[cfg(feature = "plot-catalog")]
pub fn bitmap_face_definition() -> KindProjection {
    KindProjection {
        kind_id: kind_id(BITMAP_FACE_KIND),
        kind_contract_revision: KindIdentity::from(BITMAP_FACE_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("bitmap"),
            value_kind: kind_id(GRAY8_BITMAP_INFO_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: vec![KindConfigurationField {
            key: "label".to_string(),
            default_value: ConfigurationValue::Text(alloc::string::String::new()),
            rule: KindConfigurationRule::DataLabel { maximum: 128 },
        }],
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_bitmap_face_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(KindSignature {
        kind: BITMAP_FACE_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "label".to_string(),
            value_type: "Text".to_string(),
            default: None,
        }],
    })?;
    profile
        .insert(bitmap_face_definition())
        .map_err(|error| error.to_string())
}
