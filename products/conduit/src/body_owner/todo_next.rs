//! Retire a verified read and admit the next command on the same Body.
//! The installed Host selects a fresh write version; no directory infers it.

use super::{debug, state, todo_waiting::NewTodoCheckpoint, Owner, TodoWaitingWorker};
use conduit_body::{BodyState, ResidentPlot};
use conduit_core::{AuthorityGrant, ResourceAccessMode};
use conduit_std_host::todo_durable_resource::{CheckpointIdentity, MissingV2Disposition};
use std::path::Path;

const WRITE_SOURCE: &str = include_str!("../../../../plots/todo/checkpoint-once.conduit");

impl Owner {
    pub(crate) fn has_verified_todo(&self) -> bool {
        self.todo_verified.is_some() && self.todo_verified_read_receipt().is_some()
    }

    /// Prepare the Host-selected write generation after an exact committed read.
    /// The returned worker still requires a current Show and interaction; no
    /// mutation occurs until its admitted command Fore reaches the Host Call.
    pub(crate) fn start_next_todo_action(
        &mut self,
        state_root: &Path,
        next: &super::super::super::NextSelectedTodoCheckpoint,
        maximum_millis: u64,
    ) -> Result<TodoWaitingWorker, String> {
        let checkpoint_root = &next.root;
        let selected_read = &next.read_content;
        let selection = &next.selection;
        let selected_write = selection.content();
        let old_selection =
            super::super::super::read_installation(&state_root.join("installation.json"))?
                .selected_todo_checkpoint
                .ok_or("next Todo action has no installed selection")?;
        if self.host.is_playing()
            || self.session.evidence().body.state != BodyState::Lulled
            || self.session.realization().is_some()
            || self.resident_name.as_deref() != Some("todo/checkpoint-restore")
            || selected_read.access != ResourceAccessMode::ReadPublished
            || selected_write.access != ResourceAccessMode::WriteCandidatePublish
            || selected_write.version == selected_read.version
            || selected_write.identity != selected_read.identity
            || selected_write.content_profile != selected_read.content_profile
            || selected_write.maximum_bytes != selected_read.maximum_bytes
            || selected_write.maximum_items != selected_read.maximum_items
            || selected_write.retention != selected_read.retention
            || selected_write.sharing != selected_read.sharing
            || selected_write.generation_slots != selected_read.generation_slots
            || selected_write.reader_leases != selected_read.reader_leases
            || selected_write.publication_slots != 1
            || selected_write.sensitive != selected_read.sensitive
            || old_selection.root() != checkpoint_root
            || old_selection.content().version != selected_read.version
            || selection.root() != checkpoint_root
        {
            return Err("next Todo action requires a fresh selected write after read".into());
        }
        let (basis, current) = self
            .todo_verified
            .as_ref()
            .ok_or("next Todo action has no verified read")?
            .clone();
        self.project_verified_todo_face(&basis, &current)?;
        let read_receipt = self
            .todo_verified_read_receipt()
            .ok_or("next Todo action has no retained read receipt")?
            .clone();
        if read_receipt["selected_content"] != serde_json::json!(selected_read)
            || basis.selection.selected_version != selected_read.version
            || current.revision == 0
        {
            return Err("next Todo action differs from verified selected read".into());
        }
        let list_key = read_receipt["write"]["checkpoint_namespace"]["list_key"]
            .as_str()
            .ok_or("verified Todo read has no list key")?
            .to_owned();
        let source = crate::plot_source::parse(WRITE_SOURCE)?;
        let plot = source.expand_entry_for_authoring()?;
        if plot.expanded.name != "todo/checkpoint-once" {
            return Err("next Todo write source differs from checked Plot".into());
        }
        let write_resident = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        let read_resident = self
            .resident
            .as_ref()
            .ok_or("next Todo action has no resident read Plot")?
            .clone();
        let old_session = self.session.clone();
        let old_name = self.resident_name.clone();
        let advertised = self.host.advertisement().clone();
        self.host
            .transition_todo_checkpoint_offer(checkpoint_root, selected_write.clone())?;
        let prepared = (|| {
            let advertisement = self.host.advertisement().clone();
            let offer = advertisement
                .capabilities
                .iter()
                .find(|offer| {
                    offer.implementation.implementation_id.as_str()
                        == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
                })
                .ok_or("next Todo Host has no selected write offer")?;
            let requirement = offer
                .authority_requirements
                .first()
                .filter(|_| offer.authority_requirements.len() == 1)
                .ok_or("next Todo write offer has unexpected authority")?;
            let grant = AuthorityGrant {
                grant_id: format!(
                    "grant/todo/{}/next/{}",
                    self.session.evidence().body_id.as_str(),
                    current.revision
                )
                .into(),
                contract_id: requirement.contract_id.clone(),
                host_call_contract_id: requirement.host_call_contract_id.clone(),
                subject_kind: requirement.subject_kind.clone(),
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                capability_id: offer.capability_id.clone(),
            };
            let mut staged = self.session.clone();
            let revision = staged.evidence().body.workload_revision;
            staged
                .remove_plot(
                    revision,
                    &read_resident,
                    &advertised.host_id,
                    &advertised.boot_id,
                )
                .map_err(debug)?;
            staged
                .admit_plot(
                    revision
                        .checked_add(1)
                        .ok_or("Todo workload revision exhausted")?,
                    write_resident.clone(),
                    &advertised.host_id,
                    &advertised.boot_id,
                )
                .map_err(debug)?;
            self.session = staged;
            self.resident = Some(write_resident.clone());
            self.resident_name = Some(plot.expanded.name.clone());
            let identity = CheckpointIdentity {
                body: self.session.evidence().body_id.as_str().to_owned(),
                plot: write_resident.checked_plot_id.as_str().to_owned(),
                workload: list_key,
                missing_v2: MissingV2Disposition::Refuse,
            };
            state::retain_with_source_and_todo_selection(
                state_root,
                self.session.evidence(),
                Some(&read_receipt),
                self.admissions.as_ref(),
                Some(WRITE_SOURCE.as_bytes()),
                Some(selection),
            )?;
            self.start_waiting_todo(
                state_root,
                &source,
                &plot,
                &grant,
                NewTodoCheckpoint {
                    root: checkpoint_root.to_path_buf(),
                    identity,
                    current,
                },
                maximum_millis,
            )
        })();
        match prepared {
            Ok(worker) => Ok(worker),
            Err(error) => {
                self.session = old_session;
                self.resident = Some(read_resident);
                self.resident_name = old_name;
                self.host
                    .transition_todo_checkpoint_offer(checkpoint_root, selected_read.clone())
                    .map_err(|rollback| {
                        format!("next Todo preparation failed: {error}; Host rollback failed: {rollback}")
                    })?;
                let retained_selection =
                    super::super::super::read_installation(&state_root.join("installation.json"))?
                        .selected_todo_checkpoint
                        .ok_or("next Todo rollback lost installed selection")?;
                state::retain_with_source_and_todo_selection(
                    state_root,
                    self.session.evidence(),
                    Some(&read_receipt),
                    self.admissions.as_ref(),
                    Some(
                        include_str!("../../../../plots/todo/checkpoint-restore.conduit")
                            .as_bytes(),
                    ),
                    (retained_selection.content().version == selected_write.version)
                        .then_some(&old_selection),
                )?;
                Err(error)
            }
        }
    }
}
