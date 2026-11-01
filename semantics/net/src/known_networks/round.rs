//! Explicit recovery-round renewal after fresh scan and actual owner retirement.
use super::*;
impl KnownNetworkPolicy {
    /// Rank only fresh, explicitly compatible observations of exhausted saved
    /// profiles. Unknown security remains unknown and cannot renew a round.
    /// This returns the same non-authorizing one-shot correlation as the strict
    /// per-profile API; actual Root retirement must still precede renewal.
    pub fn request_observed_recovery_round<'a>(
        &mut self,
        profiles: &KnownNetworkProfiles,
        now: u64,
        scans: impl ExactSizeIterator<Item = KnownScanObservation<'a>> + Clone,
        backend: KnownBackendAuth,
    ) -> Result<KnownRecoveryRenewal, KnownNetworkRefusal> {
        self.clock(now)?;
        if scans.len() > MAXIMUM_KNOWN_SCAN_RESULTS {
            return Err(KnownNetworkRefusal::Bounds);
        }
        for (index, scan) in scans
            .clone()
            .take(MAXIMUM_KNOWN_SCAN_RESULTS + 1)
            .enumerate()
        {
            if index == MAXIMUM_KNOWN_SCAN_RESULTS
                || scan.observed_ms > now
                || !(-127..=0).contains(&scan.rssi_dbm)
            {
                return Err(KnownNetworkRefusal::Bounds);
            }
        }
        if self.pending.is_some() {
            return Err(KnownNetworkRefusal::Outcome);
        }
        let mut best: Option<(KnownProfileInfo, KnownScanObservation<'a>, Option<u64>)> = None;
        for info in profiles.entries() {
            let Some(state) = self.states[info.key.slot as usize]
                .as_ref()
                .filter(|state| state.key == info.key)
            else {
                continue;
            };
            if state.attempts < KNOWN_MAXIMUM_ATTEMPTS_PER_ROUND
                || now < state.cooldown_until
                || !backend.supports(info.security)
            {
                continue;
            }
            let observed = scans
                .clone()
                .take(MAXIMUM_KNOWN_SCAN_RESULTS)
                .filter(|scan| {
                    matches!(scan.security, ObservedNetworkSecurity::Known(security)
                    if profiles.matches(info.key, scan.ssid, security))
                        && scan.observed_ms >= state.cooldown_until
                        && scan.observed_ms <= now
                        && (-127..=0).contains(&scan.rssi_dbm)
                        && now - scan.observed_ms <= KNOWN_SCAN_FRESH_MS
                })
                .max_by_key(|scan| (scan.rssi_dbm, scan.observed_ms));
            let Some(scan) = observed else {
                continue;
            };
            // Same priority/RSSI/previous-success/stable-slot order as ordinary
            // observed selection. No credential strings or result array copies.
            let rank = (
                info.priority,
                scan.rssi_dbm,
                state.last_success,
                core::cmp::Reverse(info.key.slot),
            );
            if best.as_ref().is_none_or(|(old, scan, success)| {
                rank > (
                    old.priority,
                    scan.rssi_dbm,
                    *success,
                    core::cmp::Reverse(old.key.slot),
                )
            }) {
                best = Some((info, scan, state.last_success));
            }
        }
        let (info, scan, _) = best.ok_or(KnownNetworkRefusal::Bounds)?;
        let ObservedNetworkSecurity::Known(security) = scan.security else {
            unreachable!()
        };
        self.request_recovery_round(
            profiles,
            info.key,
            now,
            &KnownScan {
                ssid: scan.ssid,
                security,
                rssi_dbm: scan.rssi_dbm,
                observed_ms: scan.observed_ms,
            },
        )
    }
    /// Request data only, after the exhausted round's cooldown and an actual
    /// fresh compatible scan observed at/after cooldown. It cannot itself renew
    /// attempts. A newer request invalidates any older outstanding correlation.
    pub fn request_recovery_round(
        &mut self,
        profiles: &KnownNetworkProfiles,
        key: KnownProfileKey,
        now: u64,
        scan: &KnownScan<'_>,
    ) -> Result<KnownRecoveryRenewal, KnownNetworkRefusal> {
        self.clock(now)?;
        profiles.info(key)?;
        if self.pending.is_some() {
            return Err(KnownNetworkRefusal::Outcome);
        }
        let state = self
            .states
            .get(key.slot as usize)
            .and_then(Option::as_ref)
            .filter(|s| s.key == key)
            .ok_or(KnownNetworkRefusal::StaleProfile)?;
        if state.attempts < KNOWN_MAXIMUM_ATTEMPTS_PER_ROUND
            || now < state.cooldown_until
            || scan.observed_ms < state.cooldown_until
            || scan.observed_ms > now
            || now - scan.observed_ms > KNOWN_SCAN_FRESH_MS
            || !(-127..=0).contains(&scan.rssi_dbm)
            || !profiles.matches(key, scan.ssid, scan.security)
        {
            return Err(KnownNetworkRefusal::Bounds);
        }
        state
            .round
            .checked_add(1)
            .ok_or(KnownNetworkRefusal::Exhausted)?;
        let sequence = self
            .renewal_sequence
            .checked_add(1)
            .ok_or(KnownNetworkRefusal::Exhausted)?;
        let renewal = Renewal {
            key,
            round: state.round,
            sequence,
            scan_ms: scan.observed_ms,
            requested_ms: now,
        };
        self.renewal = Some(renewal);
        self.renewal_sequence = sequence;
        self.last_now = now;
        Ok(KnownRecoveryRenewal(renewal))
    }
    /// ROOT-ONLY INTEGRATION BOUNDARY: the caller must first confirm actual AP
    /// restoration and retirement of the exact exhausted round's affine owners
    /// and mandatory evidence. Do not expose this method directly to an HTTP
    /// route, timer or scan result. Pure policy cannot authenticate that custody;
    /// invoking this method is the caller's explicit confirmation, not its proof.
    /// A matching request is consumed once, while round numbers never rewind.
    pub fn renew_after_retirement(
        &mut self,
        profiles: &KnownNetworkProfiles,
        request: KnownRecoveryRenewal,
        now: u64,
    ) -> Result<u64, KnownNetworkRefusal> {
        self.clock(now)?;
        profiles.info(request.0.key)?;
        if self.pending.is_some() || self.renewal != Some(request.0) {
            return Err(KnownNetworkRefusal::Outcome);
        }
        let state = self
            .states
            .get_mut(request.0.key.slot as usize)
            .and_then(Option::as_mut)
            .filter(|s| s.key == request.0.key && s.round == request.0.round)
            .ok_or(KnownNetworkRefusal::StaleProfile)?;
        if state.attempts < KNOWN_MAXIMUM_ATTEMPTS_PER_ROUND
            || now < state.cooldown_until
            || now < request.0.requested_ms
            || now - request.0.scan_ms > KNOWN_SCAN_FRESH_MS
        {
            return Err(KnownNetworkRefusal::Bounds);
        }
        let round = state
            .round
            .checked_add(1)
            .ok_or(KnownNetworkRefusal::Exhausted)?;
        state.round = round;
        state.attempts = 0;
        self.renewal = None;
        self.candidate = None;
        self.last_now = now;
        Ok(round)
    }
}
