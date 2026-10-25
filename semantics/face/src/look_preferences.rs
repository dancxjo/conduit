//! Typed, finite preference resolution before checked Mask expansion/Plan.
//! Decoding preference data does not establish a saved-default commit: the
//! caller independently admits and retrieves the selected profile resource.
use crate::{
    LookInfo, LookPreferenceLookOption, LookPreferenceTemperatureOption, LookPreferencesInfo,
    LookQuantityFacetOption, LookTemperatureUnit, MAX_LOOK_BYTES,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

impl LookPreferenceLookOption {
    pub fn from_look(look: LookInfo) -> Result<Self, NativeBindingRefusal> {
        Self::some(
            look.generative().clone(),
            look.identity().clone(),
            look.quantity().clone(),
            look.spoken().clone(),
            *look.version(),
            look.visual().clone(),
        )
    }
}

pub const LOOK_PREFERENCES_SOURCE: &str = include_str!("../look-preferences.conduit");
pub const MAX_LOOK_PREFERENCES_BYTES: usize = MAX_LOOK_BYTES;
#[derive(Debug)]
pub enum LookPreferencesRefusal {
    TooLarge,
    Native(NativeBindingRefusal),
}
/// Field origin is retained independently of later visual/Host admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookPreferenceOrigin {
    Absent,
    Authored,
    RetainedLook,
    RetainedTemperatureUnit,
    EncounterLook,
    EncounterTemperatureUnit,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedLookPreferences {
    requested: LookInfo,
    look_origin: LookPreferenceOrigin,
    quantity_origin: LookPreferenceOrigin,
}
impl ResolvedLookPreferences {
    pub fn requested(&self) -> &LookInfo {
        &self.requested
    }
    pub const fn look_origin(&self) -> LookPreferenceOrigin {
        self.look_origin
    }
    pub const fn quantity_origin(&self) -> LookPreferenceOrigin {
        self.quantity_origin
    }
}

pub fn encode_look_preferences(
    value: &LookPreferencesInfo,
) -> Result<Vec<u8>, LookPreferencesRefusal> {
    let bytes = value
        .clone()
        .encode()
        .map_err(LookPreferencesRefusal::Native)?;
    if bytes.len() > MAX_LOOK_PREFERENCES_BYTES {
        return Err(LookPreferencesRefusal::TooLarge);
    }
    Ok(bytes)
}
pub fn decode_look_preferences(
    bytes: &[u8],
) -> Result<LookPreferencesInfo, LookPreferencesRefusal> {
    if bytes.len() > MAX_LOOK_PREFERENCES_BYTES {
        return Err(LookPreferencesRefusal::TooLarge);
    }
    LookPreferencesInfo::decode(bytes).map_err(LookPreferencesRefusal::Native)
}

/// Authored -> retained user defaults -> explicit encounter choices. In each
/// preference layer, a selected Look contributes its present quantity facet;
/// an explicit unit then overrides that facet. Selecting another palette with
/// no quantity facet inherits the earlier unit instead of resetting it.
/// An absent unit preference means inherit; removing a retained override needs
/// an independently committed replacement preference record with that field
/// absent. This function performs no storage, authority or renderer operation.
/// Accessibility and hard Host constraints are admitted after this resolution.
pub fn resolve_look_preferences(
    authored: &LookInfo,
    retained: Option<&LookPreferencesInfo>,
    encounter: Option<&LookPreferencesInfo>,
) -> Result<ResolvedLookPreferences, NativeBindingRefusal> {
    use LookPreferenceOrigin as O;
    let mut base = authored.clone();
    let mut quantity = authored.quantity().clone();
    let mut look_origin = O::Authored;
    let mut quantity_origin = if matches!(quantity, LookQuantityFacetOption::Some(_)) {
        O::Authored
    } else {
        O::Absent
    };
    for (layer, look_source, unit_source) in [
        (retained, O::RetainedLook, O::RetainedTemperatureUnit),
        (encounter, O::EncounterLook, O::EncounterTemperatureUnit),
    ] {
        let Some(layer) = layer else {
            continue;
        };
        if let LookPreferenceLookOption::Some(look) = layer.look() {
            base = LookInfo::new(
                look.generative().clone(),
                look.identity().clone(),
                look.quantity().clone(),
                look.spoken().clone(),
                *look.version(),
                look.visual().clone(),
            )?;
            look_origin = look_source;
            if matches!(look.quantity(), LookQuantityFacetOption::Some(_)) {
                quantity = look.quantity().clone();
                quantity_origin = look_source;
            }
        }
        if let LookPreferenceTemperatureOption::Some(unit) = layer.temperature_unit() {
            let unit: &LookTemperatureUnit = unit;
            quantity = LookQuantityFacetOption::some(*unit)?;
            quantity_origin = unit_source;
        }
    }
    Ok(ResolvedLookPreferences {
        requested: LookInfo::new(
            base.generative().clone(),
            base.identity().clone(),
            quantity,
            base.spoken().clone(),
            *base.version(),
            base.visual().clone(),
        )?,
        look_origin,
        quantity_origin,
    })
}
