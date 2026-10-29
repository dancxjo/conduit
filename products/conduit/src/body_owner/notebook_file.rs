//! Notebook file encounters use the installed owner's one Host and lifecycle.
use super::super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use super::{checkpoint, NotebookDocument, SOURCE};
use conduit_body::{BodyId, ResidentPlot};
use conduit_core::*;
use conduit_semantic_catalog::TextDocument;
use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
use conduit_std_host::{ExternalForeDelivery, ExternalForeInput, RunControl};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    path::Path,
    time::{Duration, Instant},
};

#[path = "notebook_file_plan.rs"]
mod planning;
const PUBLISH: &str = include_str!("../../../../plots/notebook/publish.conduit");
const LOAD: &str = include_str!("../../../../plots/notebook/load.conduit");

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    schema: String,
    body_id: BodyId,
    name: String,
    revision: u64,
    reference: Vec<u8>,
    payload_sha256: String,
    root_device: u64,
    root_inode: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root_path: Option<std::path::PathBuf>,
}

fn selected_path(path: &Path) -> Result<(File, std::path::PathBuf), String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("selected notebook storage: {error}"))?;
    let text = path.to_str().ok_or("notebook storage path is not UTF-8")?;
    if text.len() > 1024 || text.chars().any(char::is_control) {
        return Err("notebook storage selection exceeds its finite path profile".into());
    }
    let directory =
        File::open(&path).map_err(|error| format!("selected notebook storage: {error}"))?;
    Ok((directory, path))
}

fn retain_selection(root: &Path, selection: &Selection) -> Result<(), String> {
    if serde_json::to_vec_pretty(selection)
        .map_err(|error| error.to_string())?
        .len()
        > 4096
    {
        return Err("notebook selection exceeds its finite metadata bound".into());
    }
    super::super::super::super::write_json_atomic(
        &root.join("body/notebook-commit.json"),
        selection,
    )
}

fn root_identity(root: &File) -> Result<(u64, u64), String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = root.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_dir() {
        return Err("notebook storage selection is not a directory".into());
    }
    Ok((metadata.dev(), metadata.ino()))
}

struct OneOutput(Vec<ExternalForeDelivery>);
impl BodyForeOutputAdapter for OneOutput {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        if !self.0.is_empty() || value.bytes.len() > conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize
        {
            return Err("notebook file Play exceeded its admitted output".into());
        }
        self.0.push(value.clone());
        Ok(())
    }
}

impl Owner {
    pub(crate) fn save_notebook_at(&mut self, root: &Path, path: &Path) -> Result<(), String> {
        let (directory, path) = selected_path(path)?;
        self.save_notebook_with_selection(root, directory, Some(path))
    }

    /// A retained path is preparation authority for this exact root identity,
    /// not permission to discover a substitute or manufacture draft bytes.
    pub(crate) fn recover_notebook_on_startup(&mut self, root: &Path) -> Result<bool, String> {
        if !matches!(
            self.resident_name.as_deref(),
            Some("notebook/transition" | "notebook/publish" | "notebook/load")
        ) {
            return Ok(false);
        }
        let record = root.join("body/notebook-commit.json");
        if !record.exists() {
            return Ok(false);
        }
        let selection: Selection =
            serde_json::from_slice(&super::super::super::super::bounded_read(&record, 4096)?)
                .map_err(|error| error.to_string())?;
        if selection.schema != "conduit.notebook/selection@1"
            || selection.body_id != self.session.evidence().body_id
        {
            return Err("retained notebook selection is invalid or belongs to another Body".into());
        }
        let Some(path) = selection.root_path else {
            return Ok(false);
        };
        if !path.is_absolute() {
            return Err(
                "retained notebook storage path is not an explicit absolute selection".into(),
            );
        }
        let (directory, _) = selected_path(&path)?;
        self.reopen_notebook(root, directory)?;
        Ok(true)
    }
    /// A caller-selected directory descriptor grants no authority until its
    /// exact file Back, content generation and grant are admitted in this Plan.
    pub(crate) fn save_notebook(&mut self, root: &Path, directory: File) -> Result<(), String> {
        let path = self
            .notebook
            .as_ref()
            .and_then(|document| document.storage_path.clone());
        self.save_notebook_with_selection(root, directory, path)
    }

