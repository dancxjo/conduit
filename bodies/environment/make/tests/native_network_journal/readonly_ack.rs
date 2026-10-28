//! Explicit fixture-local seam for read-only export lease tests. This supplies
//! no ACK scheduling, admission, physical write or durable custody proof.
pub(crate) enum Writing {}
pub(crate) fn install() {}
impl crate::NativeRoot {
    pub(crate) fn advance_network_ack<T>(
        &mut self,
        _session: &mut Option<Box<T>>,
        _now: u64,
    ) -> bool {
        false
    }
}
