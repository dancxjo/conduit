//! Progress for a fresh private OS request on the SAME serving Root.
//! The opaque result owns uncertain carriers; diagnostics confer no Start/Ready.
use super::{body::HostSource, DurableHostRuntime, Request, Response, PROTOCOL};
use crate::durable_host::source_revision::source_execution;
use std::path::Path;

#[derive(Default)]
pub(super) struct SourceExecutionProgress {
    attempted: bool,
    retained: Option<source_execution::RetainedExecution>,
}

impl DurableHostRuntime {
    pub(super) fn progress_selected_source_execution(&mut self, root: &Path) -> Result<(), String> {
        if self.source_execution.attempted || !source_execution::pending_request(root)? {
            return Ok(());
        }
        // Preclaim refusals are not automatically retried either. The file's
        // exact Boot and durable claim separately prevent restart replay.
        self.source_execution.attempted = true;
        let result = match &mut self.host {
            HostSource::Body {
                owner,
                running: None,
                root: actual,
            } if actual == root => owner.execute_selected_source(root),
            _ => Err("fresh Source execution requires the same idle Body owner".into()),
        };
        match result {
            Ok(retained) => {
                self.source_execution.retained = retained;
                if let Some(attempt) = &self.source_execution.retained {
                    for outcome in &attempt.outcomes {
                        eprintln!("selected Source execution outcome: {outcome}");
                    }
                }
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

/// A bounded ordinary status round trip wakes an idle service. A lost wakeup
/// reply leaves the already queued request outcome unknown; never resubmit it.
pub(crate) fn wake_selected_source_execution(state: &Path) -> Result<(), String> {
    let mut token = super::read_secret(&state.join("control.token"))?;
    let request = Request::Status {
        protocol: PROTOCOL,
        token: token.to_vec(),
    };
    token.fill(0);
    match super::body::call(state, request) {
        Ok(Response::Status {
            protocol: PROTOCOL, ..
        }) => Ok(()),
        _ => Err(
            "Source request queued; service wakeup outcome unknown; do not automatically resubmit"
                .into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn invalid_request(root: &Path) {
        let directory = root.join("source-execution");
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(directory.join("pending.json"), b"not a request").unwrap();
        std::fs::set_permissions(
            directory.join("pending.json"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }

    #[test]
    #[ignore = "requires explicit digest-pinned product Source worker artifact"]
    fn original_custody_claims_once_and_survives_failed_request_without_execution() {
        let (owner, root, source) = crate::durable_host::source_revision::real_owner_state();
        let before = owner.selected_source_current_basis();
        invalid_request(&root);
        let mut runtime = DurableHostRuntime::from_selected_owner(
            "fixture/source".into(),
            "fixture/image".into(),
            owner,
            &root,
        );
        let refusal = runtime.progress_selected_source_execution(&root).unwrap_err();
        assert!(root.join("source-execution/claimed.json").exists(), "{refusal}");
        assert!(root.join("source-execution/claimed.json").exists());
        assert!(!root.join("source-execution/pending.json").exists());
        runtime.progress_selected_source_execution(&root).unwrap();
        let HostSource::Body { owner, .. } = &mut runtime.host else {
            panic!("owner lost")
        };
        assert_eq!(owner.selected_source_current_basis(), before);
        // This verifies the actual still-installed original field again; the
        // claimed slot is never replayed, and there is no disk-Source fallback.
        assert!(owner.execute_selected_source(&root).unwrap().is_none());
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_file(source).unwrap();
    }

    #[test]
    #[ignore = "requires explicit digest-pinned product Source worker artifact"]
    fn replaced_policy_cannot_launder_same_source_into_original_caller_custody() {
        let (owner, root, source) = crate::durable_host::source_revision::real_owner_state();
        let before = owner.selected_source_current_basis();
        let path = root.join("source-check-selection.json");
        let original = std::fs::read(&path).unwrap();
        let mut changed: serde_json::Value = serde_json::from_slice(&original).unwrap();
        changed["caller_workspace_bytes"] =
            (changed["caller_workspace_bytes"].as_u64().unwrap() + 1).into();
        std::fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        invalid_request(&root);
        let mut runtime = DurableHostRuntime::from_selected_owner(
            "fixture/source".into(),
            "fixture/image".into(),
            owner,
            &root,
        );
        assert!(runtime
            .progress_selected_source_execution(&root)
            .unwrap_err()
            .contains("original retained preparation identity"));
        assert!(root.join("source-execution/pending.json").exists());
        assert!(!root.join("source-execution/claimed.json").exists());
        std::fs::write(&path, original).unwrap();
        runtime.progress_selected_source_execution(&root).unwrap();
        assert!(
            !root.join("source-execution/claimed.json").exists(),
            "no automatic preclaim retry"
        );
        let HostSource::Body { owner, .. } = &mut runtime.host else {
            panic!("owner lost")
        };
        assert_eq!(owner.selected_source_current_basis(), before);
        assert!(owner.execute_selected_source(&root).is_err()); // claims only malformed request
        assert!(root.join("source-execution/claimed.json").exists());
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_file(source).unwrap();
    }
    #[test]
    fn absent_inbox_neither_creates_files_nor_consumes_an_attempt() {
        let root =
            std::env::temp_dir().join(format!("source-execution-idle-{}", std::process::id()));
        assert!(!root.exists());
        let mut runtime = DurableHostRuntime::new(
            "fixture/source".into(),
            "fixture/image".into(),
            conduit_std_host::StdHost::new(),
        );
        runtime.progress_selected_source_execution(&root).unwrap();
        assert!(!runtime.source_execution.attempted);
        assert!(!root.exists());
    }
}
