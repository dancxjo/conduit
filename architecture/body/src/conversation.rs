//! Bounded, provenance-bearing Body truth suitable for conversational projection.

use alloc::{string::String, vec::Vec};
use conduit_core::{
    ActivePlayId, HostId, LineAvailability, LineAvailabilitySign, Plan, PlanId, SignId,
};
use serde::{Deserialize, Serialize};

use crate::{
    Body, BodyId, BodyState, HostPresenceState, HostPresenceTable, Wake, WakeId, WakePlanState,
};

pub const BODY_CONVERSATION_CONTEXT_VALUE_KIND: &str = "body/conversation-context@2";
pub const MAXIMUM_CONVERSATION_HOSTS: usize = crate::MAX_BODY_PARTS;
pub const MAXIMUM_CONVERSATION_FORMS: usize = crate::MAX_BODY_FORMS;
pub const MAXIMUM_CONVERSATION_LINES: usize = 32;
pub const MAXIMUM_CONVERSATION_SIGNS: usize = 16;
pub const MAXIMUM_BODY_DISPLAY_NAME_BYTES: usize = 128;
pub const MAXIMUM_BODY_CONVERSATION_CONTEXT_BYTES: usize = 32_768;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BodyConversationContextBasis {
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub wake_sequence: u64,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BodyConversationHost {
    pub host_id: HostId,
    pub present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BodyConversationLine {
    pub line_id: String,
    pub source_host_id: HostId,
    pub target_host_id: HostId,
    pub availability: Option<LineAvailability>,
    pub availability_sign_id: Option<SignId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BodyConversationContext {
    pub schema: String,
    pub display_name: String,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub wake_sequence: u64,
    pub basis: BodyConversationContextBasis,
    pub hosts: Vec<BodyConversationHost>,
    pub active_forms: Vec<String>,
    pub current_plan_id: Option<PlanId>,
    pub active_play_id: Option<ActivePlayId>,
    pub lines: Vec<BodyConversationLine>,
    pub recent_sign_ids: Vec<SignId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyConversationContextRefusal {
    InvalidDisplayName,
    BodyNotAwake,
    WrongWake,
    WrongPresenceBody,
    WrongPlan,
    HostCapacityExceeded,
    FormCapacityExceeded,
    LineCapacityExceeded,
    InvalidLine,
}

impl BodyConversationContext {
    pub fn from_current_truth(
        display_name: &str,
        body: &Body,
        wake: &Wake,
        presence: &HostPresenceTable,
        plan: Option<&Plan>,
        line_availability: &[LineAvailabilitySign],
    ) -> Result<Self, BodyConversationContextRefusal> {
        Self::from_current_truth_at_revision(
            display_name,
            body,
            wake,
            presence,
            plan,
            line_availability,
            0,
        )
    }

    pub fn from_current_truth_at_revision(
        display_name: &str,
        body: &Body,
        wake: &Wake,
        presence: &HostPresenceTable,
        plan: Option<&Plan>,
        line_availability: &[LineAvailabilitySign],
        revision: u64,
    ) -> Result<Self, BodyConversationContextRefusal> {
        if display_name.is_empty() || display_name.len() > MAXIMUM_BODY_DISPLAY_NAME_BYTES {
            return Err(BodyConversationContextRefusal::InvalidDisplayName);
        }
        if body.state
            != (BodyState::Awake {
                wake_id: wake.wake_id.clone(),
            })
        {
            return Err(BodyConversationContextRefusal::BodyNotAwake);
        }
        if wake.body_id != body.body_id || wake.workload_revision != body.workload_revision {
            return Err(BodyConversationContextRefusal::WrongWake);
        }
        if presence.body_id != body.body_id {
            return Err(BodyConversationContextRefusal::WrongPresenceBody);
        }
        if presence.leases.len() > MAXIMUM_CONVERSATION_HOSTS {
            return Err(BodyConversationContextRefusal::HostCapacityExceeded);
        }
        if wake.workset.len() > MAXIMUM_CONVERSATION_FORMS {
            return Err(BodyConversationContextRefusal::FormCapacityExceeded);
        }
        let playing = wake
            .plans
            .iter()
            .find(|candidate| candidate.state == WakePlanState::Playing);
        let (current_plan_id, active_play_id) = match (playing, plan) {
            (Some(current), Some(plan)) if current.plan_id == plan.plan_id => {
                (Some(plan.plan_id.clone()), current.active_play_id.clone())
            }
            (None, None) => (None, None),
            _ => return Err(BodyConversationContextRefusal::WrongPlan),
        };
        let mut lines = Vec::new();
        if let Some(plan) = plan {
            for admitted in plan
                .fragments
                .iter()
                .flat_map(|fragment| fragment.connections.iter())
                .flat_map(|connection| connection.admitted_lines.iter())
            {
                if lines
                    .iter()
                    .any(|line: &BodyConversationLine| line.line_id == admitted.line_id.as_str())
                {
                    continue;
                }
                if lines.len() == MAXIMUM_CONVERSATION_LINES {
                    return Err(BodyConversationContextRefusal::LineCapacityExceeded);
                }
                let availability = line_availability.iter().rev().find(|sign| {
                    sign.line_id == admitted.line_id
                        && sign.binding_id == admitted.binding.binding_id
                });
                lines.push(BodyConversationLine {
                    line_id: admitted.line_id.as_str().into(),
                    source_host_id: admitted.binding.source.host_id.clone(),
                    target_host_id: admitted.binding.sink.host_id.clone(),
                    availability: availability.map(|sign| sign.availability),
                    availability_sign_id: availability.map(|sign| sign.sign_id.clone()),
                });
            }
        } else if !line_availability.is_empty() {
            return Err(BodyConversationContextRefusal::InvalidLine);
        }
        let mut signs: Vec<_> = wake
            .sign_ids
            .iter()
            .rev()
            .take(MAXIMUM_CONVERSATION_SIGNS)
            .cloned()
            .collect();
        signs.reverse();
        Ok(Self {
            schema: "conduit.body/conversation-context-value@2".into(),
            display_name: display_name.into(),
            body_id: body.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            wake_sequence: wake.wake_sequence,
            basis: BodyConversationContextBasis {
                body_id: body.body_id.clone(),
                wake_id: wake.wake_id.clone(),
                wake_sequence: wake.wake_sequence,
                revision,
            },
            hosts: presence
                .leases
                .iter()
                .map(|lease| BodyConversationHost {
                    host_id: lease.host_id.clone(),
                    present: lease.state == HostPresenceState::Available,
                })
                .collect(),
            active_forms: wake
                .workset
                .forms()
                .iter()
                .map(|form| form.source_document_id.as_str().into())
                .collect(),
            current_plan_id,
            active_play_id,
            lines,
            recent_sign_ids: signs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HostPresenceClock, HostPresenceClockScale};
    use alloc::{format, vec};
    use conduit_core::{CheckedFormId, SignId, SourceDocumentId};

    fn awake(name: &str) -> (Body, Wake, HostPresenceTable) {
        let body = Body::born(
            SourceDocumentId::from(format!("source/{name}")),
            CheckedFormId::from(format!("checked/{name}")),
            1,
            SignId::from(format!("sign/{name}/born")),
        )
        .unwrap();
        let (body, wake) = body
            .wake(4, SignId::from(format!("sign/{name}/wake")))
            .unwrap();
        let presence = HostPresenceTable::new(
            body.body_id.clone(),
            HostPresenceClock::new(
                "clock/body-chat".into(),
                HostPresenceClockScale::Milliseconds,
                1,
                0,
            )
            .unwrap(),
            30_000,
        )
        .unwrap();
        (body, wake, presence)
    }

    #[test]
    fn projection_binds_matching_current_body_wake_presence_and_canonical_bytes() {
        let (body, wake, presence) = awake("roseau");
        let context = BodyConversationContext::from_current_truth(
            "Roseau",
            &body,
            &wake,
            &presence,
            None,
            &[],
        )
        .unwrap();
        assert_eq!(context.wake_sequence, 4);
        assert_eq!(context.active_forms, vec!["source/roseau"]);
    }

    #[test]
    fn projection_refuses_presence_from_another_body() {
        let (body, wake, _) = awake("roseau");
        let (_, _, other_presence) = awake("other");
        assert_eq!(
            BodyConversationContext::from_current_truth(
                "Roseau",
                &body,
                &wake,
                &other_presence,
                None,
                &[]
            ),
            Err(BodyConversationContextRefusal::WrongPresenceBody)
        );
    }
}
