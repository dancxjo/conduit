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
fn zero_ready_read_refuses_without_repolling_the_carrier() {
    let mut link = Link::new(Packets {
        input: vec![vec![]].into(),
        ..Default::default()
    });
    let mut output = [0xa5; 32];
    assert_eq!(ready(link.receive(&mut output)), Err(Refusal::NoProgress));
    assert_eq!(link.carrier.reads, [READ_TIMEOUT_MILLIS]);
    assert_eq!(output, [0xa5; 32]);
    // Refusal does not consume a subsequent frame; the composition explicitly
    // resets interrupted framing before beginning a new exchange.
    link.carrier.input.push_back(frame(b"fresh"));
    link.reset();
    assert_eq!(ready(link.receive(&mut output)).unwrap(), b"fresh");
}
#[test]
fn zero_ready_document_read_refuses_without_repolling_or_completing_custody() {
    let mut link = Link::new(Packets {
        input: vec![vec![]].into(),
        ..Default::default()
    });
    let mut backing = [0xa5; 128];
    let mut document = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut backing, 4);
    assert_eq!(
        ready(link.receive_document(&mut document)),
        Err(Refusal::NoProgress)
    );
    assert_eq!(link.carrier.reads, [READ_TIMEOUT_MILLIS]);
    assert!(document.completed().is_none());
    assert_eq!(backing, [0xa5; 128]);
}
