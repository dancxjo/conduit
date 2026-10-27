use super::*;
pub const MAXIMUM_KNOWN_SCAN_RESULTS: usize = 16;
pub const KNOWN_SCAN_FRESH_MS: u64 = 15_000;
pub const KNOWN_MINIMUM_DWELL_MS: u64 = 60_000;
pub const KNOWN_FAILURE_COOLDOWN_MS: u64 = 30_000;
pub const KNOWN_RSSI_MARGIN_DB: i16 = 8;
pub const KNOWN_SUSTAINED_OBSERVATIONS: u8 = 3;
pub const KNOWN_MAXIMUM_ATTEMPTS_PER_ROUND: u8 = 3;
/// Compatibility name: the bound applies to one explicit recovery round.
pub const KNOWN_MAXIMUM_ATTEMPTS_PER_GENERATION: u8 = KNOWN_MAXIMUM_ATTEMPTS_PER_ROUND;
/// Actual scan data supplied by the radio owner; unsupported security must not
/// be translated into a compatible enum. This struct grants no scan authority.
pub struct KnownScan<'a> {
    pub ssid: &'a [u8],
    pub security: KnownSecurity,
    pub rssi_dbm: i16,
    pub observed_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentKnownLine {
    pub key: KnownProfileKey,
    pub since_ms: u64,
    pub observed_ms: u64,
    pub healthy: bool,
    pub verified_body_peer: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownSelection {
    Keep(KnownProfileKey),
    Wait,
    AwaitOutcome(KnownProfileKey),
    Attempt(KnownProfileKey),
    Exhausted,
}
#[derive(Clone, Copy)]
struct State {
    key: KnownProfileKey,
    attempts: u8,
    round: u64,
    cooldown_until: u64,
    last_success: Option<u64>,
}
#[derive(Clone, Copy)]
struct Candidate {
    key: KnownProfileKey,
    observed_ms: u64,
    count: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Renewal {
    key: KnownProfileKey,
    round: u64,
    sequence: u64,
    scan_ms: u64,
    requested_ms: u64,
}
/// Opaque correlation for a requested recovery round. This is data, not proof
/// of AP restoration, owner retirement, a grant or Body membership. Not Clone.
#[derive(Debug)]
pub struct KnownRecoveryRenewal(Renewal);
impl KnownRecoveryRenewal {
    pub fn profile(&self) -> KnownProfileKey {
        self.0.key
    }
    pub fn exhausted_round(&self) -> u64 {
        self.0.round
    }
}
/// Fixed state across polls/Line changes; never reconstruct to reset attempts.
/// Each Attempt reserves one of three attempts in that profile recovery round.
/// Caller reports its matching outcome after affine execution/retirement. A
/// lost outcome deliberately fences further decisions; timeout is not success.
pub struct KnownNetworkPolicy {
    last_now: u64,
    states: [Option<State>; 4],
    pending: Option<KnownProfileKey>,
    candidate: Option<Candidate>,
    renewal: Option<Renewal>,
    renewal_sequence: u64,
}
impl Default for KnownNetworkPolicy {
    fn default() -> Self {
        Self::new()
    }
}
impl KnownNetworkPolicy {
    pub const fn new() -> Self {
        Self {
            last_now: 0,
            states: [None; 4],
            pending: None,
            candidate: None,
            renewal: None,
            renewal_sequence: 0,
        }
    }
    fn clock(&self, now: u64) -> Result<(), KnownNetworkRefusal> {
        if now < self.last_now {
            Err(KnownNetworkRefusal::Clock)
        } else {
            Ok(())
        }
    }
    pub fn select(
        &mut self,
        profiles: &KnownNetworkProfiles,
        now: u64,
        scans: &[KnownScan<'_>],
        current: Option<CurrentKnownLine>,
    ) -> Result<KnownSelection, KnownNetworkRefusal> {
        self.clock(now)?;
        if scans.len() > MAXIMUM_KNOWN_SCAN_RESULTS
            || scans
                .iter()
                .any(|s| s.observed_ms > now || !(-127..=0).contains(&s.rssi_dbm))
        {
            return Err(KnownNetworkRefusal::Bounds);
        }
        if let Some(line) = current {
            profiles.info(line.key)?;
            if line.since_ms > now || line.observed_ms > now {
                return Err(KnownNetworkRefusal::Clock);
            }
        }
        self.last_now = now;
        if let Some(key) = self.pending {
            return Ok(KnownSelection::AwaitOutcome(key));
        }
        // Preserve an actually current verified Body route even when another
        // isolated LAN reports a stronger RSSI or higher configured priority.
        let healthy = current.filter(|l| l.healthy && now - l.observed_ms <= KNOWN_SCAN_FRESH_MS);
        if let Some(line) = healthy.filter(|l| l.verified_body_peer) {
            self.candidate = None;
            return Ok(KnownSelection::Keep(line.key));
        }
        let mut best: Option<(KnownProfileInfo, i16, u64, Option<u64>)> = None;
        let mut current_rssi = None;
        let mut exhausted = false;
        for info in profiles.entries() {
            let slot = info.key.slot as usize;
            if self.states[slot].is_none_or(|s| s.key != info.key) {
                self.states[slot] = Some(State {
                    key: info.key,
                    attempts: 0,
                    round: 1,
                    cooldown_until: 0,
                    last_success: None,
                });
            }
            let state = self.states[slot].as_ref().unwrap();
            let scan = scans
                .iter()
                .filter(|s| {
                    now - s.observed_ms <= KNOWN_SCAN_FRESH_MS
                        && profiles.matches(info.key, s.ssid, s.security)
                })
                .max_by_key(|s| (s.rssi_dbm, s.observed_ms));
            let Some(scan) = scan else {
                continue;
            };
            if healthy.is_some_and(|l| l.key == info.key) {
                current_rssi = Some(scan.rssi_dbm);
            }
            if state.attempts >= KNOWN_MAXIMUM_ATTEMPTS_PER_ROUND {
                exhausted = true;
                continue;
            }
            if now < state.cooldown_until {
                continue;
            }
            let rank = (
                info.priority,
                scan.rssi_dbm,
                state.last_success,
                core::cmp::Reverse(info.key.slot),
            );
            if best.as_ref().is_none_or(|(old, rssi, _, success)| {
                rank > (
                    old.priority,
                    *rssi,
                    *success,
                    core::cmp::Reverse(old.key.slot),
                )
            }) {
                best = Some((info, scan.rssi_dbm, scan.observed_ms, state.last_success));
            }
        }
        let Some((best, rssi, observed, _)) = best else {
            self.candidate = None;
            return Ok(healthy.map_or(
                if exhausted {
                    KnownSelection::Exhausted
                } else {
                    KnownSelection::Wait
                },
                |l| KnownSelection::Keep(l.key),
            ));
        };
        if let Some(line) = healthy {
            if line.key == best.key
                || now - line.since_ms < KNOWN_MINIMUM_DWELL_MS
                || current_rssi.is_none_or(|old| rssi < old + KNOWN_RSSI_MARGIN_DB)
            {
                self.candidate = None;
                return Ok(KnownSelection::Keep(line.key));
            }
            let candidate = match self.candidate {
                Some(mut c)
                    if c.key == best.key
                        && observed >= c.observed_ms
                        && observed - c.observed_ms <= KNOWN_SCAN_FRESH_MS =>
                {
                    if observed > c.observed_ms {
                        c.count = c.count.saturating_add(1);
                        c.observed_ms = observed;
                    }
                    c
                }
                _ => Candidate {
                    key: best.key,
                    observed_ms: observed,
                    count: 1,
                },
            };
            self.candidate = Some(candidate);
            if candidate.count < KNOWN_SUSTAINED_OBSERVATIONS {
                return Ok(KnownSelection::Keep(line.key));
            }
        }
        self.candidate = None;
        self.states[best.key.slot as usize]
            .as_mut()
            .unwrap()
            .attempts += 1;
        self.pending = Some(best.key);
        Ok(KnownSelection::Attempt(best.key))
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
    pub fn outcome(
        &mut self,
        key: KnownProfileKey,
        now: u64,
        success: bool,
    ) -> Result<(), KnownNetworkRefusal> {
        self.clock(now)?;
        if self.pending != Some(key) {
            return Err(KnownNetworkRefusal::Outcome);
        }
        let state = self
            .states
            .get_mut(key.slot as usize)
            .and_then(Option::as_mut)
            .filter(|s| s.key == key)
            .ok_or(KnownNetworkRefusal::StaleProfile)?;
        let cooldown = if success {
            0
        } else {
            now.checked_add(KNOWN_FAILURE_COOLDOWN_MS)
                .ok_or(KnownNetworkRefusal::Clock)?
        };
        if success {
            state.last_success = Some(now);
        }
        state.cooldown_until = cooldown;
        self.pending = None;
        self.last_now = now;
        Ok(())
    }
}

#[cfg(test)]
mod round_overflow_tests {
    use super::*;
    #[test]
    fn exhausted_round_and_request_generations_never_wrap() {
        let mut profiles = KnownNetworkProfiles::new();
        let key = profiles
            .upsert(
                PrivateNetworkInput::new(
                    b"known",
                    b"public-test-pass",
                    KnownSecurity::Wpa2Personal,
                    0,
                )
                .unwrap(),
            )
            .unwrap();
        let mut policy = KnownNetworkPolicy::new();
        policy.states[key.slot as usize] = Some(State {
            key,
            attempts: 3,
            round: u64::MAX,
            cooldown_until: 0,
            last_success: None,
        });
        let scan = KnownScan {
            ssid: b"known",
            security: KnownSecurity::Wpa2Personal,
            rssi_dbm: -50,
            observed_ms: 0,
        };
        assert!(matches!(
            policy.request_recovery_round(&profiles, key, 0, &scan),
            Err(KnownNetworkRefusal::Exhausted)
        ));
        policy.states[key.slot as usize].as_mut().unwrap().round = 1;
        policy.renewal_sequence = u64::MAX;
        assert!(matches!(
            policy.request_recovery_round(&profiles, key, 0, &scan),
            Err(KnownNetworkRefusal::Exhausted)
        ));
        assert!(policy.renewal.is_none());
        assert_eq!(policy.states[key.slot as usize].unwrap().attempts, 3);
    }
}
