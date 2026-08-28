use crate::MAX_BODY_PARTS;

use super::{
    validate_identity, HostPresenceEventKind, HostPresenceRefusal, HostPresenceState,
    HostPresenceTable, MAX_PRESENCE_EVENTS, MAX_PRESENCE_LEASE_MILLIS,
};

impl HostPresenceTable {
    pub fn validate(&self) -> Result<(), HostPresenceRefusal> {
        validate_identity(self.body_id.as_str())?;
        validate_identity(&self.clock.basis_id)?;
        if self.clock.resolution_ticks == 0 {
            return Err(HostPresenceRefusal::InvalidClock);
        }
        if self.maximum_lease_millis == 0
            || self.maximum_lease_millis > MAX_PRESENCE_LEASE_MILLIS
            || self.leases.len() > MAX_BODY_PARTS
            || self.events.len() > MAX_PRESENCE_EVENTS
            || self
                .dropped_event_count
                .checked_add(self.events.len() as u64)
                != Some(self.revision)
        {
            return Err(HostPresenceRefusal::MalformedState);
        }
        for (index, lease) in self.leases.iter().enumerate() {
            validate_identity(lease.part_id.as_str())?;
            validate_identity(lease.host_id.as_str())?;
            validate_identity(lease.boot_id.as_str())?;
            validate_identity(lease.membership_proof_id.as_str())?;
            validate_identity(lease.session_binding_id.as_str())?;
            if lease.sequence == 0
                || lease.observed_at_millis > lease.expires_at_millis
                || lease.expires_at_millis - lease.observed_at_millis > self.maximum_lease_millis
                || self.leases[..index]
                    .iter()
                    .any(|prior| prior.part_id == lease.part_id)
            {
                return Err(HostPresenceRefusal::MalformedState);
            }
            let latest = self
                .events
                .iter()
                .rev()
                .find(|event| event.part_id == lease.part_id)
                .ok_or(HostPresenceRefusal::MalformedState)?;
            let event_available = matches!(
                latest.kind,
                HostPresenceEventKind::Started | HostPresenceEventKind::Renewed
            );
            if latest.host_id != lease.host_id
                || latest.boot_id != lease.boot_id
                || latest.offer_generation != lease.offer_generation
                || latest.membership_proof_id != lease.membership_proof_id
                || latest.session_binding_id != lease.session_binding_id
                || latest.sequence != lease.sequence
                || event_available != (lease.state == HostPresenceState::Available)
            {
                return Err(HostPresenceRefusal::MalformedState);
            }
        }
        for (index, event) in self.events.iter().enumerate() {
            validate_identity(event.part_id.as_str())?;
            validate_identity(event.host_id.as_str())?;
            validate_identity(event.boot_id.as_str())?;
            validate_identity(event.membership_proof_id.as_str())?;
            validate_identity(event.session_binding_id.as_str())?;
            validate_identity(event.sign_id.as_str())?;
            if event.revision != self.dropped_event_count + index as u64 + 1
                || event.sequence == 0
                || (!matches!(event.kind, HostPresenceEventKind::Expired)
                    && (event.observed_at_millis > event.expires_at_millis
                        || event.expires_at_millis - event.observed_at_millis
                            > self.maximum_lease_millis))
            {
                return Err(HostPresenceRefusal::MalformedState);
            }
        }
        Ok(())
    }
}
