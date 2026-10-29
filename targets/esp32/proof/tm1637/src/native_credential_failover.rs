//! Finite saved-profile policy, retained with the actual private source owner.
//! No scan or automatic exhausted-round renewal is supplied by this adapter.
use super::known;
use conduit_net::{KnownNetworkPolicy, KnownProfileKey, KnownSelection};
struct Reservation {
    selection: known::Selection,
    generation: u64,
}
pub(super) struct Fallback {
    policy: KnownNetworkPolicy,
    reserved: Option<Reservation>,
    pending: Option<KnownProfileKey>,
    taken: Option<known::Selection>,
}
impl Fallback {
    pub(super) fn new() -> Self {
        Self {
            policy: KnownNetworkPolicy::new(),
            reserved: None,
            pending: None,
            taken: None,
        }
    }
    pub(super) fn choose(
        &mut self,
        profiles: &conduit_net::KnownNetworkProfiles,
        now: u64,
        preferred: Option<KnownProfileKey>,
    ) -> Result<KnownProfileKey, &'static str> {
        match self
            .policy
            .select_saved(
                profiles,
                now,
                preferred,
                conduit_net::KnownBackendAuth {
                    wpa2_personal: true,
                    wpa3_personal: false,
                },
            )
            .map_err(|_| "credential-policy")?
        {
            KnownSelection::Attempt(key) | KnownSelection::AwaitOutcome(key) => {
                if preferred.is_some_and(|wanted| wanted != key) {
                    return Err("credential-policy-pending");
                }
                self.pending = Some(key);
                Ok(key)
            }
            KnownSelection::Exhausted => Err("credential-policy-exhausted"),
            _ => Err("credential-policy-waiting"),
        }
    }
    pub(super) fn frozen(
        &mut self,
        selection: known::Selection,
        generation: u64,
    ) -> Result<(), &'static str> {
        if self.pending != Some(selection.profile) || self.reserved.is_some() || generation == 0 {
            return Err("credential-policy-scope");
        }
        self.reserved = Some(Reservation {
            selection,
            generation,
        });
        Ok(())
    }
    pub(super) fn validates(&self, selection: known::Selection, generation: u64) -> bool {
        self.pending == Some(selection.profile)
            && self.taken.is_none()
            && self
                .reserved
                .as_ref()
                .is_some_and(|r| r.selection == selection && r.generation == generation)
    }
    pub(super) fn taken(&mut self, selection: known::Selection) -> Result<(), &'static str> {
        if self.taken.is_some()
            || self
                .reserved
                .as_ref()
                .is_none_or(|r| r.selection != selection)
        {
            return Err("credential-policy-scope");
        }
        self.taken = Some(selection);
        Ok(())
    }
    pub(super) fn supersede(&mut self, now: u64) -> Result<(), &'static str> {
        if let Some(key) = self.pending {
            if self.taken.is_some() {
                return Err("credential-policy-inflight");
            }
            self.policy
                .retire_unissued(key, now)
                .map_err(|_| "credential-policy-retirement")?;
            self.reserved = None;
            self.pending = None;
        }
        Ok(())
    }
    pub(super) fn settled(
        &mut self,
        generation: u64,
        revision: u64,
        outcome: Option<(bool, u64)>,
        now: u64,
    ) -> Result<(), &'static str> {
        let Some(reserved) = &self.reserved else {
            return Err("credential-policy-outcome");
        };
        if reserved.generation != generation || reserved.selection.store_revision != revision {
            return Err("credential-policy-scope");
        }
        match (self.taken, outcome) {
            (Some(taken), Some((success, observed)))
                if taken == reserved.selection && observed <= now =>
            {
                self.policy
                    .outcome(taken.profile, observed, success)
                    .map_err(|_| "credential-policy-outcome")?;
            }
            (None, None) => self
                .policy
                .retire_unissued(reserved.selection.profile, now)
                .map_err(|_| "credential-policy-retirement")?,
            // Consumed input with no actual station outcome remains fenced.
            _ => return Err("credential-policy-outcome"),
        }
        self.reserved = None;
        self.pending = None;
        self.taken = None;
        Ok(())
    }
}
