use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
pub(crate) fn ready<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fixture unexpectedly suspended"),
    }
}

use crate::transport::{Carrier, Link, Refusal, READ_TIMEOUT_MILLIS, WRITE_TIMEOUT_MILLIS};
use std::{collections::VecDeque, vec, vec::Vec};
#[derive(Default)]
struct Packets {
    input: VecDeque<Vec<u8>>,
    writes: Vec<(Vec<u8>, u32)>,
    reads: Vec<u32>,
}
impl Carrier for Packets {
    type Error = ();
    async fn connected(&mut self) {}
    async fn read(&mut self, output: &mut [u8], timeout: u32) -> Result<usize, ()> {
        self.reads.push(timeout);
        let packet = self.input.pop_front().ok_or(())?;
        let target = output.get_mut(..packet.len()).ok_or(())?;
        target.copy_from_slice(&packet);
        Ok(packet.len())
    }
    async fn write(&mut self, bytes: &[u8], timeout: u32) -> Result<(), ()> {
        self.writes.push((bytes.to_vec(), timeout));
        Ok(())
    }
}
fn frame(payload: &[u8]) -> Vec<u8> {
    let mut bytes = (payload.len() as u16).to_be_bytes().to_vec();
    bytes.extend_from_slice(payload);
    bytes
}
#[test]
fn fragmented_and_coalesced_messages_preserve_boundaries_and_deadlines() {
    let mut bytes = frame(b"first");
    bytes.extend(frame(b"second"));
    let mut link = Link::new(Packets {
        input: vec![
            bytes[..1].to_vec(),
            bytes[1..4].to_vec(),
            bytes[4..].to_vec(),
        ]
        .into(),
        ..Default::default()
    });
    let mut output = [0; 32];
    assert_eq!(ready(link.receive(&mut output)).unwrap(), b"first");
    assert_eq!(ready(link.receive(&mut output)).unwrap(), b"second");
    assert_eq!(link.carrier.reads, [READ_TIMEOUT_MILLIS; 3]);
    assert!(link.carrier.input.is_empty());
}
#[test]
fn trailing_empty_packet_has_the_same_finite_write_deadline() {
    let mut link = Link::new(Packets::default());
    ready(link.send(&[7; 64])).unwrap();
    assert_eq!(
        link.carrier
            .writes
            .iter()
            .map(|(bytes, _)| bytes.len())
            .collect::<Vec<_>>(),
        [2, 64, 0]
    );
    assert!(link
        .carrier
        .writes
        .iter()
        .all(|(_, timeout)| *timeout == WRITE_TIMEOUT_MILLIS));
    let effects = link.carrier.writes.len();
    assert!(matches!(ready(link.send(&[])), Err(Refusal::PayloadExtent)));
    assert!(matches!(
        ready(link.send(&[0; 4097])),
        Err(Refusal::PayloadExtent)
    ));
    assert_eq!(link.carrier.writes.len(), effects);
}
#[test]
fn explicit_link_reset_retires_only_an_interrupted_frame() {
    let mut link = Link::new(Packets {
        input: vec![vec![0, 5, b'x']].into(),
        ..Default::default()
    });
    let mut output = [0; 32];
    assert!(matches!(
        ready(link.receive(&mut output)),
        Err(Refusal::Carrier)
    ));
    link.carrier.input.push_back(frame(b"fresh"));
    link.reset();
    assert_eq!(ready(link.receive(&mut output)).unwrap(), b"fresh");
}

#[test]
fn empty_read_yields_before_retry_and_preserves_the_next_frame() {
    let mut link = Link::new(Packets {
        input: vec![vec![], frame(b"fresh")].into(),
        ..Default::default()
    });
    let mut output = [0xa5; 32];
    {
        let mut receive = core::pin::pin!(link.receive(&mut output));
        assert!(receive
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        assert_eq!(ready(receive).unwrap(), b"fresh");
    }
    assert_eq!(link.carrier.reads, [READ_TIMEOUT_MILLIS; 2]);
}
#[test]
fn empty_document_read_yields_and_preserves_the_next_document_frame() {
    let mut encoded = [0; 64];
    let payload = b"document";
    let (length, next) =
        conduit_wire::encode_bounded_document_chunk(&mut encoded, payload, 0, &[3; 32]).unwrap();
    assert_eq!(next, payload.len());
    let mut link = Link::new(Packets {
        input: vec![vec![], frame(&encoded[..length])].into(),
        ..Default::default()
    });
    let mut backing = [0xa5; 32];
    let mut document = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut backing, 2);
    {
        let mut receive = core::pin::pin!(link.receive_document(&mut document));
        assert!(receive
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        ready(receive).unwrap();
    }
    assert_eq!(link.carrier.reads, [READ_TIMEOUT_MILLIS; 2]);
    assert_eq!(document.completed().unwrap().0, payload);
}

#[derive(Default)]
struct AlwaysEmpty {
    reads: usize,
}
impl Carrier for AlwaysEmpty {
    type Error = ();
    async fn connected(&mut self) {}
    async fn read(&mut self, _: &mut [u8], _: u32) -> Result<usize, ()> {
        self.reads += 1;
        assert!(
            self.reads <= 3,
            "empty reads did not yield within the poll budget"
        );
        Ok(0)
    }
    async fn write(&mut self, _: &[u8], _: u32) -> Result<(), ()> {
        Ok(())
    }
}
#[test]
fn repeated_empty_reads_remain_cancellable_at_each_poll_in_both_paths() {
    let mut link = Link::new(AlwaysEmpty::default());
    let mut output = [0xa5; 32];
    {
        let mut receive = core::pin::pin!(link.receive(&mut output));
        for _ in 0..3 {
            assert!(receive
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
        }
    }
    assert_eq!(link.carrier.reads, 3);
    assert_eq!(output, [0xa5; 32]);
    link.carrier.reads = 0;
    let mut document = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut output, 2);
    {
        let mut receive = core::pin::pin!(link.receive_document(&mut document));
        for _ in 0..3 {
            assert!(receive
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
        }
    }
    assert_eq!(link.carrier.reads, 3);
    assert!(document.completed().is_none());
    assert_eq!(output, [0xa5; 32]);
}
