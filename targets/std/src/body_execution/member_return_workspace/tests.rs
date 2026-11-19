use super::*;
use std::sync::atomic::Ordering;
fn host() -> StdHost {
    use conduit_host_make::{
        build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
        BuildInputs,
    };
    let source = r#"host return-frame-test {
      schema = 1
      target = {architecture: "x86_64", machine: "computer", os: "linux"}
      base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
      capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
      need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}
      limits = {static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}
    }"#;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/return-frame".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    StdHost::from_selected_timer_image(
        crate::StdHostConfig {
            host_id: "std/durable".into(),
            boot_id: "boot/return-frame".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap()
}

#[test]
fn fixed_capture_is_original_charged_and_borrowed_until_scrubbed() {
    let host = host();
    let used = host.kernel_resources.image_memory_observer();
    let before = used.load(Ordering::Acquire);
    let construction = crate::allocation_probe::begin();
    let mut frame = host.prepare_member_return_frame().unwrap();
    let requested = construction.finish_bytes();
    assert!(requested <= backing_bytes().unwrap());
    assert_eq!(
        used.load(Ordering::Acquire),
        before + backing_bytes().unwrap()
    );
    let probe = crate::allocation_probe::begin();
    {
        let captured = frame
            .capture(&host, &mut &b"private raw frame"[..])
            .unwrap();
        assert_eq!(captured.as_bytes(), b"private raw frame");
    }
    assert_eq!(probe.finish(), 0);
    assert!(frame.bytes.iter().all(|b| *b == 0));
    drop(frame);
    assert_eq!(used.load(Ordering::Acquire), before);
}
#[test]
fn one_byte_short_refuses_before_frame_allocation() {
    let host = host();
    let used = host.kernel_resources.image_memory_observer();
    let before = used.load(Ordering::Acquire);
    let limit = 64 * 1024 * 1024; // exact checked fixture IMAGE limit
    let padding = host
        .kernel_resources
        .fund_tick_preparation(limit - before - backing_bytes().unwrap() + 1)
        .unwrap();
    let prior = used.load(Ordering::Acquire);
    let probe = crate::allocation_probe::begin();
    assert!(matches!(
        host.prepare_member_return_frame(),
        Err(MemberReturnFrameRefusal::Unavailable)
    ));
    let diagnostic_bytes = probe.finish_bytes();
    assert!(
        diagnostic_bytes < 1024,
        "refusal may format its bounded diagnostic, never allocate a frame"
    );
    assert_eq!(used.load(Ordering::Acquire), prior);
    drop(padding);
    assert_eq!(used.load(Ordering::Acquire), before);
}
#[test]
fn equal_labels_on_another_counter_refuse_before_read() {
    let original = host();
    let other = host();
    assert_eq!(original.image_identity(), other.image_identity());
    let mut frame = original.prepare_member_return_frame().unwrap();
    let mut reader = std::io::Cursor::new(b"private");
    assert!(matches!(
        frame.capture(&other, &mut reader),
        Err(MemberReturnFrameRefusal::WrongOriginalOwner)
    ));
    assert_eq!(reader.position(), 0);
}
#[test]
fn hosted_host_does_not_supply_a_fallback_grant() {
    assert!(matches!(
        StdHost::new().prepare_member_return_frame(),
        Err(MemberReturnFrameRefusal::NoOriginalImage)
    ));
}
#[test]
fn extents_and_io_refusals_scrub_and_never_grow_frame() {
    let host = host();
    let mut frame = host.prepare_member_return_frame().unwrap();
    let capacity = frame.bytes.capacity();
    assert!(matches!(
        frame.capture(&host, &mut &b""[..]),
        Err(MemberReturnFrameRefusal::Extent)
    ));
    let mut extra = std::io::repeat(b'x');
    assert!(matches!(
        frame.capture(&host, &mut extra),
        Err(MemberReturnFrameRefusal::Extent)
    ));
    assert_eq!(frame.bytes.capacity(), capacity);
    assert!(frame.bytes.iter().all(|b| *b == 0));
    struct Broken(bool);
    impl Read for Broken {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.0 {
                return Err(std::io::Error::from(ErrorKind::ConnectionReset));
            }
            self.0 = true;
            out[0] = 7;
            Ok(1)
        }
    }
    assert!(matches!(
        frame.capture(&host, &mut Broken(false)),
        Err(MemberReturnFrameRefusal::Read(ErrorKind::ConnectionReset))
    ));
    assert!(frame.bytes.iter().all(|b| *b == 0));
}
#[test]
fn reader_unwind_scrubs_without_releasing_reusable_reservation() {
    let host = host();
    let mut frame = host.prepare_member_return_frame().unwrap();
    let used = host.kernel_resources.image_memory_observer();
    let before = used.load(Ordering::Acquire);
    struct Panics;
    impl Read for Panics {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            out[0] = 7;
            panic!("fixture read unwind")
        }
    }
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = frame.capture(&host, &mut Panics);
    }));
    assert!(outcome.is_err());
    assert!(frame.bytes.iter().all(|b| *b == 0));
    assert_eq!(used.load(Ordering::Acquire), before);
    assert_eq!(
        frame.capture(&host, &mut &b"next"[..]).unwrap().as_bytes(),
        b"next"
    );
}

