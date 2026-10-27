//! Authenticated local owner notebook controls, with no client-side document copy.
use super::{body::HostSource, DurableHostRuntime};
use crate::cli::NotebookCommand;
use conduit_semantic_catalog::TextEdit;
use std::path::Path;

impl DurableHostRuntime {
    pub(super) fn notebook_control(
        &mut self,
        command: NotebookCommand,
    ) -> Result<serde_json::Value, String> {
        let HostSource::Body {
            owner,
            root,
            running,
        } = &mut self.host
        else {
            return Err("notebook control requires an installed Notebook Body owner".into());
        };
        if running.is_some() && !matches!(&command, NotebookCommand::Show) {
            return Err("notebook mutation requires an idle owner or a current Mask action".into());
        }
        let operation = match command {
            NotebookCommand::New { name } => {
                owner.initialize_notebook(&name)?;
                owner.persist(root)?;
                "new"
            }
            NotebookCommand::Show => "show",
            NotebookCommand::Replace { revision, text } => {
                let (_, current) = owner
                    .notebook_document()
                    .ok_or("notebook is not open; reopen its committed revision first")?;
                let end = std::str::from_utf8(current.text)
                    .map_err(|error| error.to_string())?
                    .chars()
                    .count() as u32;
                owner.edit_notebook(
                    root,
                    TextEdit {
                        base_revision: revision,
                        start: 0,
                        end,
                        insert_text: text.as_bytes(),
                    },
                )?;
                "replace"
            }
            NotebookCommand::Save { storage_dir } => {
                owner.save_notebook_at(root, &storage_dir)?;
                "save"
            }
            NotebookCommand::Reopen { storage_dir } => {
                owner.reopen_notebook_at(root, &storage_dir)?;
                "reopen"
            }
        };
        let (name, document) = owner
            .notebook_document()
            .ok_or("notebook is not open; reopen its committed revision first")?;
        let mut response = serde_json::json!({
            "schema":"conduit.notebook/owner-response@1", "operation":operation,
            "body_id":owner.notebook_body_id(),
            "name":name, "revision":document.revision,
            "text":std::str::from_utf8(document.text).map_err(|error| error.to_string())?,
        });
        if operation == "save" {
            response["save_committed"] = true.into();
        }
        Ok(response)
    }
}

#[cfg(unix)]
pub(crate) fn run(root: &Path, command: NotebookCommand) -> Result<(), String> {
    use super::{
        body::{call, token},
        Request, Response, PROTOCOL,
    };
    match call(
        root,
        Request::Notebook {
            protocol: PROTOCOL,
            token: token(root)?,
            command,
        },
    )? {
        Response::BodyTruth {
            protocol: PROTOCOL,
            truth,
        } => {
            println!("{truth}");
            Ok(())
        }
        Response::Refused { code, .. } => Err(format!("notebook owner refused: {code}")),
        _ => Err("notebook owner returned an unexpected response".into()),
    }
}

impl DurableHostRuntime {
    /// A return is a retained Source result, never just queue staging.
    pub(super) fn submit_owned_notebook_action(
        &mut self,
        show: &conduit_face::MaskShow,
        interaction: &conduit_face::FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        use super::body_run::OwnedRunWorker;
        use conduit_std_host::BodyLiveForeAdmission;
        let HostSource::Body {
            owner,
            running: Some(OwnedRunWorker::Notebook(worker)),
            ..
        } = &mut self.host
        else {
            return Err("notebook has no current waiting edit Play".into());
        };
        let saving = interaction.action_id == "notebook.save";
        if saving {
            worker.authorize_save(owner, show, interaction)?;
        } else if !matches!(
            worker.submit_interaction(owner, show, interaction)?,
            BodyLiveForeAdmission::Accepted { .. }
        ) {
            return Err("notebook edit queue is full".into());
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            self.progress_owned_body()
                .map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_string())?;
            if matches!(&self.host, HostSource::Body { running: None, .. }) {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err(super::CONTROL_OUTCOME_UNKNOWN.into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let publication = if saving {
            let HostSource::Body {
                owner,
                root,
                running: None,
            } = &mut self.host
            else {
                return Err("notebook Save requires the retired edit encounter".into());
            };
            owner.save_selected_notebook(root)
        } else {
            Ok(())
        };
        let committed = {
            let HostSource::Body { owner, .. } = &self.host else {
                unreachable!("notebook owns its Host");
            };
            let (name, document) = owner
                .notebook_document()
                .ok_or("completed notebook lost its document")?;
            serde_json::json!({
                "schema":if saving { "conduit.notebook/mask-save-committed@1" } else { "conduit.notebook/mask-edit-committed@1" }, "body_id":owner.notebook_body_id(),
                "name":name, "revision":document.revision,
                "text":std::str::from_utf8(document.text).map_err(|error| error.to_string())?,
                "interaction_id":interaction.identity.as_str(),
            })
        };
        // Admit the next bounded encounter before a Mask requests the new Face.
        self.start_owned_body(300_000, None)
            .map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_string())?;
        loop {
            self.progress_owned_body()
                .map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_string())?;
            if matches!(&self.host, HostSource::Body { owner, .. } if owner.current_play_id().is_some())
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err(super::CONTROL_OUTCOME_UNKNOWN.into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        publication?;
        Ok(committed)
    }
}
