//! Adapter for one affine residence with independently checked append/read calls.
use super::*;
use conduit_resource_snapshot::{
    native_service::{RecordServiceHostCall, ServiceRefusal},
    nor::NorStorage,
};
fn refusal(error: ServiceRefusal) -> CallFailure {
    match error {
        ServiceRefusal::Capability(_)
        | ServiceRefusal::Possession
        | ServiceRefusal::WrongBinding
        | ServiceRefusal::InvalidInput => CallFailure::Denied,
        _ => CallFailure::Failed,
    }
}
// SAFETY: the sole prepared provider owns two immutable checked routes and
// independent opaque operations. Its invoke is synchronous and bounded with
// prepared storage, and its exact cached result controls retry/uncertain custody.
// No transport or durable-recorder acknowledgment is synthesized here.
unsafe impl<S: NorStorage> SynchronousHostCall for RecordServiceHostCall<S> {
    fn node(&self) -> NodeId {
        self.node()
    }
    fn call(&self) -> HostCallId {
        self.routes()[0].call
    }
    fn maximum_input_bytes(&self) -> u32 {
        self.routes()
            .iter()
            .map(|r| r.maximum_input_bytes)
            .max()
            .unwrap_or(0)
    }
    fn maximum_output_bytes(&self) -> u32 {
        self.routes()
            .iter()
            .map(|r| r.maximum_output_bytes)
            .max()
            .unwrap_or(0)
    }
    fn call_bounds(&self, call: HostCallId) -> Option<CallBounds> {
        self.routes()
            .iter()
            .find(|r| r.call == call)
            .map(|r| CallBounds {
                input_bytes: r.maximum_input_bytes,
                output_bytes: r.maximum_output_bytes,
            })
    }
    fn invoke(
        &mut self,
        r: HostCallRequest,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CallFailure> {
        self.invoke(r.node, r.call, r.request, input, output)
            .map_err(refusal)
    }
    fn acknowledge(&mut self, _: HostCallRequest) -> Result<(), CallFailure> {
        // A service requires the kernel-accepted bytes, not a scalar success.
        Err(CallFailure::Denied)
    }
    fn acknowledge_result(&mut self, r: HostCallRequest, output: &[u8]) -> Result<(), CallFailure> {
        self.acknowledge_kernel_completion(r.node, r.call, r.request, output)
            .map_err(refusal)
    }
    fn revoke(&mut self) -> Result<(), CallFailure> {
        self.revoke().map_err(refusal)
    }
}
