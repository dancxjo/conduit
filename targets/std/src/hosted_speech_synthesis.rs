//! Explicitly admitted local eSpeak synthesis. Produces PCM, never opens a speaker.
use crate::hosted_process::{run_process, ProcessError, ProcessRequest, ProcessTerminal};
use conduit_core::{
    authority_grant, kind_id, resource_offer, AuthorityGrant, AuthorityGrantId, BootId,
    CapabilityOffer, ConfigurationValue, HostBaseId, HostId, OfferGeneration, PlannedGear,
    ResourceContentOffer, ResourceOffer,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{ffi::OsString, time::Duration};
mod discovery;
pub(crate) mod streaming;
mod wav;
pub use crate::hosted_language::{
    language_request_literal, read_language_coverage, read_language_request, HostedLanguageRefusal,
};
pub use discovery::EspeakDiscovery;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspeakFailure {
    InvalidProvider,
    Language(HostedLanguageRefusal),
    ProviderChanged,
    InvalidLimits,
    InvalidText,
    StaleProvider,
    WrongPlacement,
    WrongResource,
    WrongAuthority,
    OutputOverflow,
    InvalidWav,
    SpawnFailed,
    ProviderLost,
    Timeout,
    Cancelled,
}
impl std::fmt::Display for EspeakFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "eSpeak provider refusal: {self:?}")
    }
}
impl std::error::Error for EspeakFailure {}
impl EspeakFailure {
    pub(crate) fn host_failure(
        self,
    ) -> (conduit_kernel::HostCallDisposition, conduit_kernel::Failure) {
        use conduit_kernel::{Failure, FailureCode as C, HostCallDisposition as D};
        let (disposition, code, detail) = match self {
            Self::Language(refusal) => (D::Denied, C::HostCallDenied, refusal.host_detail()),
            Self::InvalidProvider => (D::Denied, C::HostCallDenied, 1),
            Self::ProviderChanged => (D::Denied, C::HostCallDenied, 2),
            Self::StaleProvider => (D::Denied, C::HostCallDenied, 3),
            Self::WrongPlacement => (D::Denied, C::HostCallDenied, 4),
            Self::WrongResource => (D::Denied, C::HostCallDenied, 5),
            Self::WrongAuthority => (D::Denied, C::HostCallDenied, 6),
            Self::InvalidLimits => (D::Failed, C::InvalidInput, 7),
            Self::InvalidText => (D::Failed, C::InvalidInput, 8),
            Self::OutputOverflow => (D::Failed, C::WorkBudgetExhausted, 9),
            Self::InvalidWav => (D::Failed, C::HostCallFailed, 10),
            Self::SpawnFailed => (D::Failed, C::HostCallFailed, 11),
            Self::ProviderLost => (D::Failed, C::HostCallFailed, 12),
            Self::Timeout => (D::Failed, C::HostCallFailed, 13),
            Self::Cancelled => (D::Cancelled, C::Cancelled, 14),
        };
        (disposition, Failure { code, detail })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EspeakSynthesisReceipt {
    pub schema: &'static str,
    pub provider_sha256: String,
    pub text_sha256: String,
    pub pcm_sha256: String,
    pub pcm_bytes: u32,
    pub maximum_pcm_bytes: u32,
    /// Bounds the child process stage; provider file verification is separate.
    pub process_timeout_millis: u64,
}

pub struct EspeakSpeechAdapter {
    discovery: EspeakDiscovery,
    host: HostId,
    boot: BootId,
    generation: OfferGeneration,
    grant: AuthorityGrantId,
    timeout: Duration,
    arguments: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
}

impl EspeakDiscovery {
    pub fn initialize(
        self,
        host: HostId,
        boot: BootId,
        generation: OfferGeneration,
        grant: AuthorityGrantId,
        timeout: Duration,
    ) -> Result<EspeakSpeechAdapter, EspeakFailure> {
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(EspeakFailure::InvalidLimits);
        }
        self.verify()?;
        let arguments = vec![
            "--stdout".into(),
            "--stdin".into(),
            "-b".into(),
            "1".into(),
            "-D".into(),
            "-v".into(),
            self.voice.clone().into(),
            "--path".into(),
            self.data_root
                .parent()
                .ok_or(EspeakFailure::InvalidProvider)?
                .as_os_str()
                .to_owned(),
        ];
        let environment = vec![
            (
                "LD_LIBRARY_PATH".into(),
                self.engine
                    .parent()
                    .ok_or(EspeakFailure::InvalidProvider)?
                    .as_os_str()
                    .to_owned(),
            ),
            ("LC_ALL".into(), "C.UTF-8".into()),
        ];
        Ok(EspeakSpeechAdapter {
            discovery: self,
            host,
            boot,
            generation,
            grant,
            timeout,
            arguments,
            environment,
        })
    }
}
impl EspeakSpeechAdapter {
    /// Recheck the selected provider's complete content before offering a
    /// current route witness. A Boot-time grant alone cannot prove presence.
    pub fn provider_is_current(&self) -> bool {
        self.discovery.verify().is_ok()
    }

