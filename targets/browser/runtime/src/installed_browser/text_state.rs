//! Bounded retained text editing and repeated line submission.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{kind_id, ConfigurationValue, HostOperationRequirement, PlannedGear};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-text-state@1";
const EDIT_IMPLEMENTATION: &str = "browser/kernel-text-edit@1";
const SUBMIT_IMPLEMENTATION: &str = "browser/kernel-text-submit-lines@1";

pub(super) static EDIT: BrowserInstallation = BrowserInstallation {
    implementation_id: EDIT_IMPLEMENTATION,
    offer: edit_offer,
    prepare,
    perform: None,
};
pub(super) static SUBMIT: BrowserInstallation = BrowserInstallation {
    implementation_id: SUBMIT_IMPLEMENTATION,
    offer: submit_offer,
    prepare,
    perform: None,
};

fn edit_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::text_edit_contract(),
        conduit_semantic_catalog::TEXT_EDIT_REVISION,
        EDIT_IMPLEMENTATION,
    )
}
fn submit_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::text_submit_lines_contract(),
        conduit_semantic_catalog::TEXT_SUBMIT_LINES_REVISION,
        SUBMIT_IMPLEMENTATION,
    )
}
fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    revision: &str,
    implementation: &'static str,
) -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        contract,
        revision,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: implementation,
            implementation,
            artifact: "conduit-browser-runtime/text-state@1",
        },
        vec![HostOperationRequirement {
            contract_id: HOST_OPERATION.into(),
            target_kind: Some(kind_id("text/bounded-state-output@1")),
            maximum_in_flight: 1,
            maximum_input_bytes: 4,
            maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    let expected = match placement.implementation_id.as_str() {
        EDIT_IMPLEMENTATION => edit_offer(),
        SUBMIT_IMPLEMENTATION => submit_offer(),
        _ => return Err("unsupported text state implementation".into()),
    };
    validate_placement(placement, &expected)?;
    Ok(BrowserOperation::unary(4, 1))
}

pub(crate) struct PreparedTextState {
    mode: Mode,
    text: Vec<u8>,
    pending_clear: bool,
}

#[derive(Clone, Copy)]
enum Mode {
    Edit,
    Submit,
}

impl PreparedTextState {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        let mode = match placement.implementation_id.as_str() {
            EDIT_IMPLEMENTATION => Mode::Edit,
            SUBMIT_IMPLEMENTATION => Mode::Submit,
            _ => return Ok(None),
        };
        let expected = match mode {
            Mode::Edit => edit_offer(),
            Mode::Submit => submit_offer(),
        };
        validate_placement(placement, &expected)?;
        let maximum = placement
            .configuration
            .iter()
            .find_map(|entry| match (entry.key.as_str(), &entry.value) {
                ("maximum-bytes", ConfigurationValue::U64(value)) => usize::try_from(*value).ok(),
                _ => None,
            })
            .ok_or("text state maximum-bytes is absent")?;
        Ok(Some(Self {
            mode,
            text: Vec::with_capacity(maximum),
            pending_clear: false,
        }))
    }

    pub(crate) fn execute(&mut self, fragment: &[u8]) -> Result<Option<&[u8]>, Failure> {
        if self.pending_clear {
            self.text.clear();
            self.pending_clear = false;
        }
        let fragment =
            core::str::from_utf8(fragment).map_err(|_| failure(FailureCode::InvalidInput, 1))?;
        if fragment == "\n" {
            return match self.mode {
                Mode::Edit => Ok(Some(&self.text)),
                Mode::Submit if self.text.is_empty() => Ok(None),
                Mode::Submit => {
                    self.pending_clear = true;
                    Ok(Some(&self.text))
                }
            };
        }
        if fragment == "\u{8}" {
            if let Some((index, _)) = core::str::from_utf8(&self.text)
                .ok()
                .and_then(|text| text.char_indices().next_back())
            {
                self.text.truncate(index);
            }
            return Ok(matches!(self.mode, Mode::Edit).then_some(&self.text));
        }
        if self
            .text
            .len()
            .checked_add(fragment.len())
            .is_none_or(|length| length > self.text.capacity())
        {
            return Err(failure(FailureCode::StateCapacityExhausted, 2));
        }
        self.text.extend_from_slice(fragment.as_bytes());
        Ok(matches!(self.mode, Mode::Edit).then_some(&self.text))
    }
}

const fn failure(code: FailureCode, detail: u16) -> Failure {
    Failure { code, detail }
}
