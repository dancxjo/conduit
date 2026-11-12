//! Finite per-member original approvals, retained until actual receiver retirement.
use super::*;
use conduit_body::{
    host_command_attribution_transcript, BodyPlayIdentity, HostCommandChallenge,
    HOST_PREPARED_WORKSET_CONTRACT,
};
use conduit_embedded_build::{
    BodyFragmentProjectionLimits, GeneratedBodyWorksetDocument, PreparedResidentExpression,
};

/// No public constructor, Clone or deserializer. Retaining this command does
/// not acknowledge receiver preparation, admission or execution.
pub(crate) struct PreparedRemoteBodyCommand {
    part: conduit_body::PartId,
    host: conduit_core::HostId,
    play: BodyPlayIdentity,
    document: GeneratedBodyWorksetDocument,
    transcript: [u8; 32],
    signature: [u8; 64],
}
impl PreparedRemoteBodyCommand {
    pub(crate) fn play(&self) -> &BodyPlayIdentity {
        &self.play
    }
    pub(crate) fn payload(&self) -> &[u8] {
        self.document.bytes()
    }
    pub(crate) fn transcript(&self) -> &[u8; 32] {
        &self.transcript
    }
    pub(crate) fn signature(&self) -> &[u8; 64] {
        &self.signature
    }
}
impl Owner {
    pub(crate) fn require_remote_workset_retired(&self) -> Result<(), String> {
        if self
            .member_carrier
            .as_ref()
            .is_some_and(|custody| !custody.commands.is_empty())
        {
            return Err("workset replacement requires actual remote receiver retirement".into());
        }
        Ok(())
    }
    pub(crate) fn approve_remote_workset(
        &mut self,
        member: &MemberPresentationBasis,
        challenge: &HostCommandChallenge<'_>,
        images: &[&PreparedResidentExpression],
        native: &[&conduit_core::CapabilityOffer],
        limits: BodyFragmentProjectionLimits,
    ) -> Result<&PreparedRemoteBodyCommand, String> {
        self.approve_remote_workset_with_line_fore(
            member,
            challenge,
            images,
            native,
            limits,
            &std::collections::BTreeMap::new(),
        )
    }
    /// Original trusted session selection follows the SAME approval/signing
    /// custody path. Received wire metadata can never supply these choices.
    #[allow(clippy::too_many_arguments)] // Original approval inputs plus explicit native sessions.
    pub(crate) fn approve_remote_workset_with_line_fore(
        &mut self,
        member: &MemberPresentationBasis,
        challenge: &HostCommandChallenge<'_>,
        images: &[&PreparedResidentExpression],
        native: &[&conduit_core::CapabilityOffer],
        limits: BodyFragmentProjectionLimits,
        sessions: &std::collections::BTreeMap<
            conduit_body::ResidentPlot,
            Vec<conduit_embedded_build::SelectedForeSession>,
        >,
    ) -> Result<&PreparedRemoteBodyCommand, String> {
        if let Some(custody) = &self.member_carrier {
            if custody.commands.iter().any(|command| {
                command.part == member.credential.part_id
                    || command.host == member.host_advertisement.host_id
            }) {
                return Err("remote approval for this Part or Host remains retained".into());
            }
            if custody.commands.len() >= conduit_body::MAX_BODY_PARTS {
                return Err("finite remote approval capacity exhausted".into());
            }
        }
        self.require_current_native_member(member)?;
        self.require_observed_member_offer(&member.host_advertisement)?;
        let issuer = self.member_carrier_peer()?;
        let credential = &member.credential;
        let actual = &member.host_advertisement;
        let lifetime = challenge
            .expires_at_millis
            .checked_sub(challenge.issued_at_millis);
        if challenge.body_id != credential.body_id.as_str()
            || challenge.part_id != credential.part_id.as_str()
            || challenge.credential_id != credential.credential_id.as_str()
            || challenge.receiver_host != actual.host_id.as_str()
            || challenge.receiver_boot != actual.boot_id.as_str()
            || challenge.receiver_generation != actual.offer_generation.0
            || challenge.receiver_profile != actual.profile.as_str()
            || challenge.issuer_host != issuer.host_id.as_str()
            || challenge.issuer_boot != issuer.boot_id.as_str()
            || challenge.issuer_generation != issuer.offer_generation.0
            || challenge.issuer_key != issuer.verifying_key
            || challenge.payload_contract != HOST_PREPARED_WORKSET_CONTRACT
            || challenge.maximum_payload_bytes == 0
            || challenge.maximum_payload_bytes > limits.maximum_document_bytes
            || challenge.nonce == [0; 32]
            || !lifetime.is_some_and(|millis| (1..=10_000).contains(&millis))
        {
            return Err(
                "remote preparation challenge differs from current authorized scope".into(),
            );
        }
        // Receiver time is a different clock. The transport must bound elapsed
        // carriage; only the receiver can accept its original unexpired slot.
        let limits = BodyFragmentProjectionLimits {
            maximum_document_bytes: challenge.maximum_payload_bytes,
            ..limits
        };
        let (play, document) = self.prepare_remote_mixed_workset_with_line_fore(
            actual, images, native, limits, sessions,
        )?;
        if self
            .member_carrier
            .as_ref()
            .is_some_and(|custody| custody.commands.iter().any(|command| command.play != play))
        {
            return Err("retained remote approvals pin another exact Body Plan or Play".into());
        }
        let transcript = host_command_attribution_transcript(challenge, document.bytes())
            .map_err(|error| format!("remote command transcript refused: {error:?}"))?;
        let custody = self
            .member_carrier
            .as_mut()
            .ok_or("owner issuer custody absent")?;
        let signature = custody
            .key
            .as_ref()
            .ok_or("owner issuer key absent")?
            .sign(&transcript);
        custody.commands.push(PreparedRemoteBodyCommand {
            part: credential.part_id.clone(),
            host: actual.host_id.clone(),
            play,
            document,
            transcript,
            signature,
        });
        // No timer or transport failure clears these slots. Receiver retirement
        // and installation/Start remain separate unfinished protocol steps.
        Ok(custody
            .commands
            .last()
            .expect("just retained original command"))
    }
}
