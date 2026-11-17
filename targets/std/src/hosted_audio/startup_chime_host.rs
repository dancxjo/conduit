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
