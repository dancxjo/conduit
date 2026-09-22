//! Bounded retained text editing and repeated line submission.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostCallRequirement, ImplementationId, Kind, PlannedGear,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_CALL: &str = "conduit.host/browser-text-state@1";
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
                maximum_input_bytes: 4,
                maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserBack, String> {
    let expected = match placement.implementation_id.as_str() {
        EDIT_IMPLEMENTATION => edit_offer(),
        SUBMIT_IMPLEMENTATION => submit_offer(),
        _ => return Err("unsupported text state implementation".into()),
    };
    validate_placement(placement, &expected)?;
    Ok(BrowserBack::unary(4, 1))
}

pub(crate) struct PreparedTextState(conduit_semantic_catalog::BoundedTextState);

impl PreparedTextState {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        let mode = match placement.implementation_id.as_str() {
            EDIT_IMPLEMENTATION => conduit_semantic_catalog::TextStateMode::Edit,
            SUBMIT_IMPLEMENTATION => conduit_semantic_catalog::TextStateMode::Submit,
            _ => return Ok(None),
        };
        let expected = match mode {
            conduit_semantic_catalog::TextStateMode::Edit => edit_offer(),
            conduit_semantic_catalog::TextStateMode::Submit => submit_offer(),
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
            .map(Self)
            .map(Some)
            .map_err(|_| "text state maximum-bytes is outside the portable bound".into())
    }

    pub(crate) fn execute(&mut self, fragment: &[u8]) -> Result<Option<&[u8]>, Failure> {
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
