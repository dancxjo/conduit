//! Synchronous typed Fore delivery without an escapable kernel storage handle.
use super::*;

/// Valid for this delivery only. Copy into independently admitted storage before
/// returning success; this view supplies no owned Arc or acknowledgement token.
pub struct BodyForeDeliveryRef<'a> {
    pub front_port_id: &'a conduit_core::PortId,
    pub track: conduit_core::ConnectionTrack,
    pub value_kind: &'a conduit_core::KindId,
    pub sequence: u64,
    pub bytes: &'a [u8],
}
pub trait BodyBorrowedForeOutputAdapter {
    fn deliver(&mut self, output: BodyForeDeliveryRef<'_>) -> Result<(), String>;
}
pub(super) struct BorrowedAdapter<'a>(pub &'a mut dyn BodyBorrowedForeOutputAdapter);
impl BodyForeOutputAdapter for BorrowedAdapter<'_> {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.0.deliver(BodyForeDeliveryRef {
            front_port_id: &output.front_port_id,
            track: output.track,
            value_kind: &output.value_kind,
            sequence: output.sequence,
            bytes: &output.bytes,
        })
    }
}
