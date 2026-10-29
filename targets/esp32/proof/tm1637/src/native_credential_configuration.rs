//! Private configuration preparation from exact returned source custody.
use super::*;
impl RootCredentialSource {
    pub(super) fn prepare_configuration(
        &mut self,
    ) -> Result<esp_radio::wifi::sta::StationConfig, &'static str> {
        self.ensure_owner()?;
        let now = embassy_time::Instant::now().as_millis();
        let owner = self.owner.as_deref_mut().expect("loaded private owner");
        let mailbox = self
            .reader
            .try_lock()
            .map_err(|_| "credential-mailbox-busy")?;
        // Caller has returned the actual unselected join owner first. Hold the
        // exact acknowledged token while retiring only its superseded snapshot.
        if mailbox
            .acknowledged_submission()
            .ok()
            .is_some_and(|token| Some(token) != owner.frozen_origin)
        {
            owner.fallback.supersede(now)?;
        }
        selection::retire_superseded(
            &mut owner.profiles,
            &mut owner.frozen_origin,
            mailbox.acknowledged_submission().ok(),
        )?;
        if owner.selection().is_none() {
            let (key, origin) = match mailbox.acknowledged_submission() {
                Ok(submission) => {
                    let key = if let Some((_, key)) = owner
                        .committed_pending
                        .filter(|(token, _)| *token == submission)
                    {
                        key
                    } else {
                        let key = mailbox
                            .prepare_acknowledged(|request| {
                                let input = conduit_net::PrivateNetworkInput::new(
                                    request.ssid,
                                    request.credential,
                                    conduit_net::KnownSecurity::Wpa2Personal,
                                    0,
                                )
                                .map_err(|_| "credential-configuration")?;
                                #[cfg(not(any(
                                    feature = "local-evidence-store",
                                    feature = "network-credential-store"
                                )))]
                                let volatile_revision = owner
                                    .store_revision()
                                    .checked_add(1)
                                    .ok_or("credential-configuration-exhausted")?;
                                owner
                                    .upsert_after_acknowledgement(input, |wire| {
                                        #[cfg(any(
                                            feature = "local-evidence-store",
                                            feature = "network-credential-store"
                                        ))]
                                        {
                                            durable()
                                                .ok_or(())?
                                                .lock(|journal| journal.borrow_mut().commit(wire))
                                                .map_err(|_| ())
                                        }
                                        #[cfg(not(any(
                                            feature = "local-evidence-store",
                                            feature = "network-credential-store"
                                        )))]
                                        {
                                            // Finite within-Boot private acceptance only.
                                            // This feature does not persist or advertise
                                            // a committed flash configuration; public
                                            // committed_revision remains zero.
                                            let _ = wire;
                                            Ok(volatile_revision)
                                        }
                                    })
                                    .map_err(|_| "credential-journal-commit")
                            })
                            .map_err(|_| "credential-not-acknowledged")??;
                        owner.committed_pending = Some((submission, key));
                        key
                    };
                    (key, Some(submission))
                }
                Err(NetworkSetupRefusal::NoSubmission) => {
                    let key = owner
                        .fallback
                        .choose(owner.profiles.profiles(), now, None)?;
                    (key, None)
                }
                Err(_) => return Err("credential-not-acknowledged"),
            };
            if owner
                .profiles()
                .info(key)
                .map_err(|_| "credential-profile-key")?
                .security
                != conduit_net::KnownSecurity::Wpa2Personal
            {
                return Err("credential-auth-unsupported");
            }
            let key = owner
                .fallback
                .choose(owner.profiles.profiles(), now, Some(key))?;
            let frozen = owner.freeze(key).map_err(|_| "credential-selection")?;
            owner.fallback.frozen(frozen, self.generation)?;
            owner.chosen = Some(key);
            owner.frozen_origin = origin;
        }
        if owner.selection().ok_or("credential-selection")?.security
            != conduit_net::KnownSecurity::Wpa2Personal
        {
            return Err("credential-auth-unsupported");
        }
        // SDK refusal deliberately retains the frozen snapshot for explicit
        // returned-owner retirement/retry, never silently clears private custody.
        if !owner.fallback.validates(
            owner.selection().ok_or("credential-selection")?,
            self.generation,
        ) {
            return Err("credential-policy-scope");
        }
        owner
            .with_selected_request(conduit_esp32_network_switch::prepare_station_configuration)
            .map_err(|_| "credential-selection")?
            .map_err(configuration_refusal)
    }
    pub(super) fn prepare_saved_retry(
        &mut self,
    ) -> Result<esp_radio::wifi::sta::StationConfig, &'static str> {
        let owner = self
            .owner
            .as_deref_mut()
            .ok_or("credential-owner-unavailable")?;
        if owner.selection().is_none() {
            let now = embassy_time::Instant::now().as_millis();
            let key = owner
                .fallback
                .choose(owner.profiles.profiles(), now, owner.chosen)?;
            let frozen = owner.freeze(key).map_err(|_| "credential-selection")?;
            owner.fallback.frozen(frozen, self.generation)?;
            owner.chosen = Some(key);
            owner.frozen_origin = None;
        }
        if !owner.fallback.validates(
            owner.selection().ok_or("credential-selection")?,
            self.generation,
        ) {
            return Err("credential-policy-scope");
        }
        owner
            .with_selected_request(conduit_esp32_network_switch::prepare_station_configuration)
            .map_err(|_| "credential-selection")?
            .map_err(configuration_refusal)
    }
}

// Public status distinguishes finite preparation pressure without exposing the
// selected SSID, password, profile key, or any native identity.
fn configuration_refusal(
    reason: conduit_esp32_network_switch::StationConfigurationRefusal,
) -> &'static str {
    match reason {
        conduit_esp32_network_switch::StationConfigurationRefusal::Capacity => {
            "credential-config-capacity"
        }
        _ => "credential-configuration",
    }
}
