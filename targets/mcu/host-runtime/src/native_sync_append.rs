//! Adapter for the ordinary already bound affine record appender.
use super::*;
use conduit_resource_snapshot::{
    append::MAXIMUM_RECORD_PAYLOAD_BYTES,
    native_append::{AppendCallRefusal, RecordAppendHostCall, APPEND_RESULT_BYTES},
    nor::NorStorage,
};
fn refusal(error: AppendCallRefusal) -> CallFailure {
    match error {
        AppendCallRefusal::Capability(_)
        | AppendCallRefusal::Possession
        | AppendCallRefusal::WrongBinding
        | AppendCallRefusal::InvalidIdentity => CallFailure::Denied,
        _ => CallFailure::Failed,
    }
}
// SAFETY: this owner retains actual independent BoundBaseOperation, current
// provider/generation and finite buffers. Exact retry/uncertain custody remains
// owner-controlled; no scalar success sentinel or hidden media retry is used.
unsafe impl<S: NorStorage> SynchronousHostCall for RecordAppendHostCall<S> {
    fn node(&self) -> NodeId {
        self.node()
    }
    fn call(&self) -> HostCallId {
        HostCallId(0)
    }
    fn maximum_input_bytes(&self) -> u32 {
        MAXIMUM_RECORD_PAYLOAD_BYTES as u32
    }
    fn maximum_output_bytes(&self) -> u32 {
        APPEND_RESULT_BYTES
    }
    fn invoke(
        &mut self,
        request: HostCallRequest,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CallFailure> {
        self.invoke(request.node, request.call, request.request, input, output)
            .map_err(refusal)
    }
    fn acknowledge(&mut self, request: HostCallRequest) -> Result<(), CallFailure> {
        let (id, outcome, _) = self.pending_outcome().ok_or(CallFailure::Failed)?;
        if id != request.request {
            return Err(CallFailure::Failed);
        }
        self.acknowledge_kernel_completion(request.node, request.call, request.request, outcome)
            .map_err(refusal)
    }
    fn revoke(&mut self) -> Result<(), CallFailure> {
        self.revoke().map_err(refusal)
    }
}
