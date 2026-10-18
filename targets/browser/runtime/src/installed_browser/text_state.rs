//! Bounded retained text editing and repeated line submission.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostCallRequirement, ImplementationId, Kind, PlannedGear,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore, ValueStorage};

pub(crate) const HOST_CALL: &str = "conduit.host/browser-text-state@1";
const EDIT_IMPLEMENTATION: &str = "browser/kernel-text-edit@1";
const DOCUMENT_IMPLEMENTATION: &str = "browser/kernel-text-edit-document@1";
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

pub(super) static DOCUMENT: BrowserInstallation = BrowserInstallation {
    implementation_id: DOCUMENT_IMPLEMENTATION,
    offer: document_offer,
    prepare,
    perform: None,
};
fn document_offer() -> CapabilityOffer {
    let mut result = offer(
        conduit_semantic_catalog::text_edit_document_semantic_contract(),
        DOCUMENT_IMPLEMENTATION,
    );
    result.host_calls[0].maximum_input_bytes = conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES;
    result.host_calls[0].maximum_output_bytes =
        conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES;
    result
}
fn edit_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::text_edit_semantic_contract(),
        EDIT_IMPLEMENTATION,
    )
}
fn submit_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::text_submit_lines_semantic_contract(),
        SUBMIT_IMPLEMENTATION,
    )
}
fn offer(contract: Kind, implementation: &'static str) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-browser-runtime/text-state@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HOST_CALL.into(),
                target_kind: Some(kind_id("text/bounded-state-output@1")),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
                maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn prepare(placement: &PlannedGear, values: &mut HostedValueStore) -> Result<BrowserBack, String> {
    let expected = match placement.implementation_id.as_str() {
        DOCUMENT_IMPLEMENTATION => document_offer(),
        EDIT_IMPLEMENTATION => edit_offer(),
        SUBMIT_IMPLEMENTATION => submit_offer(),
        _ => return Err("unsupported text state implementation".into()),
    };
    validate_placement(placement, &expected)?;
    if placement.implementation_id.as_str() == DOCUMENT_IMPLEMENTATION {
        let initial = values
            .store(&[0; 8])
            .map_err(|e| format!("initial document: {e:?}"))?;
        Ok(BrowserBack::unary_with_initial(
            conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES,
            initial,
        ))
    } else if placement.implementation_id.as_str() == EDIT_IMPLEMENTATION {
        let initial = values
            .store(&[])
            .map_err(|error| format!("store initial empty editor Current: {error:?}"))?;
        Ok(BrowserBack::unary_with_initial(
            conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
            initial,
        ))
    } else {
        Ok(BrowserBack::unary(
            conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
            1,
        ))
    }
}

pub(crate) struct PreparedTextState(conduit_semantic_catalog::BoundedTextState, bool, [u8; 264]);

impl PreparedTextState {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        let mode = match placement.implementation_id.as_str() {
            DOCUMENT_IMPLEMENTATION | EDIT_IMPLEMENTATION => {
                conduit_semantic_catalog::TextStateMode::Edit
            }
            SUBMIT_IMPLEMENTATION => conduit_semantic_catalog::TextStateMode::Submit,
            _ => return Ok(None),
        };
        let expected = if placement.implementation_id.as_str() == DOCUMENT_IMPLEMENTATION {
            document_offer()
        } else {
            match mode {
                conduit_semantic_catalog::TextStateMode::Edit => edit_offer(),
                conduit_semantic_catalog::TextStateMode::Submit => submit_offer(),
            }
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
        conduit_semantic_catalog::BoundedTextState::new(mode, maximum)
            .map(|state| {
                Self(
                    state,
                    placement.implementation_id.as_str() == DOCUMENT_IMPLEMENTATION,
                    [0; 264],
                )
            })
            .map(Some)
            .map_err(|_| "text state maximum-bytes is outside the portable bound".into())
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> (u64, &[u8]) {
        (self.0.revision(), self.0.current())
    }

    pub(crate) fn execute(&mut self, fragment: &[u8]) -> Result<Option<&[u8]>, Failure> {
        if self.1 {
            let edit =
                conduit_semantic_catalog::TextEdit::decode(fragment).map_err(edit_failure)?;
            self.0.replace(edit).map_err(edit_failure)?;
            let document = conduit_semantic_catalog::TextDocument {
                revision: self.0.revision(),
                text: self.0.current(),
            };
            let len = document
                .encode_into(&mut self.2)
                .map_err(|_| failure(FailureCode::InvalidInput, 4))?;
            return Ok(Some(&self.2[..len]));
        }
        self.0.apply(fragment).map_err(|refusal| match refusal {
            conduit_semantic_catalog::TextStateRefusal::CapacityExhausted => {
                failure(FailureCode::StateCapacityExhausted, 2)
            }
            conduit_semantic_catalog::TextStateRefusal::InvalidCapacity
            | conduit_semantic_catalog::TextStateRefusal::InvalidUtf8 => {
                failure(FailureCode::InvalidInput, 1)
            }
        })
    }
}

const fn failure(code: FailureCode, detail: u16) -> Failure {
    Failure { code, detail }
}

fn edit_failure(refusal: conduit_semantic_catalog::TextEditRefusal) -> Failure {
    use conduit_semantic_catalog::TextEditRefusal::*;
    match refusal {
        CapacityExhausted => failure(FailureCode::StateCapacityExhausted, 10),
        RevisionExhausted => failure(FailureCode::IdentityCapacityExhausted, 11),
        StaleRevision => failure(FailureCode::InvalidInput, 12),
        InvalidRange => failure(FailureCode::InvalidInput, 13),
        InvalidUtf8 => failure(FailureCode::InvalidInput, 14),
        InvalidEncoding => failure(FailureCode::InvalidInput, 15),
        InvalidStorage | UnsupportedMode => failure(FailureCode::InvalidInput, 16),
    }
}
