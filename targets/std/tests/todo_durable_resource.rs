use conduit_core::*;
use conduit_std_host::todo_durable_resource::{
    CheckpointIdentity, Refusal, SelectedTodoResidence, AUTHORITY_CONTRACT, CHECKPOINT_MAX_BYTES,
    PUBLISH_OPERATION, READ_OPERATION,
};
use conduit_todo_plot::{TodoCommand, TodoState};
use std::path::PathBuf;

fn placement(host: &str, boot: &str, write: bool, generation: u8) -> PlannedGear {
    let host: HostId = host.into();
    let boot: BootId = boot.into();
    let kind = kind_id(if write {
        "todo/checkpoint-publish"
    } else {
        "todo/checkpoint-read"
    });
    let operation = if write {
        PUBLISH_OPERATION
    } else {
        READ_OPERATION
    };
    let capability: CapabilityId = operation.into();
    let access = if write {
        ResourceAccessMode::WriteCandidatePublish
    } else {
        ResourceAccessMode::ReadPublished
    };
    planned_gear_from_parts! {
        semantic_contract: Default::default(),
        placement_id: "todo-checkpoint".into(),
        gear_id: "todo-checkpoint".into(),
        kind_id: kind.clone(),
        kind_contract_revision: "todo/checkpoint@1".into(),
        execution_profile_id: "std/todo-checkpoint@1".into(),
        configuration: Default::default(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        offer_generation: OfferGeneration(1),
        capability_id: capability.clone(),
        implementation_id: "std/todo-checkpoint@1".into(),
        artifact_id: "std/todo-checkpoint@1".into(),
        base: None,
        realization_characteristics: Vec::new(),
        limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 4096 },
        inputs: Vec::new(),
        outputs: Vec::new(),
        terminal_transductions: Vec::new(),
        host_calls: vec![HostCallRequirement {
            contract_id: operation.into(),
            target_kind: Some(kind.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: 4096,
            maximum_output_bytes: 4096,
        }],
        resources: vec![ResourceBinding {
            pool_id: "shared-checkpoint".into(),
            class_id: "resource/todo-checkpoint@1".into(),
            units: 1,
            compute: None,
            protected: None,
            content: Some(ResourceContentOffer {
                contract: ResourceContentRequirement {
                    identity: ResourceSemanticIdentity::from_digest([1; 32]),
                    version: ResourceVersionIdentity::from_digest([generation; 32]),
                    content_profile: kind_id("conduit.todo/checkpoint-envelope@1"),
                    maximum_bytes: CHECKPOINT_MAX_BYTES as u32,
                    maximum_items: 1,
                    retention: ResourceRetention::ExternalDurable,
                    sharing: ResourceSharing::SingleWriterPublished,
                    access,
                    generation_slots: 1,
                    reader_leases: 1,
                    publication_slots: u16::from(write),
                    sensitive: false,
                },
                owner_host: host.clone(),
                owner_boot: boot.clone(),
                base_id: "std/explicit-shared-checkpoint".into(),
                residence_profile: kind_id("std/explicit-shared-checkpoint@1"),
            }),
        }],
        authority: vec![AuthorityBinding {
            grant_id: format!("grant/{}/{}/{operation}", host.as_str(), boot.as_str()).into(),
            contract_id: AUTHORITY_CONTRACT.into(),
            host_call_contract_id: operation.into(),
            subject_kind: kind,
            host_id: host,
            boot_id: boot,
            capability_id: capability,
        }],
        pool_references: Vec::new(),
    }
}

fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "conduit-todo-durable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
fn identity() -> CheckpointIdentity {
    CheckpointIdentity {
        body: "body-1".into(),
        plot: "checked-todo-1".into(),
        workload: "revision-1".into(),
    }
}

#[test]
fn committed_todo_recovers_on_second_selected_host_and_refuses_wrong_grants() {
    let root = root();
    let writer_plan = placement("host-a", "boot-a", true, 2);
    let reader_plan = placement("host-b", "boot-b", false, 2);
    let writer = SelectedTodoResidence::prepare(&root, &writer_plan, identity()).unwrap();
    let reader = SelectedTodoResidence::prepare(&root, &reader_plan, identity()).unwrap();
    let initial = TodoState::new("Groceries".into()).unwrap();
    let next = initial
        .apply(&TodoCommand::Add {
            text: "Buy milk".into(),
        })
        .unwrap();
    assert!(matches!(
        reader.recover(&reader_plan.authority[0]),
        Err(Refusal::Missing)
    ));
    assert_eq!(
        writer.commit(&reader_plan.authority[0], &next),
        Err(Refusal::WrongAuthority)
    );
    writer.commit(&writer_plan.authority[0], &next).unwrap();
    assert_eq!(reader.recover(&reader_plan.authority[0]).unwrap(), next);
    assert_eq!(
        writer.commit(&writer_plan.authority[0], &next),
        Err(Refusal::StaleRevision)
    );
    assert!(matches!(
        reader.recover(&writer_plan.authority[0]),
        Err(Refusal::WrongAuthority)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_current_generation_refuses_without_falling_back() {
    let root = root();
    let writer_plan = placement("host-a", "boot-a", true, 2);
    let reader_plan = placement("host-b", "boot-b", false, 2);
    let writer = SelectedTodoResidence::prepare(&root, &writer_plan, identity()).unwrap();
    let reader = SelectedTodoResidence::prepare(&root, &reader_plan, identity()).unwrap();
    let state = TodoState::new("Groceries".into())
        .unwrap()
        .apply(&TodoCommand::Add {
            text: "Buy milk".into(),
        })
        .unwrap();
    writer.commit(&writer_plan.authority[0], &state).unwrap();
    let checkpoint = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "checkpoint"))
        .unwrap();
    std::fs::write(checkpoint, b"corrupt").unwrap();
    assert!(matches!(
        reader.recover(&reader_plan.authority[0]),
        Err(Refusal::Corrupt)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn selector_failure_keeps_previous_checkpoint_current() {
    let root = root();
    let writer_plan = placement("host-a", "boot-a", true, 2);
    let reader_plan = placement("host-b", "boot-b", false, 2);
    let writer = SelectedTodoResidence::prepare(&root, &writer_plan, identity()).unwrap();
    let reader = SelectedTodoResidence::prepare(&root, &reader_plan, identity()).unwrap();
    let first = TodoState::new("Groceries".into())
        .unwrap()
        .apply(&TodoCommand::Add {
            text: "Buy milk".into(),
        })
        .unwrap();
    writer.commit(&writer_plan.authority[0], &first).unwrap();
    let next = first
        .apply(&TodoCommand::Add {
            text: "Cat food".into(),
        })
        .unwrap();
    let selector = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "current"))
        .unwrap();
    let staged = selector.with_extension("next");
    std::fs::create_dir(&staged).unwrap();
    let next_writer_plan = placement("host-a", "boot-a", true, 3);
    let next_writer = SelectedTodoResidence::prepare(&root, &next_writer_plan, identity()).unwrap();
    assert_eq!(
        next_writer.commit(&next_writer_plan.authority[0], &next),
        Err(Refusal::Storage)
    );
    assert_eq!(reader.recover(&reader_plan.authority[0]).unwrap(), first);
    std::fs::remove_dir(&staged).unwrap();
    next_writer
        .commit(&next_writer_plan.authority[0], &next)
        .unwrap();
    let next_reader_plan = placement("host-b", "boot-c", false, 3);
    let next_reader = SelectedTodoResidence::prepare(&root, &next_reader_plan, identity()).unwrap();
    assert_eq!(
        next_reader.recover(&next_reader_plan.authority[0]).unwrap(),
        next
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn next_generation_requires_fresh_planned_version_and_read_grant() {
    let root = root();
    let first_write = placement("host-a", "boot-a", true, 2);
    let first_read = placement("host-b", "boot-b", false, 2);
    let next_write = placement("host-a", "boot-a", true, 3);
    let next_read = placement("host-b", "boot-c", false, 3);
    let first = TodoState::new("Groceries".into())
        .unwrap()
        .apply(&TodoCommand::Add {
            text: "Buy milk".into(),
        })
        .unwrap();
    let next = first
        .apply(&TodoCommand::Add {
            text: "Cat food".into(),
        })
        .unwrap();
    let first_writer = SelectedTodoResidence::prepare(&root, &first_write, identity()).unwrap();
    let first_reader = SelectedTodoResidence::prepare(&root, &first_read, identity()).unwrap();
    first_writer
        .commit(&first_write.authority[0], &first)
        .unwrap();
    let next_writer = SelectedTodoResidence::prepare(&root, &next_write, identity()).unwrap();
    next_writer.commit(&next_write.authority[0], &next).unwrap();
    assert_eq!(
        first_reader.recover(&first_read.authority[0]),
        Err(Refusal::StaleRevision)
    );
    let next_reader = SelectedTodoResidence::prepare(&root, &next_read, identity()).unwrap();
    assert_eq!(next_reader.recover(&next_read.authority[0]).unwrap(), next);
    std::fs::remove_dir_all(root).unwrap();
}
