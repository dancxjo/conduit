//! Permission projection for Root's actual retained display; no bearer issuance.
use super::*;
use conduit_tm1637_wire::{installation::*, WRITE_CALL, WRITE_KIND};
impl RootIssuer {
    pub fn grant_display(&self, native: &WriteNativeIdentity) -> Result<c::AuthorityGrant, ()> {
        if native.host_id != self.host || native.boot_id != self.boot {
            return Err(());
        }
        Ok(c::AuthorityGrant {
            grant_id: "grant/esp32/tm1637-write/1".into(),
            contract_id: WRITE_AUTHORITY.into(),
            host_call_contract_id: WRITE_CALL.into(),
            subject_kind: WRITE_KIND.into(),
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            capability_id: WRITE_CAPABILITY.into(),
        })
    }
}
