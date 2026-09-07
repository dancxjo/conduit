//! Portable, authority-checked Body lifecycle administration.

use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyAdministrativeIntent {
    Birth,
    Join,
    Leave,
    AddForm,
    RemoveForm,
    Wake,
    Lull,
    AcquireResource,
    ReleaseResource,
    GrantAuthority,
    RevokeAuthority,
    Inspect,
    RecoverContinuity,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyAdministrativeRequest {
    pub request_id: String,
    pub subject: String,
    pub authority: Option<String>,
    pub expected_revision: u64,
    pub intent: BodyAdministrativeIntent,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyAdministrativeRefusal {
    InvalidIdentity,
    Denied,
    Stale,
    Conflict,
    Replay,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyAdministrativeResult {
    pub request_id: String,
    pub resulting_revision: u64,
    pub evidence_id: String,
}

pub fn administer(
    request: &BodyAdministrativeRequest,
    current_revision: u64,
    used_requests: &[String],
) -> Result<BodyAdministrativeResult, BodyAdministrativeRefusal> {
    if request.request_id.is_empty() || request.subject.is_empty() {
        return Err(BodyAdministrativeRefusal::InvalidIdentity);
    }
    if used_requests.iter().any(|used| used == &request.request_id) {
        return Err(BodyAdministrativeRefusal::Replay);
    }
    if request.expected_revision != current_revision {
        return Err(BodyAdministrativeRefusal::Stale);
    }
    if request
        .authority
        .as_ref()
        .is_none_or(|authority| authority.is_empty())
        && request.intent != BodyAdministrativeIntent::Inspect
    {
        return Err(BodyAdministrativeRefusal::Denied);
    }
    let resulting_revision = if request.intent == BodyAdministrativeIntent::Inspect {
        current_revision
    } else {
        current_revision
            .checked_add(1)
            .ok_or(BodyAdministrativeRefusal::Conflict)?
    };
    Ok(BodyAdministrativeResult {
        request_id: request.request_id.clone(),
        resulting_revision,
        evidence_id: alloc::format!("admin-evidence:{}", request.request_id),
    })
}
