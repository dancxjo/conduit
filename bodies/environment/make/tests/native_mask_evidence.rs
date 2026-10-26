//! Real volatile consumer, real canonical kernel page; no archive/HIL claim.
use conduit_core as c;
use conduit_environment_runtime::display_evidence as wire;
use conduit_kernel::{self as k, SignSink};
use sha2::{Digest, Sha256};
mod native_display {
    pub trait EvidenceSink {
        fn accept(
            &mut self,
            invocation: u64,
            active: &conduit_core::ActivePlayIdentity,
            closed: &[u8],
            signs: &conduit_kernel::FixedSignLog<64, 12>,
        ) -> Result<(), &'static str>;
    }
}
#[path = "../../../../targets/esp32/proof/tm1637/src/native_mask_evidence.rs"]
mod consumer;
use native_display::EvidenceSink;
fn log(terminal: bool) -> k::FixedSignLog<64, 12> {
    let mut log = k::FixedSignLog::new_with_remote_storage(
        64 * core::mem::size_of::<k::KernelEvent>() as u32,
        12,
        k::remote_sign_storage_bytes(12).unwrap(),
    )
    .unwrap();
    log.record_remote(
        k::NodeId(0),
        k::PortId(0),
        k::KernelEventKind::RemoteInputAdmitted,
        k::RemoteLifecycleIdentity {
            endpoint: k::RemoteEndpointId(0),
            cord: k::CordId(0),
            direction: k::RemoteCordDirection::Ingress,
            sequence: 0,
        },
    )
    .unwrap();
    log.record(
        k::NodeId(2),
        None,
        None,
        if terminal {
            k::KernelEventKind::BackCompleted
        } else {
            k::KernelEventKind::StepYielded
        },
    )
    .unwrap();
    log
}
fn active(sequence: u64) -> c::ActivePlayIdentity {
    c::bind_active_play(
        &"plan/consumer".into(),
        &"host/consumer".into(),
        &"boot/consumer".into(),
        sequence,
    )
}
fn chain(
    previous: [u8; 32],
    invocation: u64,
    active: &c::ActivePlayIdentity,
    closed: &[u8],
    log: &k::FixedSignLog<64, 12>,
) -> ([u8; 32], usize) {
    let mut page = [0; wire::MAXIMUM_BYTES];
    let length = wire::encode(invocation, log.events(), log, &mut page).unwrap();
    let parsed = wire::Page::parse(&page[..length]).unwrap();
    assert_eq!(parsed.records().count(), 2);
    assert!(parsed.records().next().unwrap().remote.is_some());
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/transient-mask-consumption@1");
    hash.update(previous);
    hash.update(invocation.to_le_bytes());
    hash.update(active.active_play_id.as_str().as_bytes());
    hash.update((closed.len() as u64).to_le_bytes());
    hash.update(Sha256::digest(closed));
    hash.update(&page[..length]);
    (hash.finalize().into(), length)
}
#[test]
fn exact_pages_chain_in_order_and_refusals_preserve_accepted_custody() {
    let mut consumer = consumer::Consumer::new();
    let signs = log(true);
    let closed = b"complete-canonical-closed-fixture";
    consumer.accept(0, &active(1), closed, &signs).unwrap();
    let (expected, length) = chain([0; 32], 0, &active(1), closed, &signs);
    let before = consumer::status();
    assert_eq!(before.consumed, 1);
    assert_eq!(before.evidence_chain, expected);
    assert_eq!(before.retained_page_bytes, length);
    assert_eq!(before.last_closed_bytes, closed.len());
    assert_eq!(
        before.last_closed_digest,
        <[u8; 32]>::from(Sha256::digest(closed))
    );
    assert!(!before.durable);
    let mut substituted = active(2);
    substituted.active_play_id = "play/substituted".into();
    for (index, play, bytes, log) in [
        (0, active(2), closed.as_slice(), log(true)),
        (2, active(2), closed.as_slice(), log(true)),
        (1, active(1), closed.as_slice(), log(true)),
        (1, substituted, closed.as_slice(), log(true)),
        (1, active(2), &[][..], log(true)),
        (1, active(2), closed.as_slice(), log(false)),
    ] {
        assert!(consumer.accept(index, &play, bytes, &log).is_err());
        let after = consumer::status();
        assert_eq!(after.consumed, before.consumed);
        assert_eq!(after.evidence_chain, before.evidence_chain);
        assert_eq!(after.retained_page_bytes, before.retained_page_bytes);
    }
    assert!(consumer.accept(1, &active(2), &[0; 8193], &signs).is_err());
    consumer.accept(1, &active(2), closed, &signs).unwrap();
    let (expected, _) = chain(expected, 1, &active(2), closed, &signs);
    assert_eq!(consumer::status().evidence_chain, expected);
    assert_eq!(consumer::status().consumed, 2);
    // A maximum-size exact page is accepted without truncation. A retained
    // terminal amid an evicted transient history is explicitly refused.
    let mut full = log(true);
    for _ in 2..64 {
        full.record(k::NodeId(0), None, None, k::KernelEventKind::StepYielded)
            .unwrap();
    }
    consumer.accept(2, &active(3), closed, &full).unwrap();
    assert_eq!(consumer::status().retained_page_bytes, wire::MAXIMUM_BYTES);
    let before_gap = consumer::status();
    full.record(k::NodeId(0), None, None, k::KernelEventKind::StepYielded)
        .unwrap();
    assert_eq!(
        consumer.accept(3, &active(4), closed, &full),
        Err("mask-consumer-evidence")
    );
    assert_eq!(consumer::status().evidence_chain, before_gap.evidence_chain);
    assert_eq!(consumer::status().consumed, before_gap.consumed);
}
