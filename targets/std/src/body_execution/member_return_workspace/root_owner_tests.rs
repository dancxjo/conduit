use super::*;
use crate::body_execution::ImageRootPreparationOwner;
use std::sync::atomic::Ordering;

#[test]
fn root_raw_frame_is_exactly_charged_and_works_with_host_on_worker() {
    let host = super::tests::host();
    let used = host.kernel_resources.image_memory_observer();
    let root = host.image_root_preparation_owner().unwrap();
    let before = used.load(Ordering::Acquire);
    let probe = crate::allocation_probe::begin();
    let mut frame = root.prepare_member_return_frame().unwrap();
    assert!(probe.finish_bytes() <= backing_bytes().unwrap());
    assert_eq!(
        used.load(Ordering::Acquire),
        before + backing_bytes().unwrap()
    );
    let (release, released) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        released.recv().unwrap();
        host
    });
    {
        let captured = frame.capture_with_root(&root, &mut &b"raw"[..]).unwrap();
        assert_eq!(captured.as_bytes(), b"raw");
    }
    assert!(frame.bytes.iter().all(|byte| *byte == 0));
    release.send(()).unwrap();
    let _host = worker.join().unwrap();
    drop(frame);
    assert_eq!(used.load(Ordering::Acquire), before);
}
struct NeverRead;
impl Read for NeverRead {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        panic!("wrong or retired original owner reached reader")
    }
}
#[test]
fn equal_labels_different_counter_and_retired_owner_refuse_before_read() {
    let host = super::tests::host();
    let other = super::tests::host();
    assert_eq!(host.image_identity(), other.image_identity());
    let root = host.image_root_preparation_owner().unwrap();
    let other_root = other.image_root_preparation_owner().unwrap();
    let mut frame = root.prepare_member_return_frame().unwrap();
    assert!(matches!(
        frame.capture_with_root(&other_root, &mut NeverRead),
        Err(MemberReturnFrameRefusal::WrongOriginalOwner)
    ));
    root.invalidate();
    assert!(matches!(
        frame.capture_with_root(&root, &mut NeverRead),
        Err(MemberReturnFrameRefusal::WrongOriginalOwner)
    ));
    let probe = crate::allocation_probe::begin();
    assert!(matches!(
        root.prepare_member_return_frame(),
        Err(MemberReturnFrameRefusal::Unavailable)
    ));
    assert!(probe.finish_bytes() < 1024);
}
struct RetireDuringRead<'a>(&'a ImageRootPreparationOwner);
impl Read for RetireDuringRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.0.invalidate();
        out[0] = 42;
        Ok(1)
    }
}
#[test]
fn retirement_during_read_refuses_and_scrubs_complete_buffer() {
    let host = super::tests::host();
    let root = host.image_root_preparation_owner().unwrap();
    let mut frame = root.prepare_member_return_frame().unwrap();
    // One finite read then EOF ensures post-capture verification is exercised.
    let mut reader = RetireDuringRead(&root).take(1);
    assert!(matches!(
        frame.capture_with_root(&root, &mut reader),
        Err(MemberReturnFrameRefusal::WrongOriginalOwner)
    ));
    assert!(frame.bytes.iter().all(|byte| *byte == 0));
}
#[test]
fn original_counter_one_byte_short_refuses_before_frame_allocation() {
    let host = super::tests::host();
    let used = host.kernel_resources.image_memory_observer();
    let root = host.image_root_preparation_owner().unwrap();
    let before = used.load(Ordering::Acquire);
    let padding = host
        .kernel_resources
        .fund_tick_preparation(64 * 1024 * 1024 - before - backing_bytes().unwrap() + 1)
        .unwrap();
    let prior = used.load(Ordering::Acquire);
    let probe = crate::allocation_probe::begin();
    assert!(matches!(
        root.prepare_member_return_frame(),
        Err(MemberReturnFrameRefusal::Unavailable)
    ));
    assert!(probe.finish_bytes() < 1024);
    assert_eq!(used.load(Ordering::Acquire), prior);
    drop(padding);
    assert_eq!(used.load(Ordering::Acquire), before);
}
struct UnwindRead;
impl Read for UnwindRead {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        out[0] = 77;
        panic!("test reader unwind")
    }
}
#[test]
fn unwind_scrubs_but_charge_survives_until_frame_drop() {
    let host = super::tests::host();
    let used = host.kernel_resources.image_memory_observer();
    let root = host.image_root_preparation_owner().unwrap();
    let before = used.load(Ordering::Acquire);
    let mut frame = root.prepare_member_return_frame().unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = frame.capture_with_root(&root, &mut UnwindRead);
    }))
    .is_err());
    assert!(frame.bytes.iter().all(|byte| *byte == 0));
    assert_eq!(
        used.load(Ordering::Acquire),
        before + backing_bytes().unwrap()
    );
    drop(frame);
    assert_eq!(used.load(Ordering::Acquire), before);
}