#[test]
fn quarantine_refuses_constructor_and_capture_before_reader_effects() {
    let mut host = host();
    let mut frame = host.prepare_member_return_frame().unwrap();
    let used = host.kernel_resources.image_memory_observer();
    let before = used.load(Ordering::Acquire);
    host.kernel_resources.quarantine_retirement(Vec::new());
    let mut reader = std::io::Cursor::new(b"private");
    assert!(matches!(
        frame.capture(&host, &mut reader),
        Err(MemberReturnFrameRefusal::WrongOriginalOwner)
    ));
    assert_eq!(reader.position(), 0);
    assert!(matches!(
        host.prepare_member_return_frame(),
        Err(MemberReturnFrameRefusal::Unavailable)
    ));
    assert_eq!(used.load(Ordering::Acquire), before);
}

#[test]
fn changed_advertised_boot_cannot_reuse_original_image_capture() {
    let mut host = host();
    let mut frame = host.prepare_member_return_frame().unwrap();
    host.advertisement.boot_id = "boot/different".into();
    let mut reader = std::io::Cursor::new(b"private");
    assert!(matches!(
        frame.capture(&host, &mut reader),
        Err(MemberReturnFrameRefusal::WrongOriginalOwner)
    ));
    assert_eq!(reader.position(), 0);
}

#[test]
fn exact_limit_is_complete_and_uses_no_additional_heap() {
    let host = host();
    let mut frame = host.prepare_member_return_frame().unwrap();
    assert_eq!(frame.bytes.capacity(), MEMBER_RETURN_FRAME_BYTES + 1);
    let mut reader = std::io::repeat(9).take(MEMBER_RETURN_FRAME_BYTES as u64);
    let probe = crate::allocation_probe::begin();
    {
        let captured = frame.capture(&host, &mut reader).unwrap();
        assert_eq!(captured.as_bytes().len(), MEMBER_RETURN_FRAME_BYTES);
        assert!(captured.as_bytes().iter().all(|byte| *byte == 9));
    }
    assert_eq!(probe.finish(), 0);
    assert!(frame.bytes.iter().all(|byte| *byte == 0));
}

#[test]
fn descriptive_image_identity_is_not_an_initialized_counter() {
    let (descriptive, _) =
        crate::image_binding_tests::bound_host("git:return-frame", "boot/return-frame", 0);
    assert!(descriptive.image_identity().is_some());
    assert!(matches!(
        descriptive.prepare_member_return_frame(),
        Err(MemberReturnFrameRefusal::Unavailable)
    ));
}
