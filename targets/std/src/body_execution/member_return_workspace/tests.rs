use super::*;
use std::sync::atomic::Ordering;
fn host() -> StdHost {
    crate::image_binding_tests::bound_host("git:return-frame", "boot/return-frame", 0).0
}
#[test]
fn fixed_capture_is_original_charged_and_borrowed_until_scrubbed() {
    let host = host();
    let used = host.kernel_resources.image_memory_observer();
    let before = used.load(Ordering::Acquire);
    let mut frame = host.prepare_member_return_frame().unwrap();
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
