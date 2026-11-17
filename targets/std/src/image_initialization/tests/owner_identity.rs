//! Fixed identity custody uses the actual canonical selected IMAGE counter.
use super::*;
use std::sync::atomic::Ordering;

#[test]
fn prepared_owner_identity_retains_original_charge_and_releases_only_on_drop() {
    let selected = image(SOURCE);
    let host = host(&selected, "boot/owner-custody").unwrap();
    let counter = host.kernel_resources.image_memory_observer();
    let before = counter.load(Ordering::Acquire);
    let identity = host.prepare_startup_owner_identity().unwrap();
    let charged = counter.load(Ordering::Acquire) - before;
    assert!(charged > 0);
    let key = identity.peer().verifying_key;
    assert_ne!(key, [0; 32]);
    assert_eq!(identity.peer().host_id, host.advertisement.host_id);
    assert_eq!(identity.peer().boot_id, host.advertisement.boot_id);
    drop(host);
    let remaining = counter.load(Ordering::Acquire);
    assert!(remaining >= charged);
    assert_eq!(identity.peer().verifying_key, key);
    drop(identity);
    // The test's observer still owns the original counter allocation itself.
    assert_eq!(counter.load(Ordering::Acquire), remaining - charged);
}

#[test]
fn identity_one_byte_short_refuses_before_owned_key_or_peer_preparation() {
    let selected = image(SOURCE);
    let initial = host(&selected, "boot/owner-custody").unwrap();
    let counter = initial.kernel_resources.image_memory_observer();
    let baseline = counter.load(Ordering::Acquire);
    let identity = initial.prepare_startup_owner_identity().unwrap();
    let required = counter.load(Ordering::Acquire);
    assert!(required > baseline);
    drop(identity);
    for (heap, accepted) in [(required, true), (required - 1, false)] {
        let source = SOURCE.replace(
            "heap_arena_bytes: 67108864",
            &format!("heap_arena_bytes: {heap}"),
        );
        let selected = image(&source);
        let current = host(&selected, "boot/owner-custody").unwrap();
        let original = current.kernel_resources.image_memory_observer();
        let before = original.load(Ordering::Acquire);
        let result = current.prepare_startup_owner_identity();
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert_eq!(original.load(Ordering::Acquire), before);
        }
        drop(result);
        assert_eq!(original.load(Ordering::Acquire), before);
    }
}

#[test]
fn joined_control_frames_refuse_shortage_and_cannot_grow_during_play() {
    use std::io::Write;
    let selected = image(SOURCE);
    let initial = host(&selected, "boot/control-frames").unwrap();
    let counter = initial.kernel_resources.image_memory_observer();
    let first = initial.prepare_startup_member_control_workspace().unwrap();
    let required = counter.load(Ordering::Acquire);
    drop(first);
    for (heap, accepted) in [(required, true), (required - 1, false)] {
        let source = SOURCE.replace(
            "heap_arena_bytes: 67108864",
            &format!("heap_arena_bytes: {heap}"),
        );
        let selected = image(&source);
        let current = host(&selected, "boot/control-frames").unwrap();
        let original = current.kernel_resources.image_memory_observer();
        let before = original.load(Ordering::Acquire);
        let result = current.prepare_startup_member_control_workspace();
        assert_eq!(result.is_ok(), accepted);
        if let Ok(mut workspace) = result {
            let charged = original.load(Ordering::Acquire);
            let capacity = crate::body_execution::STARTUP_MEMBER_CONTROL_FRAME_BYTES;
            assert_eq!(workspace.read_buffer().len(), capacity);
            let probe = crate::allocation_probe::begin();
            let mut writer = std::io::Cursor::new(workspace.write_buffer());
            writer.write_all(b"finite control frame").unwrap();
            writer.set_position(capacity as u64);
            assert!(writer.write_all(b"overflow").is_err());
            assert_eq!(probe.finish(), 0);
            assert_eq!(original.load(Ordering::Acquire), charged);
            let shared = workspace.into_shared();
            let probe = crate::allocation_probe::begin();
            let retained = std::sync::Arc::clone(&shared);
            assert_eq!(probe.finish(), 0);
            drop(shared);
            drop(current);
            assert_eq!(original.load(Ordering::Acquire), charged);
            drop(retained);
        }
        assert_eq!(original.load(Ordering::Acquire), before);
    }
}
