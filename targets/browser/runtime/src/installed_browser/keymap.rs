#[path = "keymap_back.rs"]
mod keymap_back;
// Stateful portable keyboard interpretation for the ordinary browser runner.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{kind_id, HostCallContractId, HostCallRequirement, PlannedGear};
use conduit_human::{KeyEvent, KeymapRefusal, PreparedKeymapChoices as AuthoredKeymap};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const KEYMAP_SELECT: &str = "conduit.host/browser-input-keymap-select@1";
pub(crate) const HOST_CALL: &str = "conduit.host/browser-input-keymap@1";
pub(super) const IMPLEMENTATION: &str = "browser/kernel-input-keymap@1";
const ARTIFACT: &str = "conduit-browser-runtime/input-keymap@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::keymap_contract(),
        conduit_semantic_catalog::KEYMAP_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![
            HostCallRequirement {
                contract_id: HostCallContractId::from(KEYMAP_SELECT),
                target_kind: Some(kind_id("input/keymap-selection")),
                maximum_in_flight: 1,
                maximum_input_bytes: 32,
                maximum_output_bytes: 0,
            },
            HostCallRequirement {
                contract_id: HostCallContractId::from(HOST_CALL),
                target_kind: Some(kind_id("input/keymap-text-fragment")),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
                maximum_output_bytes: conduit_human::KEYMAP_MAXIMUM_TEXT_BYTES as u32,
            },
        ],
        Vec::new(),
        Vec::new(),
    )
}

fn prepare(placement: &PlannedGear, values: &mut HostedValueStore) -> Result<BrowserBack, String> {
    prepare_routed(placement, values, false)
}
pub(crate) fn prepare_routed(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
    require_selection: bool,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserBack::installed_step(
        keymap_back::KeymapBack::with_selection(require_selection),
    ))
}

pub(crate) struct PreparedKeymap {
    state: AuthoredKeymap,
}

pub(crate) struct EncodedText {
    bytes: [u8; conduit_human::KEYMAP_MAXIMUM_TEXT_BYTES],
    len: usize,
}

impl EncodedText {
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl PreparedKeymap {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        Ok(Some(Self {
            state: AuthoredKeymap::new(
                configuration(placement, "layout")?,
                configuration(placement, "choices")?,
            )?,
        }))
    }

    pub(crate) fn select(&mut self, input: &[u8]) -> Result<(), Failure> {
        self.state.select(input).map_err(|_| failure(6))
    }

    pub(crate) fn execute(&mut self, input: &[u8]) -> Result<Option<EncodedText>, Failure> {
        let event = KeyEvent::decode(input).map_err(|_| failure(1))?;
        match self.state.apply(event) {
            Ok(Some(text)) => {
                let mut bytes = [0; conduit_human::KEYMAP_MAXIMUM_TEXT_BYTES];
                let encoded = text.as_slice();
                bytes[..encoded.len()].copy_from_slice(encoded);
                Ok(Some(EncodedText {
                    bytes,
                    len: encoded.len(),
                }))
            }
            Ok(None) => Ok(None),
            Err(reason) => Err(failure(match reason {
                KeymapRefusal::UnknownComposeSequence => 2,
                KeymapRefusal::EmptyUnicodeEntry => 3,
                KeymapRefusal::UnicodeEntryOverflow => 4,
                KeymapRefusal::InvalidUnicodeScalar => 5,
            })),
        }
    }
}

const fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

fn configuration<'a>(
    placement: &'a PlannedGear,
    key: &str,
) -> Result<&'a conduit_core::StructuredConfigurationValue, String> {
    match placement
        .configuration
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| &entry.value)
    {
        Some(conduit_core::ConfigurationValue::Structured(value)) => Ok(value),
        _ => Err("browser keymap requires a checked authored layout".into()),
    }
}
