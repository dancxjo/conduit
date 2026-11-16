//! Fixed-capacity presence attached to its original authenticated viewer lane.
use super::*;
impl Owner {
    /// The protected carrier worker may renew reachability only for its exact
    /// authenticated incarnation. This grants no Line, Mask or effect authority.
    /// A missed short lease requires fresh carrier authentication; a late packet
    /// cannot resurrect it merely because the overall session has time remaining.
    pub(crate) fn renew_current_member_presence(
        &mut self,
        session_id: &conduit_core::LinkBindingId,
        sequence: u64,
    ) -> Result<conduit_body::HostPresenceLease, String> {
        self.select_native_viewer_session(session_id)?;
        let inspection = self.inspect_current_member_carrier(session_id)?;
        if sequence == 0 || sequence > u64::from(inspection.maximum_frames) {
            return Err("member presence sequence exceeds carrier bounds".into());
        }
        let active = self
            .member_carrier
            .as_mut()
            .and_then(|custody| custody.active.as_mut())
            .ok_or("carrier session absent")?;
        let remaining = active
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        let lease_millis = PRESENCE_LEASE_MILLIS.min(remaining.min(u128::from(u64::MAX)) as u64);
        if lease_millis == 0 {
            return Err("member presence carrier expired".into());
        }
        let membership = &self.session.evidence().membership;
        let part = &active.member.credential.part_id;
        let current = membership
            .parts
            .iter()
            .find(|entry| &entry.part_id == part)
            .and_then(|entry| entry.current.as_ref())
            .ok_or("presence current member absent")?;
        if current.proof_id.retained_backing_bytes() > active.presence_proof_capacity {
            return Err("presence proof backing changed; fresh carrier required".into());
        }
        let mut sign = String::with_capacity(carrier_bound::PRESENCE_SIGN_BYTES);
        use std::fmt::Write;
        write!(&mut sign, "{}/presence/{sequence}", session_id.as_str())
            .map_err(|_| "member presence sign formatting failed")?;
        let sign = conduit_core::SignId::from(sign);
        if let Some(presence) = active.presence.as_mut() {
            let now = presence
                .clock
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64;
            if presence
                .table
                .leases
                .first()
                .is_none_or(|lease| now >= lease.expires_at_millis)
            {
                return Err("member presence lease expired".into());
            }
            presence
                .table
                .renew(
                    membership,
                    part,
                    session_id,
                    sequence,
                    now,
                    lease_millis,
                    sign,
                )
                .map_err(|error| format!("member presence renewal: {error:?}"))?;
        } else {
            if sequence != 1 {
                return Err("member presence must start at sequence one".into());
            }
            let clock = conduit_body::HostPresenceClock::new(
                session_id.as_str().into(),
                conduit_body::HostPresenceClockScale::Milliseconds,
                1,
                1,
            )
            .map_err(|error| format!("member presence clock: {error:?}"))?;
            let mut table = conduit_body::HostPresenceTable::new(
                membership.body_id.clone(),
                clock,
                PRESENCE_LEASE_MILLIS,
            )
            .map_err(|error| format!("member presence table: {error:?}"))?;
            // Exact capacities are funded at original carrier admission. This
            // table serves only its one original Part and never grows its Vecs.
            table.leases = Vec::with_capacity(1);
            table.events = Vec::with_capacity(conduit_body::MAX_PRESENCE_EVENTS);
            let started = Instant::now();
            table
                .start(
                    membership,
                    part,
                    session_id.clone(),
                    sequence,
                    0,
                    lease_millis,
                    sign,
                )
                .map_err(|error| format!("member presence start: {error:?}"))?;
            active.presence = Some(Presence {
                clock: started,
                table,
            });
        }
        let lease = active
            .presence
            .as_ref()
            .and_then(|presence| presence.table.leases.first())
            .cloned()
            .ok_or("member presence absent")?;
        self.refresh_member_carrier_route_presence()?;
        Ok(lease)
    }
}
