//! A deterministic send-burst limit catches the original three-frame burst,
//! independent of whether the other worker happens to drain its queue quickly.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc,
};
enum Packet {
    Frame(Vec<u8>),
    Closed,
}
struct Carrier {
    send: Option<mpsc::SyncSender<Packet>>,
    receive: mpsc::Receiver<Packet>,
    burst: usize,
    maximum: Arc<AtomicUsize>,
}
impl p::ProtectedFrameCarrier for Carrier {
    fn send_frame(&mut self, frame: &[u8]) -> std::result::Result<(), p::CarrierFailure> {
        self.burst += 1;
        self.maximum.fetch_max(self.burst, Ordering::SeqCst);
        if self.burst > 2 || frame.len() > 4130 {
            return Err(p::CarrierFailure::Pressure);
        }
        self.send
            .as_ref()
            .ok_or(p::CarrierFailure::Lost)?
            .try_send(Packet::Frame(frame.to_vec()))
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => p::CarrierFailure::Pressure,
                mpsc::TrySendError::Disconnected(_) => p::CarrierFailure::Lost,
            })
    }
    fn receive_frame(
        &mut self,
        out: &mut [u8],
        timeout: u32,
    ) -> std::result::Result<usize, p::CarrierFailure> {
        let frame = self
            .receive
            .recv_timeout(std::time::Duration::from_millis(u64::from(timeout)))
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => p::CarrierFailure::TimedOut,
                mpsc::RecvTimeoutError::Disconnected => p::CarrierFailure::Lost,
            })?;
        let frame = match frame {
            Packet::Frame(frame) => frame,
            Packet::Closed => return Err(p::CarrierFailure::Closed),
        };
        if frame.len() > out.len() {
            return Err(p::CarrierFailure::Pressure);
        }
        out[..frame.len()].copy_from_slice(&frame);
        self.burst = 0;
        Ok(frame.len())
    }
    fn close(&mut self) -> std::result::Result<(), p::CarrierFailure> {
        self.send
            .take()
            .ok_or(p::CarrierFailure::Lost)?
            .try_send(Packet::Closed)
            .map_err(|_| p::CarrierFailure::Lost)
    }
}
fn exchange(disconnect: bool, cursor: u32) {
    let lines = [
        line_offer("collector", "source", "request", true),
        line_offer("source", "collector", "response", true),
    ];
    let prepared = crate::history_relay_plan::prepare_proof_plan(
        Archive {
            zero: vec![],
            end: vec![],
        },
        &lines,
    )
    .unwrap();
    let connections: Vec<_> = prepared
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.connections)
        .collect();
    let request = &connections
        .iter()
        .find(|c| c.sink_placement_id == prepared.provider_placement)
        .unwrap()
        .connection_id;
    let response = &connections
        .iter()
        .find(|c| c.source_placement_id == prepared.provider_placement)
        .unwrap()
        .connection_id;
    let (a, b, policy) = super::local_binding::sessions(false);
    let (tx_a, rx_a) = mpsc::sync_channel(2);
    let (tx_b, rx_b) = mpsc::sync_channel(2);
    let max_a = Arc::new(AtomicUsize::new(0));
    let max_b = Arc::new(AtomicUsize::new(0));
    let mut collector = HistoryRelay::from_plan(
        ProtectedCarrier::new(
            Carrier {
                send: Some(tx_a),
                receive: rx_b,
                burst: 0,
                maximum: max_a.clone(),
            },
            a,
            policy,
        )
        .unwrap(),
        &prepared.plan,
        request,
        response,
        true,
    )
    .unwrap();
    let mut source = HistoryRelay::from_plan(
        ProtectedCarrier::new(
            Carrier {
                send: Some(tx_b),
                receive: rx_a,
                burst: 0,
                maximum: max_b.clone(),
            },
            b,
            policy,
        )
        .unwrap(),
        &prepared.plan,
        request,
        response,
        false,
    )
    .unwrap();
    let source = std::thread::spawn(move || {
        source.activate().unwrap();
        source
            .serve_one(|request| {
                assert_eq!(h::owner::decode_request(request).unwrap(), cursor);
                Ok(codec::encode_result(
                    &h::HistoryReadContract::prepare().unwrap(),
                    b"signed batch fixture",
                )
                .unwrap())
            })
            .unwrap();
        let closed = source.serve_one(|_| panic!("closure must not dispatch another read"));
        if disconnect {
            assert!(closed.is_err());
        } else {
            assert!(matches!(closed.unwrap(), ServeOutcome::InputClosed));
            source.finish_source().unwrap();
        }
    });
    collector.activate().unwrap();
    let contract = h::HistoryReadContract::prepare().unwrap();
    let result = collector
        .read(&codec::encode_request(&contract, cursor).unwrap())
        .unwrap();
    assert_eq!(
        codec::decode_result(&contract, &result).unwrap(),
        b"signed batch fixture"
    );
    if disconnect {
        drop(collector);
    } else {
        collector.close().unwrap();
    }
    source.join().unwrap();
    assert_eq!(max_a.load(Ordering::SeqCst), 2);
    assert_eq!(max_b.load(Ordering::SeqCst), 2);
}

#[test]
fn two_frame_queues_and_single_inflight_need_no_hidden_capacity() {
    exchange(false, 0);
}
#[test]
fn resumed_cursor_one_closes_after_one_exchange_without_count_heuristics() {
    exchange(false, 1);
}
#[test]
fn unexpected_disconnect_after_one_exchange_is_not_normal_completion() {
    exchange(true, 1);
}
