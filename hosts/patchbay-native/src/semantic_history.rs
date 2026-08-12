//! Finite history for accepted canonical semantic Form transactions.
//!
//! This owns source/identity checkpoints only. It cannot represent lifecycle,
//! Host, filesystem, evidence, selection, navigation, or viewport rollback.

pub(super) const MAX_SEMANTIC_HISTORY_TRANSACTIONS: usize = 16;
pub(super) const MAX_SEMANTIC_HISTORY_SOURCE_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SemanticCheckpoint {
    pub(super) source: String,
    pub(super) source_revision: u64,
    pub(super) saved_revision: u64,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) expanded_form_id: String,
}

impl SemanticCheckpoint {
    pub(super) fn from_editor(
        editor: &patchbay_model::FormEditor,
        graph: &patchbay_model::PatchbayGraph,
    ) -> Result<Self, String> {
        let view = editor.view();
        let source_document_id = view
            .checked
            .source_document_id
            .ok_or("checked canonical Form identity is absent")?;
        let checked_form_id = view
            .checked
            .forms
            .iter()
            .find(|form| form.name == view.open_form)
            .map(|form| form.checked_form_id.clone())
            .ok_or("open checked Form identity is absent")?;
        Ok(Self {
            source: view.source,
            source_revision: view.revision,
            saved_revision: view.saved_revision,
            source_document_id: source_document_id.as_str().into(),
            checked_form_id: checked_form_id.as_str().into(),
            expanded_form_id: graph.expanded_form_id.as_str().into(),
        })
    }

    fn same_current_basis(&self, current: &Self) -> bool {
        self.source == current.source
            && self.source_revision == current.source_revision
            && self.source_document_id == current.source_document_id
            && self.checked_form_id == current.checked_form_id
            && self.expanded_form_id == current.expanded_form_id
    }
}

impl super::PatchbayApplication {
    pub(super) fn semantic_checkpoint(&self) -> Result<SemanticCheckpoint, String> {
        SemanticCheckpoint::from_editor(
            self.form_editor
                .as_ref()
                .ok_or("canonical Form editor is absent")?,
            self.graphical_form
                .as_ref()
                .ok_or("graphical Form projection is absent")?,
        )
    }

    pub(super) fn move_semantic_history(
        &mut self,
        direction: SemanticHistoryDirection,
    ) -> Result<(), String> {
        if self.lifecycle_flow().state_code != "FORM_CHECKED" {
            self.publish_refusal(format!(
                "{} unavailable while {}: semantic history cannot rewind lifecycle or external state",
                direction.label(),
                self.lifecycle_flow().state_code
            ));
            return Ok(());
        }
        let current = self.semantic_checkpoint()?;
        let prepared = match self
            .semantic_history
            .as_ref()
            .ok_or("semantic history is absent")?
            .prepare(direction, &current)
        {
            Ok(prepared) => prepared,
            Err(SemanticHistoryRefusal::Empty) => {
                self.publish_refusal(format!("Nothing to {}", direction.label().to_lowercase()));
                return Ok(());
            }
            Err(error) => {
                self.publish_refusal(format!("{} refused: {error:?}", direction.label()));
                return Ok(());
            }
        };
        let source = prepared.source.clone();
        let editor = self
            .form_editor
            .as_mut()
            .ok_or("canonical Form editor is absent")?;
        editor
            .replace_source(source)
            .map_err(|error| error.to_string())?;
        editor.recheck().map_err(|error| error.to_string())?;
        self.form_selection = 0;
        self.refresh_graphical_form()?;
        let mut restored = self.semantic_checkpoint()?;
        let matches_saved = self
            .semantic_history
            .as_ref()
            .expect("history presence checked")
            .restored_matches_saved_source(&restored);
        if matches_saved {
            self.form_editor
                .as_mut()
                .expect("editor presence checked")
                .mark_saved(restored.source_revision)
                .map_err(|error| error.to_string())?;
            restored = self.semantic_checkpoint()?;
        }
        self.semantic_history
            .as_mut()
            .expect("history presence checked")
            .commit(prepared, restored)
            .map_err(|error| format!("semantic history commit: {error:?}"))?;
        self.publish_completed(format!("{} semantic edit", direction.label()));
        Ok(())
    }

    pub(super) fn mark_semantic_history_saved(&mut self) -> Result<(), String> {
        let current = self.semantic_checkpoint()?;
        self.semantic_history
            .as_mut()
            .ok_or("semantic history is absent")?
            .mark_saved(&current)
            .map_err(|error| format!("semantic history save: {error:?}"))
    }
}