    fn save_notebook_with_selection(
        &mut self,
        root: &Path,
        directory: File,
        path: Option<std::path::PathBuf>,
    ) -> Result<(), String> {
        let result = (|| {
            let selected = directory.try_clone().map_err(|error| error.to_string())?;
            self.publish_notebook(root, directory, path.clone())?;
            let document = self.notebook.as_mut().ok_or("notebook disappeared")?;
            document.storage = Some(selected);
            document.storage_path = path;
            Ok(())
        })();
        if let Some(document) = &mut self.notebook {
            document.save_failed = result.is_err();
        }
        result
    }

    pub(crate) fn save_selected_notebook(&mut self, root: &Path) -> Result<(), String> {
        let selected = self
            .notebook
            .as_ref()
            .and_then(|document| document.storage.as_ref())
            .ok_or("notebook has no explicitly selected storage directory")?
            .try_clone()
            .map_err(|error| error.to_string())?;
        self.save_notebook(root, selected)
    }

    fn publish_notebook(
        &mut self,
        root: &Path,
        directory: File,
        path: Option<std::path::PathBuf>,
    ) -> Result<(), String> {
        self.ensure_execution_retained(root)?;
        let notebook = self.notebook.as_ref().ok_or("notebook is not open")?;
        let body_id = self.session.evidence().body_id.clone();
        let name = notebook.name.clone();
        let revision = TextDocument::decode(&notebook.current)
            .map_err(debug)?
            .revision;
        let payload = checkpoint::encode(&body_id, &name, &notebook.current)?;
        let (root_device, root_inode) = root_identity(&directory)?;
        let mut version = [0; 32];
        getrandom::fill(&mut version).map_err(|error| error.to_string())?;
        let reference = BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(
                Sha256::digest(body_id.as_str().as_bytes()).into(),
            ),
            content_profile: kind_id("value/text"),
            access_class: conduit_data::DATA_GENERATION_ACCESS_CLASS.into(),
            extent: ResourceExtent {
                bytes: payload.len() as u64,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest(version),
                expires_at: None,
            },
        };
        self.host
            .transition_text_file_offer(directory, reference.clone(), true)?;
        self.select_notebook_source(root, PUBLISH)?;
        let result = self.notebook_file_encounter(root, true, payload.clone());
        let restored = self.select_notebook_source(root, SOURCE);
        let saved = result?;
        restored?;
        let decoded = conduit_data::DataReference::decode_for(&kind_id("value/text"), &saved)
            .map_err(debug)?;
        if decoded.reference() != &reference {
            return Err("notebook publication returned another generation".into());
        }
        let selection = Selection {
            schema: "conduit.notebook/selection@1".into(),
            body_id,
            name,
            revision,
            reference: saved,
            payload_sha256: super::super::super::super::digest(&payload),
            root_device,
            root_inode,
            root_path: path,
        };
        // This durable selection follows the successful file and terminal Sign.
        // A crash before selection leaves the previous committed revision selected.
        retain_selection(root, &selection)?;
        if let Some(document) = &mut self.notebook {
            document.committed = Some(document.current.clone());
        }
        Ok(())
    }

    /// Reopen only an explicitly selected directory and this Body's last committed
    /// generation. Draft bytes in the prior process are never a recovery input.
    pub(crate) fn reopen_notebook_at(&mut self, root: &Path, path: &Path) -> Result<(), String> {
        let (directory, path) = selected_path(path)?;
        self.reopen_notebook_with_selection(root, directory, Some(path))
    }

    pub(crate) fn reopen_notebook(&mut self, root: &Path, directory: File) -> Result<(), String> {
        self.reopen_notebook_with_selection(root, directory, None)
    }

    fn reopen_notebook_with_selection(
        &mut self,
        root: &Path,
        directory: File,
        path: Option<std::path::PathBuf>,
    ) -> Result<(), String> {
        self.ensure_execution_retained(root)?;
        let selected = directory.try_clone().map_err(|error| error.to_string())?;
        let bytes = super::super::super::super::bounded_read(
            &root.join("body/notebook-commit.json"),
            4096,
        )?;
        let mut selection: Selection =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        let identity = root_identity(&directory)?;
        if selection.schema != "conduit.notebook/selection@1"
            || selection.body_id != self.session.evidence().body_id
            || identity != (selection.root_device, selection.root_inode)
            || self
                .notebook
                .as_ref()
                .is_some_and(|document| document.name != selection.name)
        {
            return Err(
                "selected notebook commit belongs to another Body, document or storage root".into(),
            );
        }
        let reference =
            conduit_data::DataReference::decode_for(&kind_id("value/text"), &selection.reference)
                .map_err(debug)?
                .reference()
                .clone();
        self.host
            .transition_text_file_offer(directory, reference, false)?;
        let encoded = selection.reference.clone();
        self.select_notebook_source(root, LOAD)?;
        let result = self.notebook_file_encounter(root, false, encoded);
        let restored = self.select_notebook_source(root, SOURCE);
        let payload = result?;
        restored?;
        if super::super::super::super::digest(&payload) != selection.payload_sha256 {
            return Err("notebook read differs from the committed payload digest".into());
        }
        let current = checkpoint::decode(&selection.body_id, &selection.name, &payload)?;
        if TextDocument::decode(&current).map_err(debug)?.revision != selection.revision {
            return Err("notebook committed revision differs from the selected generation".into());
        }
        if let Some(path) = path {
            selection.root_path = Some(path);
            retain_selection(root, &selection)?;
        }
        self.notebook = Some(NotebookDocument {
            name: selection.name,
            committed: Some(current.clone()),
            current,
            save_failed: false,
            navigation: self
                .notebook
                .as_ref()
                .map(|document| document.navigation.clone())
                .unwrap_or_default(),
            storage: Some(selected),
            storage_path: selection.root_path,
        });
        Ok(())
    }

    /// Source custody and workload replacement cross the existing owner journal.
    /// No replacement Body or independent notebook scheduler is constructed.
    fn select_notebook_source(&mut self, root: &Path, text: &str) -> Result<(), String> {
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("notebook Source selection requires an idle owner".into());
        }
        let source = crate::plot_source::parse(text)?;
        let plot = source.expand_entry_for_authoring()?;
        let resident = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        if self.resident.as_ref() == Some(&resident) {
            return Ok(());
        }
        let old = self
            .resident
            .as_ref()
            .ok_or("notebook has no resident Source")?
            .clone();
        if !matches!(
            self.resident_name.as_deref(),
            Some("notebook/transition" | "notebook/publish" | "notebook/load")
        ) {
            return Err("notebook cannot replace another application's Source".into());
        }
        let ad = self.host.advertisement();
        let mut next = self.session.clone();
        let revision = next.evidence().body.workload_revision;
        next.remove_plot(revision, &old, &ad.host_id, &ad.boot_id)
            .map_err(debug)?;
        next.admit_plot(
            revision
                .checked_add(1)
                .ok_or("notebook workload revision exhausted")?,
            resident.clone(),
            &ad.host_id,
            &ad.boot_id,
        )
        .map_err(debug)?;
        // Keep the actual staged journal in memory on a write failure; a later
        // request must recover/retain it instead of reverting an on-disk commit.
        self.session = next;
        self.resident = Some(resident);
        self.resident_name = Some(plot.expanded.name);
        state::retain_session(
            root,
            &mut self.session,
            self.last_execution.as_ref(),
            self.admissions.as_ref(),
            Some(text.as_bytes()),
            None,
        )
    }

    fn notebook_file_encounter(
        &mut self,
        root: &Path,
        write: bool,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        self.plan_notebook_file(write)?;
        self.persist(root)?;
        state::require_execution_capacity(root)?;
        let proposed = self
            .session
            .realization()
            .ok_or("notebook file Plan required")?
            .clone();
        let ad = self.host.advertisement().clone();
        let inputs = [ExternalForeInput {
            front_port_id: port_id(if write { "value" } else { "selected" }),
            track: ConnectionTrack::Payload,
            bytes,
        }];
        let control = RunControl::default();
        let session = &mut self.session;
        let admissions = self.admissions.as_ref();
        let host = &mut self.host;
        let (report, output) = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("conduit-notebook-file".into())
                .stack_size(4 * 1024 * 1024)
                .spawn_scoped(scope, || {
                    let mut fore = OneOutput(Vec::with_capacity(1));
                    let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                    let mut timer = DeadlineTimer {
                        until: Instant::now() + Duration::from_secs(5),
                        control: control.clone(),
                    };
                    let report = host.run_body_plan_with_fore_to_with_start(
                        BodyRunRequest {
                            wake: &proposed.wake,
                            plan: &proposed.plan,
                            control: &control,
                            keyboard: None,
                        },
                        &inputs,
                        false,
                        &mut fore,
                        &mut output,
                        &mut timer,
                        |play, wake| {
                            let mut next = session.clone();
                            next.started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                                .map_err(debug)?;
                            state::retain_session(root, &mut next, None, admissions, None, None)?;
                            *session = next;
                            Ok(())
                        },
                    );
                    (report, fore)
                })
                .map_err(|error| format!("start notebook file worker: {error}"))?
                .join()
                .map_err(|_| "notebook file worker panicked".to_string())
        })?;
        let report = report?;
        let success = report.terminal == TerminalDisposition::Completed
            && report.failure.is_none()
            && report.cleanup_failure.is_none()
            && report.fore_deliveries.len() == 1
            && output.0.len() == 1
            && output.0[0].track == ConnectionTrack::Payload
            && output.0[0].front_port_id == port_id(if write { "saved" } else { "value" })
            && report.terminal_sign.active_play_id == Some(report.play.active_play_id.clone());
        let mut next = self.session.clone();
        if next
            .realization()
            .and_then(|realization| realization.play.as_ref())
            != Some(&report.play)
        {
            return Err("notebook file report differs from retained started Play".into());
        }
        next.lull(&ad.host_id, &ad.boot_id, Some(&report.play))
            .map_err(debug)?;
        let publication_interaction = self
            .last_execution
            .as_ref()
            .filter(|receipt| {
                write
                    && receipt["schema"] == "conduit.notebook/waiting-edit-receipt@1"
                    && receipt["initiating_action"]["action_id"] == "notebook.save"
            })
            .and_then(|receipt| receipt.get("initiating_action"))
            .cloned();
        let receipt = serde_json::json!({
            "schema":"conduit.notebook/file-receipt@1", "body_id":proposed.wake.body_id,
            "host_id":ad.host_id, "boot_id":ad.boot_id, "plan_id":proposed.plan.plan_id,
            "wake_id":proposed.wake.wake_id, "play":report.play, "terminal":report.terminal,
            "failure":report.failure, "cleanup_failure":report.cleanup_failure,
            "terminal_sign":report.terminal_sign, "publication":write, "file_operation_succeeded":success,
            "publication_interaction":publication_interaction,
        });
        self.retain_execution(root, next, receipt)?;
        if !success {
            return Err(
                "notebook storage refused the operation; current document preserved".into(),
            );
        }
        Ok(output.0[0].bytes.to_vec())
    }
}
