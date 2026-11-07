//! Immediate execution faults retain typed ownership; formatting belongs to the
//! caller's admitted diagnostic slot. Worker submission is a separate profile.
use super::*;
#[derive(Debug)]
pub(crate) enum ImmediateReadFailure {
    Request,
    Sequence,
    NotImmediate,
    Authorization(BaseCapabilityRefusal),
    Completion(BaseCapabilityRefusal),
    Revoke(BaseCapabilityRefusal),
    ResultEnvelope,
    Provider(String),
}
impl core::fmt::Display for ImmediateReadFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Authorization(e) => write!(f, "native Source authorization: {e:?}"),
            Self::Completion(e) => write!(f, "native Source completion: {e:?}"),
            Self::Revoke(e) => write!(f, "native Source revoke: {e:?}"),
            Self::Provider(e) => write!(f, "native Source provider: {e}"),
            Self::Request => f.write_str("native Source request differs from selected call"),
            Self::Sequence => f.write_str("native Source request sequence exhausted"),
            Self::NotImmediate => f.write_str("native Source is not an immediate provider"),
            Self::ResultEnvelope => f.write_str("native Source exceeded its fixed result slot"),
        }
    }
}
impl PreparedNativeSource<'_> {
    pub(crate) fn read_immediate(
        &mut self,
        request: conduit_kernel::scheduler::HostCallRequest,
        input: &[u8],
    ) -> Result<&[u8], ImmediateReadFailure> {
        if self.cancelled
            || self.pending.is_some()
            || request.node != self.node
            || request.call != conduit_kernel::HostCallId(0)
            || request.request != conduit_kernel::RequestId(self.next_request)
            || input.len() > self.operation.inspection().scope.maximum_parameter_bytes as usize
        {
            return Err(ImmediateReadFailure::Request);
        }
        let NativeProvider::Immediate(provider) = self.provider else {
            return Err(ImmediateReadFailure::NotImmediate);
        };
        // Refuse exhaustion before acquiring a lease, so a failed sequence
        // transition cannot strand an authorized operation.
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(ImmediateReadFailure::Sequence)?;
        let lease = self
            .operation
            .authorize()
            .map_err(ImmediateReadFailure::Authorization)?;
        self.next_request = next;
        let result = provider.read(input, &mut self.output);
        let bytes = result.as_ref().copied().unwrap_or(0);
        let accepted = if bytes <= self.output.len() { bytes } else { 0 };
        self.operation
            .complete(lease, accepted as u32)
            .map_err(ImmediateReadFailure::Completion)?;
        if bytes > self.output.len() {
            self.operation
                .revoke()
                .map_err(ImmediateReadFailure::Revoke)?;
            return Err(ImmediateReadFailure::ResultEnvelope);
        }
        let bytes = result.map_err(ImmediateReadFailure::Provider)?;
        Ok(&self.output[..bytes])
    }
}
#[cfg(test)]
#[path = "immediate/tests.rs"]
mod tests;
