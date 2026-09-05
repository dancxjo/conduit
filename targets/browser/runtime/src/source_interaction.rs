//! Typed, bounded authoring interaction for one editable inline Form.

use conduit_core::KindId;
use conduit_human::{
    HumanInteractionProposal, InteractionApplicationOutcome, InteractionContract,
    InteractionCurrentState, InteractionFamily, InteractionProposalPayload, InteractionValue,
    TypedInteractionFlow,
};
use serde::{Deserialize, Serialize};

pub(super) const SOURCE_INTERACTION_MAXIMUM_BYTES: u32 = 8 * 1_024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SourceInteractionEvidence {
    pub schema: String,
    pub disposition: String,
    pub semantic_id: String,
    pub contract_identity: String,
    pub state_identity: String,
    pub proposal_identity: String,
    pub result_identity: String,
    pub sequence: u64,
    pub value_kind: String,
    pub value_bytes: u32,
}

pub(super) fn admit_source(
    source: &[u8],
    sequence: u64,
) -> Result<SourceInteractionEvidence, String> {
    let contract = source_contract()?;
    let current = source_state(&contract)?;
    let value = InteractionValue::new(KindId::from(conduit_human::TEXT_INFO_ID), source.to_vec())
        .map_err(debug_error)?;
    let proposal = HumanInteractionProposal::new(
        &contract,
        &current,
        sequence,
        InteractionProposalPayload::Values(vec![value]),
    )
    .map_err(debug_error)?;
    let mut flow = TypedInteractionFlow::new(contract.clone(), current.clone(), None, 1, 1)
        .map_err(debug_error)?;
    flow.admit(proposal.clone()).map_err(debug_error)?;
    let result = flow
        .finish_front(InteractionApplicationOutcome::Accepted {
            resulting_state_identity: current.state_identity.clone(),
        })
        .map_err(debug_error)?;
    Ok(SourceInteractionEvidence {
        schema: "conduit.tour/source-interaction@1".into(),
        disposition: "accepted".into(),
        semantic_id: "interaction/executable-tour-source".into(),
        contract_identity: contract.contract_identity,
        state_identity: current.state_identity,
        proposal_identity: proposal.proposal_identity,
        result_identity: result.result_identity,
        sequence,
        value_kind: conduit_human::TEXT_INFO_ID.into(),
        value_bytes: u32::try_from(source.len()).map_err(|_| "source byte bound overflow")?,
    })
}

fn source_contract() -> Result<InteractionContract, String> {
    InteractionContract::new(
        "interaction/executable-tour-source",
        InteractionFamily::Text {
            maximum_bytes: SOURCE_INTERACTION_MAXIMUM_BYTES,
            allow_empty: false,
        },
    )
    .map_err(debug_error)
}

fn source_state(contract: &InteractionContract) -> Result<InteractionCurrentState, String> {
    InteractionCurrentState::new(contract, 0, None, Vec::new()).map_err(debug_error)
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("source interaction refused: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_human::{InteractionRefusal, TypedInteractionFlow};

    #[test]
    fn accepted_evidence_retains_bounds_and_identities_but_not_plaintext() {
        let source = b"form secret { value: text/literal(\"do not retain\") }";
        let evidence = admit_source(source, 7).unwrap();
        assert_eq!(evidence.disposition, "accepted");
        assert_eq!(evidence.sequence, 7);
        assert_eq!(evidence.value_bytes, source.len() as u32);
        assert!(!format!("{evidence:?}").contains("do not retain"));
        assert!(evidence.contract_identity.len() > 8);
        assert!(evidence.proposal_identity.len() > 8);
        assert!(evidence.result_identity.len() > 8);
    }

    #[test]
    fn empty_oversized_duplicate_pressure_and_cancellation_remain_distinct() {
        assert!(admit_source(b"", 0).unwrap_err().contains("MalformedValue"));
        assert!(admit_source(
            &vec![b'x'; SOURCE_INTERACTION_MAXIMUM_BYTES as usize + 1],
            0
        )
        .unwrap_err()
        .contains("ValueBoundExceeded"));

        let contract = source_contract().unwrap();
        let current = source_state(&contract).unwrap();
        let proposal = HumanInteractionProposal::new(
            &contract,
            &current,
            1,
            InteractionProposalPayload::Values(vec![InteractionValue::new(
                KindId::from(conduit_human::TEXT_INFO_ID),
                b"one".to_vec(),
            )
            .unwrap()]),
        )
        .unwrap();
        let next = HumanInteractionProposal::new(
            &contract,
            &current,
            2,
            InteractionProposalPayload::Values(vec![InteractionValue::new(
                KindId::from(conduit_human::TEXT_INFO_ID),
                b"two".to_vec(),
            )
            .unwrap()]),
        )
        .unwrap();
        let mut flow = TypedInteractionFlow::new(contract, current, None, 1, 1).unwrap();
        flow.admit(proposal.clone()).unwrap();
        assert_eq!(
            flow.admit(proposal),
            Err(InteractionRefusal::DuplicateProposal)
        );
        assert_eq!(flow.admit(next), Err(InteractionRefusal::QueuePressure));
        assert_eq!(
            flow.cancel_front().unwrap().outcome,
            InteractionApplicationOutcome::Cancelled
        );
    }
}
