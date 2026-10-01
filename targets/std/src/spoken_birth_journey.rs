//! One screen-free Tongues Host crossing one canonical Body Birth boundary.

use conduit_body::{Body, BodyId};
use conduit_core::{BootId, HostId, SignId};
use conduit_tongues::{OutputCondition, SpeechFault, SpeechOutcome, SpeechRunReceipt};
use serde::{Deserialize, Serialize};

const MAX_ACTION_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SpokenEventOwner {
    HostBootstrap { host_id: HostId, boot_id: BootId },
    Body { body_id: BodyId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenEventEvidence {
    pub event_id: String,
    pub owner: SpokenEventOwner,
    pub producing_host_id: HostId,
    pub producing_boot_id: BootId,
    pub speech: SpeechRunReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenBirthJourneyEvidence {
    pub schema: String,
    pub graphical_display_advertised: bool,
    pub confirmation_action_id: String,
    pub body_id: BodyId,
    pub birth_sign_id: SignId,
    pub bootstrap: SpokenEventEvidence,
    pub body_manifestation: SpokenEventEvidence,
    /// Artifact production is not evidence that a person heard it.
    pub human_hearing_observed: bool,
}

impl SpokenBirthJourneyEvidence {
    pub const SCHEMA: &'static str = "conduit.std/tongues-spoken-birth-journey@1";

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != Self::SCHEMA
            || self.graphical_display_advertised
            || self.confirmation_action_id.is_empty()
            || self.confirmation_action_id.len() > MAX_ACTION_ID_BYTES
            || self.human_hearing_observed
        {
            return Err("invalid Tongues spoken Birth journey envelope".into());
        }
        let SpokenEventOwner::HostBootstrap { host_id, boot_id } = &self.bootstrap.owner else {
            return Err("bootstrap speech is not Host-owned".into());
        };
        if self.bootstrap.producing_host_id != *host_id
            || self.bootstrap.producing_boot_id != *boot_id
            || self.body_manifestation.producing_host_id != *host_id
            || self.body_manifestation.producing_boot_id != *boot_id
        {
            return Err("spoken events do not share one exact Host observation".into());
        }
        if self.body_manifestation.owner
            != (SpokenEventOwner::Body {
                body_id: self.body_id.clone(),
            })
        {
            return Err("post-Birth speech is not owned by the born Body".into());
        }
        validate_event(&self.bootstrap)?;
        validate_event(&self.body_manifestation)
    }
}

/// Consuming this value is the exact one-invocation bound.
pub struct ConfirmBirthAction(String);

impl ConfirmBirthAction {
    pub fn new(action_id: impl Into<String>) -> Result<Self, String> {
        let action_id = action_id.into();
        if action_id.is_empty() || action_id.len() > MAX_ACTION_ID_BYTES {
            return Err("invalid Birth confirmation action identity".into());
        }
        Ok(Self(action_id))
    }
}

pub struct AwaitingBirth {
    host_id: HostId,
    boot_id: BootId,
    bootstrap: SpokenEventEvidence,
}

/// Execute Host-owned bootstrap speech through the in-process Tongues library.
/// No Body has been constructed when this function runs.
pub fn begin(bootstrap_text: &str) -> Result<AwaitingBirth, String> {
    let fixture = headless_fixture()?;
    let speech = conduit_tongues::run_speech_text(
        bootstrap_text,
        OutputCondition::DegradedWavArtifact,
        SpeechFault::None,
    )?;
    let host_id = fixture.advertisement.host_id;
    let boot_id = fixture.advertisement.boot_id;
    let bootstrap = SpokenEventEvidence {
        event_id: "event/tongues-bootstrap-spoken".into(),
        owner: SpokenEventOwner::HostBootstrap {
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
        },
        producing_host_id: host_id.clone(),
        producing_boot_id: boot_id.clone(),
        speech,
    };
    validate_event(&bootstrap)?;
    Ok(AwaitingBirth {
        host_id,
        boot_id,
        bootstrap,
    })
}

impl AwaitingBirth {
    /// Consume one action, perform canonical Birth, and only then invoke
    /// Tongues for the Body-attributed manifestation.
    pub fn confirm(
        self,
        action: ConfirmBirthAction,
        source_document_id: conduit_core::SourceDocumentId,
        checked_form_id: conduit_core::CheckedFormId,
        birth_sequence: u64,
        birth_sign_id: SignId,
        body_text: &str,
    ) -> Result<(Body, SpokenBirthJourneyEvidence), String> {
        let body = Body::born(
            source_document_id,
            checked_form_id,
            birth_sequence,
            birth_sign_id.clone(),
        )
        .map_err(|error| format!("canonical Birth refused: {error}"))?;
        let speech = conduit_tongues::run_speech_text(
            body_text,
            OutputCondition::DegradedWavArtifact,
            SpeechFault::None,
        )?;
        let evidence = SpokenBirthJourneyEvidence {
            schema: SpokenBirthJourneyEvidence::SCHEMA.into(),
            graphical_display_advertised: false,
            confirmation_action_id: action.0,
            body_id: body.body_id.clone(),
            birth_sign_id,
            bootstrap: self.bootstrap,
            body_manifestation: SpokenEventEvidence {
                event_id: "event/tongues-body-spoken".into(),
                owner: SpokenEventOwner::Body {
                    body_id: body.body_id.clone(),
                },
                producing_host_id: self.host_id,
                producing_boot_id: self.boot_id,
                speech,
            },
            human_hearing_observed: false,
        };
        evidence.validate()?;
        Ok((body, evidence))
    }
}

fn headless_fixture() -> Result<conduit_tongues::SpeechHostFixture, String> {
    let fixture = conduit_tongues::speech_host_fixture(OutputCondition::DegradedWavArtifact);
    if fixture.advertisement.capabilities.iter().any(|offer| {
        let kind = offer.kind_id.as_str();
        kind == conduit_presentation::RENDERER_KIND
            || kind.starts_with("display/")
            || kind.starts_with("graphics/")
            || kind.contains("graphics-scene")
    }) {
        return Err("Tongues bootstrap Host advertises graphical display work".into());
    }
    Ok(fixture)
}

fn validate_event(event: &SpokenEventEvidence) -> Result<(), String> {
    if event.event_id.is_empty()
        || event.speech.plan_id.is_empty()
        || event.speech.text_sha256.len() != 64
        || event.speech.sign_count == 0
        || event.speech.kernel_event_count == 0
        || !matches!(
            event.speech.outcome,
            SpeechOutcome::WavArtifact {
                wav_bytes: 1..,
                ref wav_sha256,
                ref pcm_sha256,
            } if wav_sha256.len() == 64 && pcm_sha256.len() == 64
        )
    {
        return Err("Tongues spoken event evidence is incomplete".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_action_crosses_from_host_owned_tongues_speech_to_body_owned_speech() {
        let awaiting = begin("No Body exists yet. Confirm to begin.").unwrap();
        let (body, evidence) = awaiting
            .confirm(
                ConfirmBirthAction::new("action/confirm-birth").unwrap(),
                "source/tongues-spoken-birth".into(),
                "checked/tongues-spoken-birth".into(),
                1,
                SignId::from("sign/tongues-spoken-birth/born"),
                "I am now speaking as the born Body.",
            )
            .unwrap();

        assert_eq!(evidence.body_id, body.body_id);
        assert_ne!(
            evidence.bootstrap.speech.text_sha256,
            evidence.body_manifestation.speech.text_sha256
        );
        assert!(matches!(
            body.events.first(),
            Some(conduit_body::BodyLifecycleEvent::Born { sign_id, .. })
                if sign_id == &evidence.birth_sign_id
        ));
        evidence.validate().unwrap();
    }
}