    pub fn provider_sha256(&self) -> &str {
        &self.discovery.provider_sha256
    }

    pub fn validate_host(
        &self,
        host: &HostId,
        boot: &BootId,
        generation: OfferGeneration,
    ) -> Result<(), EspeakFailure> {
        if *host != self.host || *boot != self.boot || generation != self.generation {
            return Err(EspeakFailure::StaleProvider);
        }
        Ok(())
    }
    pub fn offer(&self) -> CapabilityOffer {
        let mut offer =
            conduit_std_offers::espeak_speech_offer(self.discovery.content_requirement());
        offer.realization_properties =
            crate::hosted_language::properties(self.discovery.coverage.as_ref());
        offer
    }
    pub fn resource_offer(&self) -> ResourceOffer {
        let mut offer = resource_offer(
            &self.discovery.pool_id(),
            conduit_std_offers::ESPEAK_SPEECH_RESOURCE_CLASS,
            1,
        );
        offer.content = Some(ResourceContentOffer {
            contract: self.discovery.content_requirement(),
            owner_host: self.host.clone(),
            owner_boot: self.boot.clone(),
            base_id: HostBaseId::from("base/espeak-provider"),
            residence_profile: kind_id("std/espeak-ng-local-files@1"),
        });
        offer
    }
    pub fn authority_grant(&self) -> AuthorityGrant {
        let offer = self.offer();
        authority_grant(
            self.grant.as_str(),
            &offer.authority_requirements[0],
            self.host.clone(),
            self.boot.clone(),
            offer.capability_id,
        )
    }
    pub fn streaming_offer(&self) -> CapabilityOffer {
        let mut offer =
            conduit_std_offers::espeak_streaming_offer(self.discovery.content_requirement());
        offer.realization_properties =
            crate::hosted_language::properties(self.discovery.coverage.as_ref());
        offer
    }
    pub fn streaming_authority_grant(&self) -> AuthorityGrant {
        let offer = self.streaming_offer();
        authority_grant(
            self.grant.as_str(),
            &offer.authority_requirements[0],
            self.host.clone(),
            self.boot.clone(),
            offer.capability_id,
        )
    }
    pub fn validate_placement(&self, placement: &PlannedGear) -> Result<u32, EspeakFailure> {
        if placement.host_id != self.host
            || placement.boot_id != self.boot
            || placement.offer_generation != self.generation
        {
            return Err(EspeakFailure::StaleProvider);
        }
        let streaming =
            placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND;
        let offer = if streaming {
            self.streaming_offer()
        } else {
            self.offer()
        };
        if placement.kind_id != offer.kind_id
            || placement.kind_contract_revision != offer.kind_contract_revision
            || placement.capability_id != offer.capability_id
            || placement.implementation_id != offer.implementation.implementation_id
            || placement.execution_profile_id != offer.implementation.execution_profile_id
            || placement.artifact_id != offer.implementation.artifact_id
            || placement.host_calls != offer.host_calls
            || placement.inputs != offer.inputs
            || placement.outputs != offer.outputs
            || placement.semantic_contract != offer.semantic_contract
            || placement.limits != offer.limits
            || placement.realization_properties != offer.realization_properties
        {
            return Err(EspeakFailure::WrongPlacement);
        }
        let request =
            crate::hosted_language::request(placement).map_err(EspeakFailure::Language)?;
        let voice =
            crate::hosted_language::provider_language(self.discovery.coverage.as_ref(), &request)
                .map_err(EspeakFailure::Language)?;
        if voice != self.discovery.voice {
            return Err(EspeakFailure::Language(HostedLanguageRefusal::Mapping));
        }
        let [resource] = placement.resources.as_slice() else {
            return Err(EspeakFailure::WrongResource);
        };
        let offered = self.resource_offer();
        if resource.pool_id != offered.pool_id
            || resource.class_id != offered.class_id
            || resource.units != 1
            || resource.protected.is_some()
            || resource.compute.is_some()
            || resource.content != offered.content
        {
            return Err(EspeakFailure::WrongResource);
        }
        let [authority] = placement.authority.as_slice() else {
            return Err(EspeakFailure::WrongAuthority);
        };
        let grant = if streaming {
            self.streaming_authority_grant()
        } else {
            self.authority_grant()
        };
        if authority.grant_id != grant.grant_id
            || authority.contract_id != grant.contract_id
            || authority.host_call_contract_id != grant.host_call_contract_id
            || authority.subject_kind != grant.subject_kind
            || authority.host_id != grant.host_id
            || authority.boot_id != grant.boot_id
            || authority.capability_id != grant.capability_id
        {
            return Err(EspeakFailure::WrongAuthority);
        }
        if streaming {
            streaming::StreamLimits::from_placement(placement).map(|limits| limits.maximum_bytes)
        } else {
            if placement.configuration.len() != 2 {
                return Err(EspeakFailure::InvalidLimits);
            }
            let Some(configuration) = placement
                .configuration
                .iter()
                .find(|entry| entry.key == "maximum-output-bytes")
            else {
                return Err(EspeakFailure::InvalidLimits);
            };
            match (&*configuration.key, &configuration.value) {
                ("maximum-output-bytes", ConfigurationValue::U64(bytes))
                    if *bytes > 0 && *bytes <= u64::from(conduit_tongues::MAXIMUM_PCM_BYTES) =>
                {
                    Ok(*bytes as u32)
                }
                _ => Err(EspeakFailure::InvalidLimits),
            }
        }
    }

