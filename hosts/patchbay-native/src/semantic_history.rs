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
    fn same_current_basis(&self, current: &Self) -> bool {
        self.source == current.source
            && self.source_revision == current.source_revision
            && self.source_document_id == current.source_document_id
            && self.checked_form_id == current.checked_form_id
            && self.expanded_form_id == current.expanded_form_id
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
}

impl SemanticHistory {
    pub(super) fn new(initial: SemanticCheckpoint) -> Result<Self, SemanticHistoryRefusal> {
        ensure_bounded(&initial)?;
        Ok(Self {
            checkpoints: vec![initial],
            cursor: 0,
            generation: 0,
            evicted: 0,
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

    pub(super) fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub(super) fn can_redo(&self) -> bool {
        self.cursor + 1 < self.checkpoints.len()
    }

    pub(super) fn transaction_count(&self) -> usize {
        self.checkpoints.len().saturating_sub(1)
    }

    pub(super) fn evicted(&self) -> u64 {
        self.evicted
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
mod tests {
    use super::*;

    fn checkpoint(sequence: u64, source: impl Into<String>) -> SemanticCheckpoint {
        SemanticCheckpoint {
            source: source.into(),
            source_revision: sequence,
            saved_revision: 0,
            source_document_id: format!("source-{sequence}"),
            checked_form_id: format!("checked-{sequence}"),
            expanded_form_id: format!("expanded-{sequence}"),
        }
    }

    fn move_and_commit(
        history: &mut SemanticHistory,
        direction: SemanticHistoryDirection,
        current: &SemanticCheckpoint,
        restored_revision: u64,
    ) -> SemanticCheckpoint {
        let prepared = history.prepare(direction, current).unwrap();
        let restored = checkpoint(restored_revision, prepared.source.clone());
        history.commit(prepared, restored.clone()).unwrap();
        restored
    }

    #[test]
    fn exact_round_trip_updates_restored_basis_without_reusing_revision() {
        let initial = checkpoint(0, "form initial {}\n");
        let edited = checkpoint(1, "form edited {}\n");
        let mut history = SemanticHistory::new(initial.clone()).unwrap();
        history.record_accepted(&initial, edited.clone()).unwrap();

        let restored_initial =
            move_and_commit(&mut history, SemanticHistoryDirection::Undo, &edited, 2);
        assert_eq!(restored_initial.source, initial.source);
        assert_eq!(restored_initial.source_revision, 2);
        let restored_edit = move_and_commit(
            &mut history,
            SemanticHistoryDirection::Redo,
            &restored_initial,
            3,
        );
        assert_eq!(restored_edit.source, edited.source);
        assert_eq!(restored_edit.source_revision, 3);
    }

    #[test]
    fn refusal_and_failed_restore_cannot_move_the_cursor() {
        let initial = checkpoint(0, "a");
        let edited = checkpoint(1, "b");
        let mut history = SemanticHistory::new(initial.clone()).unwrap();
        assert_eq!(
            history.record_accepted(&initial, initial.clone()),
            Err(SemanticHistoryRefusal::Unchanged)
        );
        assert!(!history.can_undo());
        history.record_accepted(&initial, edited.clone()).unwrap();
        let prepared = history
            .prepare(SemanticHistoryDirection::Undo, &edited)
            .unwrap();
        assert!(history.can_undo());
        drop(prepared);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn divergent_accepted_edit_clears_redo_deterministically() {
        let initial = checkpoint(0, "a");
        let first = checkpoint(1, "b");
        let mut history = SemanticHistory::new(initial.clone()).unwrap();
        history.record_accepted(&initial, first.clone()).unwrap();
        let restored = move_and_commit(&mut history, SemanticHistoryDirection::Undo, &first, 2);
        assert!(history.can_redo());
        history
            .record_accepted(&restored, checkpoint(3, "divergent"))
            .unwrap();
        assert!(!history.can_redo());
        assert!(history.can_undo());
    }

    #[test]
    fn capacity_evicts_oldest_transaction_and_reports_count() {
        let mut current = checkpoint(0, "0");
        let mut history = SemanticHistory::new(current.clone()).unwrap();
        for sequence in 1..=MAX_SEMANTIC_HISTORY_TRANSACTIONS as u64 + 4 {
            let next = checkpoint(sequence, sequence.to_string());
            history.record_accepted(&current, next.clone()).unwrap();
            current = next;
        }
        assert_eq!(
            history.transaction_count(),
            MAX_SEMANTIC_HISTORY_TRANSACTIONS
        );
        assert_eq!(history.evicted(), 4);
        for revision in 100..100 + MAX_SEMANTIC_HISTORY_TRANSACTIONS as u64 {
            current = move_and_commit(
                &mut history,
                SemanticHistoryDirection::Undo,
                &current,
                revision,
            );
        }
        assert!(!history.can_undo());
    }

    #[test]
    fn stale_current_stale_move_and_oversize_refuse_distinctly() {
        let initial = checkpoint(0, "a");
        let edited = checkpoint(1, "b");
        let mut history = SemanticHistory::new(initial.clone()).unwrap();
        history.record_accepted(&initial, edited.clone()).unwrap();
        assert_eq!(
            history.prepare(SemanticHistoryDirection::Undo, &checkpoint(9, "b")),
            Err(SemanticHistoryRefusal::StaleCurrent)
        );
        let prepared = history
            .prepare(SemanticHistoryDirection::Undo, &edited)
            .unwrap();
        history.generation += 1;
        assert_eq!(
            history.commit(prepared, checkpoint(2, "a")),
            Err(SemanticHistoryRefusal::StaleMove)
        );
        let oversized = checkpoint(10, "x".repeat(MAX_SEMANTIC_HISTORY_SOURCE_BYTES + 1));
        assert!(matches!(
            SemanticHistory::new(oversized),
            Err(SemanticHistoryRefusal::Oversize)
        ));
    }
}
