//! A prepared pure transition over canonical document/edit Info. The calling
//! Plot owns the current value and adopts only an acknowledged successful result.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{MAXIMUM_TEXT_DOCUMENT_BYTES, MAXIMUM_TEXT_EDIT_BYTES};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TEXT_APPLY_EDIT_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TextApplyEditBack {
    output: Vec<u8>,
    length: usize,
    staged: bool,
    emitted: bool,
}
impl TextApplyEditBack {
    pub(super) fn allocation_capacity(&self) -> usize {
        self.output.capacity()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for TextApplyEditBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            if io.input_closed(PortId(0)) && io.input_closed(PortId(1)) {
                io.consume_closed(PortId(0)).expect("closed document");
                io.consume_closed(PortId(1)).expect("closed edit");
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        if io.input(PortId(0)).is_none() || io.input(PortId(1)).is_none() {
            return if io.input_closed(PortId(0)) || io.input_closed(PortId(1)) {
                StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 5,
                })
            } else {
                StepOutcome::Await
            };
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(document), Some(edit)) = (bytes.input(PortId(0)), bytes.input(PortId(1))) else {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 5,
            });
        };
        self.length =
            match conduit_semantic_catalog::apply_document_edit(document, edit, &mut self.output) {
                Ok(length) => length,
                Err(refusal) => {
                    return StepOutcome::Fail(super::text_state_back::edit_failure(refusal))
                }
            };
        io.consume(PortId(0)).expect("present document");
        io.consume(PortId(1)).expect("present edit");
        io.send_prepared(PortId(0), self.length as u32)
            .expect("admitted candidate document");
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(&self.output[..self.length])
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.emitted = true;
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
    }
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let offer = conduit_std_offers::text_apply_edit_offer();
    if placement.kind_id != offer.kind_id
        || placement.semantic_contract != offer.semantic_contract
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.limits != offer.limits
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.configuration.is_empty()
    {
        return Err("text apply-edit differs from its exact pure Back".into());
    }
    Ok(BackBudget {
        value_items: 3,
        value_bytes: 2 * MAXIMUM_TEXT_DOCUMENT_BYTES + MAXIMUM_TEXT_EDIT_BYTES,
        host_requests: 0,
        sign_items: 12,
        maximum_value_bytes: MAXIMUM_TEXT_EDIT_BYTES,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(placement)?;
    Ok(InstalledBack::TextApplyEdit(TextApplyEditBack {
        output: vec![0; MAXIMUM_TEXT_DOCUMENT_BYTES as usize],
        length: 0,
        staged: false,
        emitted: false,
    }))
}
