//! CPU-only identity/secret fixture; never compiled into device provisioning.
use super::*;
impl RootIssuer {
    pub fn fixture(host: c::HostId, boot: c::BootId) -> Self {
        Self {
            host,
            boot,
            key: [7; 32],
            issued: [false; 2],
            wait_issued: false,
            pulse_issued: false,
            display_issued: false,
            reserved_display_play: None,
            issued_display_play: None,
            display_generation: 0,
            pulse_generation: 0,
            wait_generation: 0,
            last_play_sequence: 0,
            reserved_play: None,
            native_issued_play: None,
        }
    }
}

impl RootIssuer {
    /// Test a refusal after the first actual selected operation was issued.
    pub(crate) fn refuse_join_fixture(&mut self) {
        self.issued[1] = true;
    }
}
