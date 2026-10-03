//! Register binding uses the shared exact selected-operation verifier.
use super::{
    REGISTER_CALL, RegisterLeaf, RegisterRefusal,
    register_call::{REGISTER_REQUEST_BYTES, REGISTER_RESULT_BYTES, RegisterHostCall, contract},
    selected_operation::{
        SelectedCallRefusal, SelectedOperationContract, SelectedOperationPlan,
        bind_selected_operation,
    },
};
use conduit_core::{ActivePlayIdentity, PlacementId, PlanFragment};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

impl RegisterHostCall {
    /// Bind existing opaque possession to the exact ordinary Plan lowering.
    pub fn bind_selected(
        leaf: RegisterLeaf,
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
        placement_id: &PlacementId,
    ) -> Result<Self, RegisterRefusal> {
        let expected = contract();
        let node = bind_selected_operation(
            &leaf.table,
            &leaf.claim,
            SelectedOperationContract {
                kind: &expected,
                call: REGISTER_CALL,
                input_bytes: REGISTER_REQUEST_BYTES,
                output_bytes: REGISTER_RESULT_BYTES,
                resource_bytes: u64::from(leaf.envelope.bytes),
            },
            SelectedOperationPlan {
                fragment,
                lowered,
                active,
                placement_id,
            },
        )
        .map_err(|refusal| match refusal {
            SelectedCallRefusal::WrongBinding => RegisterRefusal::WrongBinding,
            SelectedCallRefusal::Possession => RegisterRefusal::Possession,
        })?;
        Ok(Self { node, leaf })
    }
}
