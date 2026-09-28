//! Strict standalone envelopes for retained execution truth.

use alloc::format;
use alloc::string::String;
use conduit_core::{
    bind_active_play, bind_sign, verify_plan, ActivePlayIdentity, Observation, ObservationKind,
    Plan, SignIdentity,
};
use serde::{Deserialize, Serialize};

use crate::validation::{
    plan_contains_connection, plan_contains_placement, validate_lifecycle, validate_play_children,
};
use crate::PlayReport;

pub const PLAN_ARTIFACT_SCHEMA: &str = "conduit.plan/artifact@1";
pub const PLAY_ARTIFACT_SCHEMA: &str = "conduit.play/artifact@1";
pub const SIGN_ARTIFACT_SCHEMA: &str = "conduit.sign/artifact@1";

const MAXIMUM_FRAGMENTS: usize = 64;
const MAXIMUM_PLACEMENTS: usize = 4096;
const MAXIMUM_CONNECTIONS: usize = 4096;
const MAXIMUM_FAILURE_MESSAGE_BYTES: usize = 4096;
const MAXIMUM_SIGN_VALUE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanArtifact {
    pub schema: String,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayArtifact {
    pub schema: String,
    pub identity: ActivePlayIdentity,
    pub plan: Plan,
    pub play: PlayReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignArtifact {
    pub schema: String,
    pub identity: SignIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Plan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play: Option<PlayArtifact>,
    pub sign: Observation,
}

pub fn validate_plan_artifact(artifact: &PlanArtifact) -> Result<(), String> {
    if artifact.schema != PLAN_ARTIFACT_SCHEMA {
        return Err(format!(
            "unsupported Plan artifact schema: {}",
            artifact.schema
        ));
    }
    validate_standalone_plan(&artifact.plan)
}

pub fn validate_play_artifact(artifact: &PlayArtifact) -> Result<(), String> {
    if artifact.schema != PLAY_ARTIFACT_SCHEMA {
        return Err(format!(
            "unsupported Play artifact schema: {}",
            artifact.schema
        ));
    }
    validate_standalone_plan(&artifact.plan)?;
    let rebound = bind_active_play(
        &artifact.identity.plan_id,
        &artifact.identity.host_id,
        &artifact.identity.boot_id,
        artifact.identity.play_sequence,
    );
    if rebound != artifact.identity {
        return Err("Play artifact active identity is not canonical".into());
    }
    if artifact.play.active_play_id != artifact.identity.active_play_id
        || artifact.play.plan_id != artifact.identity.plan_id
        || artifact.play.host_id != artifact.identity.host_id
        || artifact.play.boot_id != artifact.identity.boot_id
        || artifact.plan.plan_id != artifact.identity.plan_id
    {
        return Err("Play artifact identities disagree".into());
    }
    if !artifact.plan.fragments.iter().any(|fragment| {
        fragment.host_id == artifact.identity.host_id
            && fragment.boot_id == artifact.identity.boot_id
    }) {
        return Err("Play artifact host/boot has no exact Plan fragment".into());
    }
    validate_failure_message(artifact.play.failure_message.as_deref())?;
    for placement in &artifact.play.placements {
        validate_failure_message(placement.failure_message.as_deref())?;
    }
    for connection in &artifact.play.connections {
        validate_failure_message(connection.failure_message.as_deref())?;
    }
    validate_lifecycle(
        artifact.play.lifecycle,
        artifact.play.terminal_disposition,
        artifact.play.failure_message.as_deref(),
    )?;
    validate_play_children(&artifact.plan, &artifact.play)
}

pub fn validate_sign_artifact(artifact: &SignArtifact) -> Result<(), String> {
    if artifact.schema != SIGN_ARTIFACT_SCHEMA {
        return Err(format!(
            "unsupported Sign artifact schema: {}",
            artifact.schema
        ));
    }
    let rebound = bind_sign(
        &artifact.identity.host_id,
        &artifact.identity.boot_id,
        artifact.identity.active_play_id.as_ref(),
        artifact.identity.sequence,
    );
    if rebound != artifact.identity {
        return Err("Sign artifact identity is not canonical".into());
    }
    if artifact.sign.sign_id != artifact.identity.sign_id
        || artifact.sign.host_id != artifact.identity.host_id
        || artifact.sign.boot_id != artifact.identity.boot_id
        || artifact.sign.active_play_id != artifact.identity.active_play_id
    {
        return Err("Sign artifact identities disagree".into());
    }

    let contextual_plan = match (&artifact.plan, &artifact.play) {
        (Some(plan), Some(play)) => {
            validate_standalone_plan(plan)?;
            validate_play_artifact(play)?;
            if plan != &play.plan {
                return Err("Sign artifact Plan contexts disagree".into());
            }
            Some(plan)
        }
        (Some(plan), None) => {
            validate_standalone_plan(plan)?;
            Some(plan)
        }
        (None, Some(play)) => {
            validate_play_artifact(play)?;
            Some(&play.plan)
        }
        (None, None) => None,
    };

    if let Some(active_play_id) = &artifact.identity.active_play_id {
        let play = artifact
            .play
            .as_ref()
            .ok_or("active-play Sign artifact lacks its exact Play context")?;
        if &play.identity.active_play_id != active_play_id {
            return Err("Sign artifact names a different Play context".into());
        }
    } else if artifact.play.is_some() {
        return Err("host-level Sign artifact cannot carry a Play context".into());
    }

    if let Some(plan_id) = &artifact.sign.plan_id {
        let plan =
            contextual_plan.ok_or("Plan-bound Sign artifact lacks its exact Plan context")?;
        if &plan.plan_id != plan_id {
            return Err("Sign artifact names a different Plan context".into());
        }
    } else if contextual_plan.is_some() {
        return Err("non-Plan Sign artifact cannot carry a Plan context".into());
    }
    if artifact.sign.presentation_id.is_some() && artifact.sign.active_play_id.is_none() {
        return Err("presentation Sign artifact has no active Play identity".into());
    }
    if let Some(placement_id) = &artifact.sign.placement_id {
        let plan = contextual_plan.ok_or("placement Sign lacks Plan context")?;
        if !plan_contains_placement(plan, placement_id) {
            return Err("Sign artifact names an unreported placement".into());
        }
    }
    if let Some(connection_id) = &artifact.sign.connection_id {
        let plan = contextual_plan.ok_or("connection Sign lacks Plan context")?;
        if !plan_contains_connection(plan, connection_id) {
            return Err("Sign artifact names an unreported connection".into());
        }
    }
    validate_sign_payload(&artifact.sign.kind)
}

fn validate_standalone_plan(plan: &Plan) -> Result<(), String> {
    if !verify_plan(plan) {
        return Err(format!(
            "Plan {} failed exact verification",
            plan.plan_id.as_str()
        ));
    }
    let placements = plan
        .fragments
        .iter()
        .try_fold(0usize, |count, fragment| {
            count.checked_add(fragment.placements.len())
        })
        .ok_or("Plan artifact placement count overflowed")?;
    let connections = plan
        .fragments
        .iter()
        .try_fold(0usize, |count, fragment| {
            count.checked_add(fragment.connections.len())
        })
        .ok_or("Plan artifact connection count overflowed")?;
    if plan.fragments.len() > MAXIMUM_FRAGMENTS
        || placements > MAXIMUM_PLACEMENTS
        || connections > MAXIMUM_CONNECTIONS
    {
        return Err("Plan artifact exceeds its finite collection bounds".into());
    }
    Ok(())
}

fn validate_failure_message(message: Option<&str>) -> Result<(), String> {
    if message.is_some_and(|message| message.len() > MAXIMUM_FAILURE_MESSAGE_BYTES) {
        return Err("Play artifact failure message exceeds its finite bound".into());
    }
    Ok(())
}

fn validate_sign_payload(kind: &ObservationKind) -> Result<(), String> {
    match kind {
        ObservationKind::ValueProduced { value }
        | ObservationKind::ValueAccepted { value }
        | ObservationKind::ValuePresented { value }
            if value.encoded.len() > MAXIMUM_SIGN_VALUE_BYTES =>
        {
            Err("Sign artifact value exceeds its finite bound".into())
        }
        ObservationKind::Failure { message, .. }
            if message
                .as_ref()
                .is_some_and(|message| message.len() > MAXIMUM_FAILURE_MESSAGE_BYTES) =>
        {
            Err("Sign artifact failure message exceeds its finite bound".into())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use conduit_core::{bind_active_play, bind_sign, Observation, ObservationKind};
    use conduit_signal_conformance::exact_std_pico_usb_plan;

    use super::*;
    use crate::PlanLifecycle;

    fn exact_artifacts() -> (PlanArtifact, PlayArtifact, SignArtifact) {
        let plan = exact_std_pico_usb_plan().unwrap().plan;
        let fragment = plan.fragments.first().unwrap();
        let identity = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 7);
        let play = PlayArtifact {
            schema: PLAY_ARTIFACT_SCHEMA.into(),
            identity: identity.clone(),
            plan: plan.clone(),
            play: PlayReport {
                active_play_id: identity.active_play_id.clone(),
                plan_id: plan.plan_id.clone(),
                host_id: identity.host_id.clone(),
                boot_id: identity.boot_id.clone(),
                lifecycle: PlanLifecycle::Active,
                terminal_disposition: None,
                failure_message: None,
                placements: vec![],
                connections: vec![],
            },
        };
        let sign_identity = bind_sign(
            &identity.host_id,
            &identity.boot_id,
            Some(&identity.active_play_id),
            11,
        );
        let sign = SignArtifact {
            schema: SIGN_ARTIFACT_SCHEMA.into(),
            identity: sign_identity.clone(),
            plan: None,
            play: Some(play.clone()),
            sign: Observation {
                sign_id: sign_identity.sign_id,
                active_play_id: Some(identity.active_play_id),
                presentation_id: None,
                host_id: identity.host_id,
                boot_id: identity.boot_id,
                plan_id: Some(plan.plan_id.clone()),
                placement_id: None,
                connection_id: None,
                kind: ObservationKind::PlanPlayStarted,
            },
        };
        (
            PlanArtifact {
                schema: PLAN_ARTIFACT_SCHEMA.into(),
                plan,
            },
            play,
            sign,
        )
    }

    #[test]
    fn exact_envelopes_validate_without_conflating_artifact_kinds() {
        let (plan, play, sign) = exact_artifacts();
        validate_plan_artifact(&plan).unwrap();
        validate_play_artifact(&play).unwrap();
        validate_sign_artifact(&sign).unwrap();

        let encoded = serde_json::to_vec(&plan).unwrap();
        assert!(serde_json::from_slice::<PlayArtifact>(&encoded).is_err());
        assert!(serde_json::from_slice::<SignArtifact>(&encoded).is_err());
    }

    #[test]
    fn versions_unknown_fields_and_stale_nested_identities_fail_closed() {
        let (mut plan, mut play, mut sign) = exact_artifacts();
        plan.schema = "conduit.plan/artifact@2".into();
        assert!(validate_plan_artifact(&plan)
            .unwrap_err()
            .contains("schema"));

        let mut value = serde_json::to_value(&play).unwrap();
        value["future"] = serde_json::json!(true);
        assert!(serde_json::from_value::<PlayArtifact>(value).is_err());

        play.play.active_play_id = conduit_core::ActivePlayId::from("stale");
        assert_eq!(
            validate_play_artifact(&play).unwrap_err(),
            "Play artifact identities disagree"
        );

        sign.sign.sign_id = conduit_core::SignId::from("stale");
        assert_eq!(
            validate_sign_artifact(&sign).unwrap_err(),
            "Sign artifact identities disagree"
        );
    }

    #[test]
    fn missing_context_and_oversized_payloads_remain_distinct_refusals() {
        let (_, _, mut sign) = exact_artifacts();
        sign.play = None;
        assert_eq!(
            validate_sign_artifact(&sign).unwrap_err(),
            "active-play Sign artifact lacks its exact Play context"
        );

        let (_, _, mut sign) = exact_artifacts();
        sign.sign.kind = ObservationKind::ValueProduced {
            value: conduit_core::ValuePayload {
                value_kind: conduit_core::KindId::from("data/bytes"),
                encoded: vec![0; MAXIMUM_SIGN_VALUE_BYTES + 1],
            },
        };
        assert_eq!(
            validate_sign_artifact(&sign).unwrap_err(),
            "Sign artifact value exceeds its finite bound"
        );
    }
}
