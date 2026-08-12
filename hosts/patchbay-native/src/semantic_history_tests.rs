use super::*;

fn checkpoint(sequence: u64, source: impl Into<String>) -> SemanticCheckpoint {
    let source = source.into();
    let semantic = source.replace(['\n', ' '], "_");
    SemanticCheckpoint {
        source,
        source_revision: sequence,
        saved_revision: 0,
        source_document_id: format!("source-{semantic}"),
        checked_form_id: format!("checked-{semantic}"),
        expanded_form_id: format!("expanded-{semantic}"),
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
    let restored = move_and_commit(&mut history, SemanticHistoryDirection::Undo, &edited, 2);
    assert_eq!(restored.source, initial.source);
    assert_eq!(restored.source_revision, 2);
    let redone = move_and_commit(&mut history, SemanticHistoryDirection::Redo, &restored, 3);
    assert_eq!(redone.source, edited.source);
    assert_eq!(redone.source_revision, 3);
}

#[test]
fn saved_baseline_tracks_source_identity_without_rewinding_filesystem_state() {
    let initial = checkpoint(0, "a");
    let edited = checkpoint(1, "b");
    let mut history = SemanticHistory::new(initial.clone()).unwrap();
    history.record_accepted(&initial, edited.clone()).unwrap();
    assert!(!history.restored_matches_saved_source(&edited));
    let restored = move_and_commit(&mut history, SemanticHistoryDirection::Undo, &edited, 2);
    assert!(history.restored_matches_saved_source(&restored));
    let mut saved = restored.clone();
    saved.saved_revision = saved.source_revision;
    history.mark_saved(&saved).unwrap();
    let redone = move_and_commit(&mut history, SemanticHistoryDirection::Redo, &saved, 3);
    assert!(!history.restored_matches_saved_source(&redone));
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

#[test]
fn restart_creates_one_fresh_checkpoint_without_persisting_history() {
    let initial = checkpoint(0, "a");
    let edited = checkpoint(1, "b");
    let mut old_process = SemanticHistory::new(initial.clone()).unwrap();
    old_process
        .record_accepted(&initial, edited.clone())
        .unwrap();
    assert!(old_process.can_undo());
    let restarted = SemanticHistory::new(edited).unwrap();
    assert_eq!(restarted.transaction_count(), 0);
    assert!(!restarted.can_undo());
    assert!(!restarted.can_redo());
    assert_eq!(restarted.evicted(), 0);
}
