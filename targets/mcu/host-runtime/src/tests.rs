use super::{seed::Seed, transport::*};
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use std::{collections::VecDeque, vec, vec::Vec};
pub(crate) fn ready<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fixture must not suspend"),
    }
}
#[derive(Default)]
struct Fake {
    input: VecDeque<Vec<u8>>,
    output: Vec<Vec<u8>>,
    deadlines: Vec<u32>,
}
impl Carrier for Fake {
    type Error = ();
    async fn connected(&mut self) {}
    async fn read(&mut self, output: &mut [u8], timeout: u32) -> Result<usize, ()> {
        self.deadlines.push(timeout);
        let packet = self.input.pop_front().ok_or(())?;
        output[..packet.len()].copy_from_slice(&packet);
        Ok(packet.len())
    }
    async fn write(&mut self, bytes: &[u8], timeout: u32) -> Result<(), ()> {
        self.deadlines.push(timeout);
        self.output.push(bytes.to_vec());
        Ok(())
    }
}
#[test]
fn fragmented_receive_preserves_framing_and_read_deadline() {
    let mut link = Link::new(Fake {
        input: [vec![0], vec![3, 10], vec![11, 12, 0, 1, 99]].into(),
        ..Fake::default()
    });
    let mut output = [0; 8];
    assert_eq!(ready(link.receive(&mut output)).unwrap(), &[10, 11, 12]);
    assert_eq!(ready(link.receive(&mut output)).unwrap(), &[99]);
    assert_eq!(link.carrier.deadlines, vec![READ_TIMEOUT_MILLIS; 3]);
}
#[test]
fn send_keeps_header_packet_chunks_and_usb_termination() {
    let mut link = Link::new(Fake::default());
    assert_eq!(ready(link.send(&[])), Err(Refusal::PayloadExtent));
    assert!(link.carrier.output.is_empty());
    ready(link.send(&[7; 64])).unwrap();
    assert_eq!(link.carrier.output, vec![vec![0, 64], vec![7; 64], vec![]]);
    assert_eq!(link.carrier.deadlines, vec![WRITE_TIMEOUT_MILLIS; 3]);
}
#[test]
fn malformed_header_refuses_and_reset_discards_partial_frame() {
    let mut link = Link::new(Fake {
        input: [vec![0, 0], vec![0, 1, 42]].into(),
        ..Fake::default()
    });
    assert_eq!(ready(link.receive(&mut [0; 8])), Err(Refusal::Framing));
    link.reset();
    assert_eq!(ready(link.receive(&mut [0; 8])).unwrap(), &[42]);
}
fn carrier() -> Vec<u8> {
    let value = serde_json::json!({
        "schema":"conduit.body/member-bootstrap@2", "body_id":"body/1", "host_id":"host/1",
        "image_id":"image/1", "configuration_id":"configuration/1",
        "owner":{"host_id":"owner/1", "boot_id":"boot/1", "offer_generation":1, "verifying_key":vec![9;32]},
        "admission":{"retained":{"credential":{"credential_id":"credential/1", "body_id":"body/1", "part_id":"part/1", "host_id":"host/1", "boot_id":"boot/old", "issued_at_millis":7}, "secret":vec![3;32]}}
    });
    let payload = serde_json::to_vec(&value).unwrap();
    let mut bytes = b"CONDUIT_SEED@2\0\0".to_vec();
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}
#[test]
fn seed_retains_existing_member_and_rejects_scope_extent_or_trailing_payload() {
    let mut bytes = carrier();
    let seed = Seed::parse(&bytes, "host/1", "image/1", "configuration/1").unwrap();
    assert_eq!(
        seed.admission.credential().unwrap().part_id.as_str(),
        "part/1"
    );
    assert!(seed.admission.spawn().is_none());
    drop(seed);
    for (host, image, configuration) in [
        ("other", "image/1", "configuration/1"),
        ("host/1", "other", "configuration/1"),
        ("host/1", "image/1", "other"),
    ] {
        assert!(Seed::parse(&bytes, host, image, configuration).is_none());
    }
    bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Seed::parse(&bytes, "host/1", "image/1", "configuration/1").is_none());
    let mut bytes = carrier();
    bytes.push(b'x');
    let length = (bytes.len() - 20) as u32;
    bytes[16..20].copy_from_slice(&length.to_le_bytes());
    assert!(Seed::parse(&bytes, "host/1", "image/1", "configuration/1").is_none());
}

#[test]
fn in_place_root_keeps_original_scope_and_refuses_reinitialization() {
    use crate::root::{Configuration, Refusal, RootStorage};
    let bytes = carrier();
    let seed = || Seed::parse(&bytes, "host/1", "image/1", "configuration/1").unwrap();
    let actual = || conduit_core::HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "host/1".into(),
        boot_id: "boot/current".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "configuration/1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let config = || Configuration {
        image_id: "image/1",
        configuration_id: "configuration/1",
        friendly_label: "fixture",
        preparation: None,
        execution: None,
    };
    let mut storage = RootStorage::<16384>::new();
    let mut foreign = actual();
    foreign.host_id = "host/foreign".into();
    assert!(matches!(
        storage.initialize(seed(), foreign, config()),
        Err(Refusal::Scope)
    ));
    let root = storage.initialize(seed(), actual(), config()).unwrap();
    assert_eq!(root.actual().host_id.as_str(), "host/1");
    assert!(!root.retained_input());
    assert!(root.receipt_digest().is_none());
    assert!(matches!(root.execution_parts(), Err(Refusal::NotMember)));
    assert!(matches!(
        storage.initialize(seed(), actual(), config()),
        Err(Refusal::Scope)
    ));
}

#[test]
fn full_payload_and_following_header_fit_funded_wire_backing() {
    let payload = vec![7; LIMIT];
    let mut wire = Vec::from((LIMIT as u16).to_be_bytes());
    wire.extend_from_slice(&payload);
    wire.extend_from_slice(&[0, 1, 9]);
    let mut link = Link::new(Fake {
        input: wire.chunks(PACKET_BYTES).map(Vec::from).collect(),
        ..Fake::default()
    });
    let mut output = [0; LIMIT];
    assert_eq!(ready(link.receive(&mut output)).unwrap(), payload);
    assert_eq!(ready(link.receive(&mut output)).unwrap(), &[9]);
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
