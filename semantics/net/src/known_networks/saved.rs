//! Saved protected-profile fallback without a scan, RSSI or reachability claim.
use super::*;
impl KnownNetworkPolicy {
    /// Reserve one bounded saved-profile attempt. An explicit preferred key is
    /// considered alone; otherwise prefer an eligible alternate to the last
    /// failed profile, then configured priority and stable slot order. This
    /// creates no scan evidence or reachability claim. Exhausted rounds remain
    /// exhausted until the separate scan-backed Root renewal contract is met.
    pub fn select_saved(
        &mut self,
        profiles: &KnownNetworkProfiles,
        now: u64,
        preferred: Option<KnownProfileKey>,
        backend: KnownBackendAuth,
    ) -> Result<KnownSelection, KnownNetworkRefusal> {
        self.clock(now)?;
        if let Some(key) = preferred {
            profiles.info(key)?;
        }
        if let Some(key) = self.pending {
            return Ok(KnownSelection::AwaitOutcome(key));
        }
        let mut best = None;
        let mut exhausted = false;
        for info in profiles.entries() {
            if preferred.is_some_and(|key| key != info.key) || !backend.supports(info.security) {
                continue;
            }
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
            if state.attempts >= KNOWN_MAXIMUM_ATTEMPTS_PER_ROUND {
                exhausted = true;
                continue;
            }
            if now < state.cooldown_until {
                continue;
            }
            if best.is_none_or(|old: KnownProfileInfo| {
                (
                    Some(info.key) != self.saved_last_failed,
                    info.priority,
                    core::cmp::Reverse(slot),
                ) > (
                    Some(old.key) != self.saved_last_failed,
                    old.priority,
                    core::cmp::Reverse(old.key.slot as usize),
                )
            }) {
                best = Some(info);
            }
        }
        self.last_now = now;
        let Some(best) = best else {
            return Ok(if exhausted {
                KnownSelection::Exhausted
            } else {
                KnownSelection::Wait
            });
        };
        self.states[best.key.slot as usize]
            .as_mut()
            .unwrap()
            .attempts += 1;
        self.pending = Some(best.key);
        Ok(KnownSelection::Attempt(best.key))
    }
    /// ROOT integration boundary: only after actual unused Ready/source owners
    /// returned, retire a matching unissued reservation. No success/failure is
    /// invented, and no attempt, cooldown or round is refunded or renewed.
    pub fn retire_unissued(
        &mut self,
        key: KnownProfileKey,
        now: u64,
    ) -> Result<(), KnownNetworkRefusal> {
        self.clock(now)?;
        if self.pending != Some(key) {
            return Err(KnownNetworkRefusal::Outcome);
        }
        self.pending = None;
        self.last_now = now;
        Ok(())
    }
}
