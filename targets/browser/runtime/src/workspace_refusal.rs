//! Stable machine-readable categories at the workspace lifecycle boundary.
use conduit_body::BodyLifecycleSessionError;

pub(super) struct Refusal {
    pub code: String,
    pub message: String,
}

impl Refusal {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<BodyLifecycleSessionError> for Refusal {
    fn from(error: BodyLifecycleSessionError) -> Self {
        use BodyLifecycleSessionError::*;
        let code = match error {
            Biography(error) => format!("Biography.{error:?}"),
            Admission(error) => format!("Admission.{error:?}"),
            Membership(error) => format!("Membership.{error:?}"),
            Lifecycle(error) => format!("Lifecycle.{error:?}"),
            Plan(error) => format!("Plan.{error:?}"),
            NotLulled => "NotLulled".into(),
            NoProposal => "NoProposal".into(),
            AlreadyPlaying => "AlreadyPlaying".into(),
            StaleHost => "StaleHost".into(),
            StalePlay => "StalePlay".into(),
            StaleWorkload => "StaleWorkload".into(),
            UninstalledForm => "UninstalledForm".into(),
            SequenceExhausted => "SequenceExhausted".into(),
            UnreconciledWake => "UnreconciledWake".into(),
            ArchivePersistenceRequired => "HistoryArchivePersistenceRequired".into(),
        };
        Self {
            code,
            message: format!("Workspace refused: {error:?}"),
        }
    }
}

impl From<String> for Refusal {
    fn from(message: String) -> Self {
        Self::new("AdmissionRefused", message)
    }
}

impl From<&str> for Refusal {
    fn from(message: &str) -> Self {
        Self::new("InvalidState", message)
    }
}
