//! Admission and reading fixtures, with no provider invocation or audible claim.
use super::super::execution::ReadingScope;
use super::*;
use conduit_std_host::spoken_face_mask::SpokenFaceSession;

fn root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(label, "owner"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn owner(host: StdHost) -> Owner {
    let plot = crate::plot_source::parse(
        "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.",
    )
    .unwrap()
    .expand_entry_for_authoring()
    .unwrap();
    Owner::open(
        host,
        ResidentPlot::new(
            plot.expanded.source_document_id,
            plot.expanded.checked_plot_id,
        ),
        None,
        "Same Body",
    )
    .unwrap()
}

/// A boundary-test Show, not evidence of a completed Mask or speaker Play.
fn fixture_show(face: &Presentation, planned: &conduit_presentation::PlannedMaskPlot) -> MaskShow {
    let placement = planned.show_placement();
    let play = conduit_core::bind_active_play(
        &planned.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        1,
    );
    MaskShow::prepared(
        planned,
        face,
        play,
        face.subjects[0].identity.clone(),
        "fixture/direct-artifact".into(),
        "sign/fixture/direct-prepared".into(),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        "sign/fixture/direct-available".into(),
    )
    .unwrap()
}

#[test]
fn direct_remaining_requires_acknowledgement_and_refuses_changed_host_offer() {
    let root = root("direct-detail-witness");
    let (host, _) = selected_host_with_artifact(&root, true);
    let mut owner = owner(host);
    let seal = owner.select_direct_spoken_route().unwrap();
    assert!(owner.prepare_selected_direct_spoken_read().is_err());
    let face = owner.local_face_snapshot().unwrap();
    let show = fixture_show(&face, &seal.planned_mask);
    owner
        .acknowledge_selected_direct_spoken_show(&seal, &show)
        .unwrap();
    let (selected, current_face, current_show) =
        owner.prepare_selected_direct_spoken_read().unwrap();
    assert_eq!(selected, seal);
    assert_eq!(current_face, face);
    assert_eq!(current_show, show);
    owner
        .validate_selected_direct_spoken_read(&seal, &show)
        .unwrap();
    let (terminal, peer) = std::os::unix::net::UnixStream::pair().unwrap();
    owner.host.attach_terminal_mask(terminal).unwrap();
    assert!(owner
        .validate_selected_direct_spoken_read(&seal, &show)
        .is_err());
    drop(peer);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn direct_remaining_cancellation_restores_same_host_without_provider_effect() {
    let root = root("direct-detail-cancellation");
    let (host, mut equipment) = selected_host_with_artifact(&root, true);
    let original = host.advertisement().clone();
    let mut owner = owner(host);
    let seal = owner.select_direct_spoken_route().unwrap();
    let face = owner.local_face_snapshot().unwrap();
    let show = fixture_show(&face, &seal.planned_mask);
    owner
        .acknowledge_selected_direct_spoken_show(&seal, &show)
        .unwrap();
    let gate = Arc::new(Barrier::new(2));
    equipment.before_play = Some(gate.clone());
    let mut runtime = runtime(owner, &root);
    assert!(runtime
        .start_direct_remaining_speech()
        .unwrap_err()
        .contains("no selected speech equipment"));
    assert_eq!(runtime.host.advertisement(), &original);
    runtime.selected_speech_equipment = Some(equipment);
    let operation = runtime.start_direct_remaining_speech().unwrap();
    gate.wait();
    assert!(runtime
        .start_direct_remaining_speech()
        .unwrap_err()
        .contains("already running"));
    runtime.stop_direct_spoken_or_reading(&operation).unwrap();
    gate.wait();
    let deadline = Instant::now() + Duration::from_secs(5);
    while runtime
        .speech_worker
        .as_ref()
        .is_some_and(|worker| !worker.thread.is_finished())
    {
        assert!(Instant::now() < deadline, "direct reader did not stop");
        std::thread::yield_now();
    }
    let receipt = runtime.direct_spoken_or_reading_status(&operation).unwrap();
    assert_eq!(receipt["outcome"], "cancelled");
    assert_eq!(receipt["source_mask"], "direct");
    assert_eq!(receipt["reader_scope"], "remaining-items");
    assert_eq!(receipt["source_show_id"], show.show_id.as_str());
    assert_eq!(receipt["source_show_still_current"], true);
    assert_eq!(receipt["completed_batch_count"], 0);
    assert_eq!(runtime.host.advertisement(), &original);
    let HostSource::Body { owner, .. } = &mut runtime.host else {
        panic!("lost Body owner")
    };
    assert_eq!(owner.local_face_snapshot().unwrap(), face);
    assert!(runtime.stop_direct_spoken_or_reading(&operation).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remaining_scope_and_pages_select_the_same_actual_todo_primary_items() {
    use conduit_core::{ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SourceDocumentId};
    use conduit_presentation::{PresentationBasis, PresentationContributionBasis};
    use conduit_todo_plot::{TodoItem, TodoState};
    let root = root("direct-detail-selection");
    let (host, _) = selected_host_with_artifact(&root, true);
    let state = TodoState {
        title: "Groceries".into(),
        revision: 20,
        next_id: 21,
        items: (1..=20)
            .map(|id| TodoItem {
                id: format!("task-{id}"),
                text: format!("Current task {id:02}"),
                complete: id > 7,
            })
            .collect(),
    };
    let fragment = conduit_todo_face::todo_fragment(
        &state,
        PresentationContributionBasis {
            checked_plot_id: CheckedPlotId::from("checked/todo"),
            plan_id: PlanId::from("plan/todo"),
            active_play_id: ActivePlayId::from("play/todo"),
            required_interaction_context: None,
        },
        true,
    )
    .unwrap();
    let face = Presentation::new_with_semantics(
        20,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/todo")),
            checked_plot_id: Some(fragment.basis.checked_plot_id),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/todo")),
            plan_id: Some(fragment.basis.plan_id),
            active_play_id: Some(fragment.basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap();
    let prepared = host.prepare_direct_spoken_mask(&face).unwrap();
    let show = fixture_show(&face, &prepared.planned_mask);
    let mut cursor = conduit_std_host::spoken_face_mask::SpokenItemCursor::new(&face).unwrap();
    let mut page_sizes = Vec::new();
    let mut paged_items = Vec::new();
    loop {
        let page = cursor.next_page(&face).unwrap();
        page_sizes.push(page.subject_ids.len());
        paged_items.extend(page.clauses);
        if !page.more {
            break;
        }
    }
    assert_eq!(page_sizes, [3, 3, 1]);
    assert_eq!(
        paged_items,
        (1..=7)
            .map(|id| format!("Current task {id:02}, item."))
            .collect::<Vec<_>>()
    );
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReadingScope::RemainingItems.command(), 1)
        .unwrap();
    let readout = reader.take_text_readout().unwrap().unwrap();
    assert_eq!(readout.face_id, face.identity.as_str());
    assert_eq!(readout.show_id, show.show_id.as_str());
    let readout = readout.clauses.join(" ");
    for id in 1..=7 {
        assert!(
            readout.contains(&format!("Current task {id:02}")),
            "{readout}"
        );
    }
    for id in 8..=20 {
        assert!(
            !readout.contains(&format!("Current task {id:02}")),
            "{readout}"
        );
    }
    assert!(!readout.contains("task-1"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn artifact_remaining_batch_refuses_stale_source_and_precancel_without_provider_effect() {
    use conduit_std_host::spoken_face_mask::ReaderCommand;
    let root = root("artifact-detail-refusals");
    let (mut host, _) = selected_host_profile(&root, true, false);
    assert!(host.spoken_artifact_only_route_is_current());
    let original = host.advertisement().clone();
    let mut owner = owner(host);
    let seal = owner.select_direct_spoken_route().unwrap();
    let face = owner.local_face_snapshot().unwrap();
    let show = fixture_show(&face, &seal.planned_mask);
    owner
        .acknowledge_selected_direct_spoken_show(&seal, &show)
        .unwrap();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let batch = reader.next_batch().unwrap().unwrap();
    let mut stale = face.clone();
    stale.revision += 1;
    host = owner.host.take_for_play().unwrap();
    let control = RunControl::default();
    let failure = super::super::execution::batch::play_batch(
        &mut host, &stale, &show, &batch, None, &control,
    )
    .err()
    .unwrap();
    assert_eq!(failure.outcome(), "refused");
    let mut stale_show = show.clone();
    stale_show.show.lifecycle = ManifestationLifecycle::Replaced;
    let failure = super::super::execution::batch::play_batch(
        &mut host,
        &face,
        &stale_show,
        &batch,
        None,
        &control,
    )
    .err()
    .unwrap();
    assert_eq!(failure.outcome(), "refused");
    control
        .request_stop(RunControlRequestId::new("stop/artifact-detail").unwrap())
        .unwrap();
    let failure =
        super::super::execution::batch::play_batch(&mut host, &face, &show, &batch, None, &control)
            .err()
            .unwrap();
    assert_eq!(failure.outcome(), "cancelled");
    assert_eq!(host.advertisement(), &original);
    assert_eq!(
        fs::read_dir(root.join("mask-artifacts")).unwrap().count(),
        0
    );
    owner.host.restore_after_play(host).unwrap();
    assert_eq!(owner.local_face_snapshot().unwrap(), face);
    fs::remove_dir_all(root).unwrap();
}
