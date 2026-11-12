//! One already bound I2C operation through the existing synchronous service.
use super::*;
use conduit_i2c_transaction::{
    contract::{I2C_MAXIMUM_BYTES, I2C_RESULT_VALUE_BYTES},
    owner::{I2cCallRefusal, I2cHostCall},
    I2cProvider,
};
fn refusal(error: I2cCallRefusal) -> CallFailure {
    match error {
        I2cCallRefusal::Capability(_)
        | I2cCallRefusal::Possession
        | I2cCallRefusal::WrongBinding => CallFailure::Denied,
        _ => CallFailure::Failed,
    }
}
// SAFETY: the affine native owner retains exact selected possession, bounded
// codecs and a synchronous provider. Service preflights storage and Signs before
// invocation. Bus outcomes are typed values, not a success sentinel or retry.
unsafe impl<P: I2cProvider> SynchronousHostCall for I2cHostCall<P> {
    fn node(&self) -> NodeId {
        self.node()
    }
    fn call(&self) -> HostCallId {
        HostCallId(0)
    }
    fn maximum_input_bytes(&self) -> u32 {
        I2C_MAXIMUM_BYTES
    }
    fn maximum_output_bytes(&self) -> u32 {
        I2C_RESULT_VALUE_BYTES as u32
    }
    fn invoke(
        &mut self,
        request: HostCallRequest,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CallFailure> {
        // Refuse a short independently funded buffer before any bus effect.
        if output.len() < I2C_RESULT_VALUE_BYTES {
            return Err(CallFailure::Failed);
        }
        let bytes = self
            .invoke(request.node, request.call, request.request, input)
            .map_err(refusal)?;
        let destination = output.get_mut(..bytes.len()).ok_or(CallFailure::Failed)?;
        destination.copy_from_slice(bytes);
        Ok(bytes.len())
    }
    fn acknowledge(&mut self, request: HostCallRequest) -> Result<(), CallFailure> {
        // This synchronous transaction has no deferred durable custody to free.
        // Service alone invokes this hook after accepting the exact completion.
        // It grants no new call and does not retry, reset or acknowledge history.
        if request.node != self.node() || request.call != HostCallId(0) {
            return Err(CallFailure::Denied);
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<(), CallFailure> {
        self.finish_idle().map_err(|_| CallFailure::Failed)
    }
    fn revoke(&mut self) -> Result<(), CallFailure> {
        self.revoke(self.node(), HostCallId(0)).map_err(refusal)
    }
}
