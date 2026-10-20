//! Owned compatibility descriptions and borrowed views of admitted Host work.
use super::InstalledRemoteFragment;
use conduit_core::HostCallContractId;
use conduit_kernel::scheduler::HostCallRequest;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteHostWork {
    pub request: HostCallRequest,
    pub contract_id: HostCallContractId,
    pub maximum_output_bytes: u32,
    pub input: Vec<u8>,
}

/// An immutable view into the prepared contract and retained kernel input.
/// The runtime cannot advance or complete the request while this borrow is live.
#[derive(Clone, Copy, Debug)]
pub struct RemoteHostWorkRef<'a> {
    pub request: HostCallRequest,
    pub contract_id: &'a HostCallContractId,
    pub maximum_output_bytes: u32,
    pub input: &'a [u8],
}

impl InstalledRemoteFragment {
    pub fn host_request_view(
        &self,
        request: HostCallRequest,
    ) -> Result<RemoteHostWorkRef<'_>, String> {
        let operation = self
            .lowered
            .host_calls
            .iter()
            .find(|operation| operation.node == request.node && operation.call == request.call)
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote std host input: {error:?}"))?;
        Ok(RemoteHostWorkRef {
            request,
            contract_id: &operation.contract_id,
            maximum_output_bytes: operation.binding.maximum_output_bytes,
            input,
        })
    }

    /// Keep the owned API for callers that need to retain a description.
    pub fn describe_host_request(
        &self,
        request: HostCallRequest,
    ) -> Result<RemoteHostWork, String> {
        let view = self.host_request_view(request)?;
        Ok(RemoteHostWork {
            request: view.request,
            contract_id: view.contract_id.clone(),
            maximum_output_bytes: view.maximum_output_bytes,
            input: view.input.to_vec(),
        })
    }
}