    /// `pcm` is caller-owned storage admitted before Play. No playback occurs.
    pub fn synthesize_into(
        &self,
        placement: &PlannedGear,
        text: &[u8],
        pcm: &mut Vec<u8>,
        cancelled: impl Fn() -> bool,
    ) -> Result<EspeakSynthesisReceipt, EspeakFailure> {
        pcm.clear();
        let maximum = self.validate_placement(placement)?;
        if pcm.capacity() < maximum as usize {
            return Err(EspeakFailure::InvalidLimits);
        }
        if text.is_empty()
            || text.len() > conduit_tongues::MAXIMUM_TEXT_BYTES as usize
            || std::str::from_utf8(text).is_err()
            || text.contains(&0)
        {
            return Err(EspeakFailure::InvalidText);
        }
        if cancelled() {
            return Err(EspeakFailure::Cancelled);
        }
        self.discovery.verify()?;
        let report = run_process(
            &ProcessRequest {
                program: &self.discovery.executable,
                arguments: &self.arguments,
                environment: &self.environment,
                stdin: text,
                maximum_stdout_bytes: maximum as usize + 44,
                maximum_stderr_bytes: 4096,
                timeout: self.timeout,
                require_process_group: true,
            },
            &cancelled,
        )
        .map_err(|error| match error {
            ProcessError::Launch(_) => EspeakFailure::SpawnFailed,
            _ => EspeakFailure::InvalidProvider,
        })?;
        match report.terminal {
            ProcessTerminal::Cancelled => return Err(EspeakFailure::Cancelled),
            ProcessTerminal::TimedOut => return Err(EspeakFailure::Timeout),
            ProcessTerminal::Exited(status) if status.success() => {}
            _ => return Err(EspeakFailure::ProviderLost),
        }
        if report.stdout.observed_bytes > report.stdout.retained.len() as u64
            || report.stderr.observed_bytes > 4096
        {
            return Err(EspeakFailure::OutputOverflow);
        }
        // Revalidate before exposing any bytes: replacement during execution invalidates the effect.
        self.discovery.verify()?;
        if cancelled() {
            return Err(EspeakFailure::Cancelled);
        }
        let payload = wav::pcm(&report.stdout.retained, maximum as usize)?;
        let receipt = EspeakSynthesisReceipt {
            schema: "conduit.host/espeak-synthesis-receipt@1",
            provider_sha256: self.discovery.provider_sha256.clone(),
            text_sha256: format!("{:x}", Sha256::digest(text)),
            pcm_sha256: format!("{:x}", Sha256::digest(payload)),
            pcm_bytes: payload.len() as u32,
            maximum_pcm_bytes: maximum,
            process_timeout_millis: self.timeout.as_millis() as u64,
        };
        pcm.extend_from_slice(payload);
        Ok(receipt)
    }
}
#[cfg(test)]
mod tests;
