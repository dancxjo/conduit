//! Stateful portable keyboard interpretation for the ordinary browser runner.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{kind_id, HostOperationContractId, HostOperationRequirement, PlannedGear};
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeymapDisposition, KeymapRefusal};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-input-keymap@1";
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
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(kind_id("input/keymap-text-fragment")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
            maximum_output_bytes: 4,
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserOperation::unary(
        conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        1,
    ))
}

pub(crate) struct PreparedKeymap {
    state: ConduitIntlKeymap,
}

pub(crate) struct EncodedText {
    bytes: [u8; 4],
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
            state: ConduitIntlKeymap::new(),
        }))
    }

    pub(crate) fn execute(&mut self, input: &[u8]) -> Result<Option<EncodedText>, Failure> {
        let event = KeyEvent::decode(input).map_err(|_| failure(1))?;
        match self.state.apply(event) {
            KeymapDisposition::Text(text) => {
                let mut bytes = [0; 4];
                let encoded = text.as_bytes();
                bytes[..encoded.len()].copy_from_slice(encoded);
                Ok(Some(EncodedText {
                    bytes,
                    len: encoded.len(),
                }))
            }
            KeymapDisposition::NoText | KeymapDisposition::Cancelled => Ok(None),
            KeymapDisposition::Refused(reason) => Err(failure(match reason {
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
