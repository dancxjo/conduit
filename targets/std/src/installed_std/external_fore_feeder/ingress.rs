use super::*;
pub(in crate::installed_std) trait ForeIngress {
    fn has_ready_work(&self) -> bool {
        false
    }

    fn admit(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String>;
    fn close(&mut self, endpoint: RemoteEndpointId, cord: CordId) -> Result<(), String>;
}

impl ForeIngress for InstalledScheduler {
    fn has_ready_work(&self) -> bool {
        InstalledScheduler::has_ready_work(self)
    }

    fn admit(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        self.admit_remote_input_fanout(targets, sequence, bytes)
            .map_err(|error| format!("admit external Fore sequence: {error:?}"))
    }

    fn close(&mut self, endpoint: RemoteEndpointId, cord: CordId) -> Result<(), String> {
        self.close_remote_input(endpoint, cord)
            .map_err(|error| format!("close external Fore sequence: {error:?}"))
    }
}