impl SemanticHistoryDirection {
    const fn label(self) -> &'static str {
        match self {
            Self::Undo => "Undo",
            Self::Redo => "Redo",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SemanticHistoryDirection {
    Undo,
    Redo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedSemanticMove {
    generation: u64,
    from: usize,
    to: usize,
    direction: SemanticHistoryDirection,
    pub(super) source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SemanticHistoryRefusal {
    Empty,
    Oversize,
    Unchanged,
    StaleCurrent,
    StaleMove,
}

#[derive(Debug, Default)]
pub(super) struct SemanticHistory {
    checkpoints: Vec<SemanticCheckpoint>,
    cursor: usize,
    generation: u64,
    evicted: u64,
    saved_source_document_id: Option<String>,
}

impl SemanticHistory {
    pub(super) fn new(initial: SemanticCheckpoint) -> Result<Self, SemanticHistoryRefusal> {
        ensure_bounded(&initial)?;
        let saved_source_document_id = (initial.saved_revision == initial.source_revision)
            .then(|| initial.source_document_id.clone());
        let mut checkpoints = Vec::with_capacity(MAX_SEMANTIC_HISTORY_TRANSACTIONS + 1);
        checkpoints.push(initial);
        Ok(Self {
            checkpoints,
            cursor: 0,
            generation: 0,
            evicted: 0,
            saved_source_document_id,
        })
    }

    pub(super) fn record_accepted(
        &mut self,
        before: &SemanticCheckpoint,
        after: SemanticCheckpoint,
    ) -> Result<(), SemanticHistoryRefusal> {
        ensure_bounded(before)?;
        ensure_bounded(&after)?;
        let Some(current) = self.checkpoints.get(self.cursor) else {
            return Err(SemanticHistoryRefusal::Empty);
        };
        if !current.same_current_basis(before) {
            return Err(SemanticHistoryRefusal::StaleCurrent);
        }
        if before.source == after.source {
            return Err(SemanticHistoryRefusal::Unchanged);
        }
        self.checkpoints.truncate(self.cursor + 1);
        self.checkpoints.push(after);
        self.cursor += 1;
        if self.checkpoints.len() > MAX_SEMANTIC_HISTORY_TRANSACTIONS + 1 {
            self.checkpoints.remove(0);
            self.cursor -= 1;
            self.evicted = self.evicted.saturating_add(1);
        }
        self.generation = self.generation.saturating_add(1);
        Ok(())
    }

    pub(super) fn prepare(
        &self,
        direction: SemanticHistoryDirection,
        current: &SemanticCheckpoint,
    ) -> Result<PreparedSemanticMove, SemanticHistoryRefusal> {
        let Some(checkpoint) = self.checkpoints.get(self.cursor) else {
            return Err(SemanticHistoryRefusal::Empty);
        };
        if !checkpoint.same_current_basis(current) {
            return Err(SemanticHistoryRefusal::StaleCurrent);
        }
        let to = match direction {
            SemanticHistoryDirection::Undo if self.cursor > 0 => self.cursor - 1,
            SemanticHistoryDirection::Redo if self.cursor + 1 < self.checkpoints.len() => {
                self.cursor + 1
            }
            _ => return Err(SemanticHistoryRefusal::Empty),
        };
        Ok(PreparedSemanticMove {
            generation: self.generation,
            from: self.cursor,
            to,
            direction,
            source: self.checkpoints[to].source.clone(),
        })
    }

    pub(super) fn commit(
        &mut self,
        prepared: PreparedSemanticMove,
        restored: SemanticCheckpoint,
    ) -> Result<(), SemanticHistoryRefusal> {
        ensure_bounded(&restored)?;
        if prepared.generation != self.generation
            || prepared.from != self.cursor
            || self
                .checkpoints
                .get(prepared.to)
                .is_none_or(|target| target.source != restored.source)
        {
            return Err(SemanticHistoryRefusal::StaleMove);
        }
        self.checkpoints[prepared.to] = restored;
        self.cursor = prepared.to;
        self.generation = self.generation.saturating_add(1);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    #[cfg(test)]
    pub(super) fn can_redo(&self) -> bool {
        self.cursor + 1 < self.checkpoints.len()
    }

    #[cfg(test)]
    pub(super) fn transaction_count(&self) -> usize {
        self.checkpoints.len().saturating_sub(1)
    }

    #[cfg(test)]
    pub(super) fn evicted(&self) -> u64 {
        self.evicted
    }

    pub(super) fn mark_saved(
        &mut self,
        current: &SemanticCheckpoint,
    ) -> Result<(), SemanticHistoryRefusal> {
        let Some(checkpoint) = self.checkpoints.get(self.cursor) else {
            return Err(SemanticHistoryRefusal::Empty);
        };
        if !checkpoint.same_current_basis(current) {
            return Err(SemanticHistoryRefusal::StaleCurrent);
        }
        self.saved_source_document_id = Some(current.source_document_id.clone());
        self.checkpoints[self.cursor] = current.clone();
        self.generation = self.generation.saturating_add(1);
        Ok(())
    }

    pub(super) fn restored_matches_saved_source(&self, restored: &SemanticCheckpoint) -> bool {
        self.saved_source_document_id.as_deref() == Some(restored.source_document_id.as_str())
    }
}

fn ensure_bounded(checkpoint: &SemanticCheckpoint) -> Result<(), SemanticHistoryRefusal> {
    if checkpoint.source.len() > MAX_SEMANTIC_HISTORY_SOURCE_BYTES {
        Err(SemanticHistoryRefusal::Oversize)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "semantic_history_tests.rs"]
mod tests;
