//! Explicit grant shape for an independently authorized selected cue operation.
impl crate::StdHost {
    /// Call only after independent authorization. Discovery and Base attachment
    /// never call this method and therefore never create playback authority.
    pub fn startup_chime_authority_grant(
        &self,
        grant_id: &str,
    ) -> Result<conduit_core::AuthorityGrant, String> {
        if grant_id.is_empty() || grant_id == "default" {
            return Err("startup chime requires an explicit grant identity".into());
        }
        let selected = self
            .playback
            .as_ref()
            .ok_or("startup chime has no selected speaker")?;
        if selected.boot_id != self.advertisement.boot_id
            || selected.offer_generation != self.advertisement.offer_generation
            || selected.executable_digest().is_none()
        {
            return Err("startup chime speaker is stale or unchecked".into());
        }
        let expected = conduit_std_offers::startup_chime_offer();
        let offer = self
            .advertisement
            .capabilities
            .iter()
            .find(|o| **o == expected)
            .ok_or("exact startup chime capability is not currently advertised")?;
        let required = offer
            .authority_requirements
            .first()
            .ok_or("startup chime lacks its exact authority requirement")?;
        Ok(conduit_core::AuthorityGrant {
            grant_id: grant_id.into(),
            contract_id: required.contract_id.clone(),
            host_call_contract_id: required.host_call_contract_id.clone(),
            subject_kind: required.subject_kind.clone(),
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            capability_id: offer.capability_id.clone(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::super::{AlsaPlaybackObservation, HostedPlaybackSelection};
    use crate::{StdHost, StdHostConfig};
    #[test]
    fn cue_grant_requires_checked_current_selection_and_explicit_identity() {
        let config = StdHostConfig {
            host_id: "host/cue".into(),
            boot_id: "boot/cue".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        };
        let mut host = StdHost::new_with_config(config.clone());
        assert!(host.startup_chime_authority_grant("grant/cue").is_err());
        let selection = HostedPlaybackSelection::from_observation(
            AlsaPlaybackObservation {
                card_index: 0,
                card_id: "fixture".into(),
                card_name: "Fixture".into(),
                device: 0,
                device_name: "PCM".into(),
                base_identity: "fixture/physical".into(),
            },
            config.boot_id,
            config.offer_generation,
        )
        .with_executable_sha256("ab".repeat(32))
        .unwrap();
        host.attach_selected_playback(selection).unwrap();
        assert!(host.startup_chime_authority_grant("default").is_err());
        let grant = host.startup_chime_authority_grant("grant/cue").unwrap();
        assert_eq!(
            grant.host_call_contract_id.as_str(),
            conduit_std_offers::STARTUP_CHIME_OPERATION
        );
        assert_eq!(grant.subject_kind.as_str(), conduit_core::BOOL_INFO_ID);
        host.playback.as_mut().unwrap().offer_generation = conduit_core::OfferGeneration(2);
        assert!(host.startup_chime_authority_grant("grant/stale").is_err());
    }
}
