//! Selected finite scan ownership; no ambient radio access or policy renewal.
mod owner;
use conduit_core::{Kind, KindId};
use conduit_net::*;
use conduit_plan_lowering::selected_operation::SelectedOperationContract;
pub use owner::*;
/// Root must independently admit the exclusive initialized controller, SDK
/// memory and service transitions. A cancelled future must leave persistent
/// completion/stop custody in this provider. Cleanup may return success only
/// after actual terminal reconciliation, list release and AP restoration.
/// Deadline expiry is refusal, never evidence of quiescence. Unknown suites
/// must remain Unknown and storage pressure must not be hidden as truncation.
pub trait NetworkScanProvider {
    fn now_ms(&self) -> u64;
    fn scan(
        &mut self,
        request: NetworkScanRequest,
        output: &mut [ScanObservation; 16],
    ) -> impl core::future::Future<Output = Result<ScanCompletion, ScanFailure>>;
    fn cleanup(
        &mut self,
        absolute_deadline_ms: u64,
    ) -> impl core::future::Future<Output = Result<(), ScanFailure>>;
}
pub struct PreparedScanContract {
    kind: Kind,
    subject: KindId,
}
impl Default for PreparedScanContract {
    fn default() -> Self {
        Self::prepare()
    }
}
impl PreparedScanContract {
    pub fn prepare() -> Self {
        Self {
            kind: network_scan_contract(),
            subject: NETWORK_SCAN_SUBJECT.into(),
        }
    }
    pub fn kind(&self) -> &Kind {
        &self.kind
    }
    fn definition(&self) -> SelectedOperationContract<'_> {
        SelectedOperationContract {
            kind: &self.kind,
            call: NETWORK_SCAN_HOST_CALL,
            input_bytes: MAXIMUM_SCAN_INPUT_BYTES,
            output_bytes: MAXIMUM_SCAN_OUTPUT_BYTES,
            resource_bytes: 1,
        }
    }
}
