//! Courier protocol tests only; synthetic bytes claim no checked Plan or Play.
use super::*;
use std::{cell::Cell, collections::VecDeque, rc::Rc};

struct Fake {
    incoming: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
    timeouts: Vec<Duration>,
    elapsed: Rc<Cell<Duration>>,
    fail_send: Option<usize>,
    receive_delays: VecDeque<Duration>,
}
impl Frames for Fake {
    fn send(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), ()> {
        self.timeouts.push(timeout);
        self.elapsed
            .set(self.elapsed.get() + Duration::from_millis(1));
        if self.fail_send == Some(self.sent.len()) {
            return Err(());
        }
        self.sent.push(bytes.to_vec());
        Ok(())
    }
    fn receive<'a>(&mut self, out: &'a mut [u8], timeout: Duration) -> Result<&'a [u8], ()> {
        self.timeouts.push(timeout);
        self.elapsed.set(
            self.elapsed.get()
                + self
                    .receive_delays
                    .pop_front()
                    .unwrap_or(Duration::from_millis(1)),
        );
        let bytes = self.incoming.pop_front().ok_or(())?;
        let target = out.get_mut(..bytes.len()).ok_or(())?;
        target.copy_from_slice(&bytes);
        Ok(target)
    }
}
fn challenge() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "body_id":"body", "part_id":"part", "credential_id":"credential",
        "receiver_host":"host", "receiver_boot":"boot", "receiver_generation":1,
        "receiver_profile":"profile", "issuer_host":"owner", "issuer_boot":"owner-boot",
        "issuer_generation":1, "issuer_key":vec![1;32], "payload_contract":HOST_PREPARED_WORKSET_CONTRACT,
        "maximum_payload_bytes":4096, "nonce":vec![2;32], "issued_at_millis":100, "expires_at_millis":10100,
    })).unwrap()
}
fn report(digest: [u8; 32]) -> Vec<u8> {
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    serde_json::to_vec(
        &serde_json::json!({"phase":"input-retained", "custody":"ram", "payload_sha256":hex,
        "resources":"not-admitted", "start":"not-started"}),
    )
    .unwrap()
}
fn fake(incoming: Vec<Vec<u8>>) -> Fake {
    Fake {
        incoming: incoming.into(),
        sent: vec![],
        timeouts: vec![],
        elapsed: Rc::new(Cell::new(Duration::ZERO)),
        fail_send: None,
        receive_delays: VecDeque::new(),
    }
}
fn run<'a>(
    frames: &mut Fake,
    timeout: Duration,
    approve: impl FnOnce(&HostCommandChallenge<'_>) -> Result<PreparationCommandBytes<'a>, ()>,
) -> Result<PreparationExchange, PreparationTransportError> {
    let start = Instant::now();
    let elapsed = frames.elapsed.clone();
    exchange(frames, timeout, 16 * 1024, approve, || {
        start + elapsed.get()
    })
}
#[test]
fn exact_chunks_signature_digest_and_one_original_deadline() {
    let payload = [42; 2048];
    let signature = [9; 64];
    let digest = Sha256::digest(payload).into();
    let mut frames = fake(vec![challenge(), report(digest)]);
    assert_eq!(
        run(&mut frames, Duration::from_secs(1), |challenge| {
            assert_eq!(challenge.body_id, "body");
            Ok(PreparationCommandBytes {
                payload: &payload,
                signature: &signature,
            })
        }),
        Ok(PreparationExchange::InputReported(
            ReportedPreparationInput {
                payload_sha256: digest
            }
        ))
    );
    assert_eq!(frames.sent.first().unwrap(), HOST_PREPARATION_REQUEST);
    assert_eq!(frames.sent.last().unwrap(), &signature);
    let mut storage = [0; 4096];
    let mut assembly = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut storage, 64);
    for chunk in &frames.sent[1..frames.sent.len() - 1] {
        assembly.push(chunk).unwrap();
    }
    let (received, actual_digest) = assembly.completed().unwrap();
    assert_eq!(received, payload);
    assert_eq!(*actual_digest, digest);
    assert!(frames.timeouts.windows(2).all(|pair| pair[1] < pair[0]));
    assert!(frames.sent.len() < 64);
}
#[test]
fn occupied_input_never_asks_for_another_approval() {
    let digest = [7; 32];
    let mut frames = fake(vec![report(digest)]);
    assert_eq!(
        run(&mut frames, Duration::from_secs(1), |_| panic!(
            "occupied receiver asked for approval"
        )),
        Ok(PreparationExchange::Occupied(ReportedPreparationInput {
            payload_sha256: digest
        }))
    );
    assert_eq!(frames.sent.len(), 1);
}
#[test]
fn approval_time_and_payload_bounds_refuse_before_submission() {
    let payload = [1; 4097];
    let signature = [2; 64];
    let mut frames = fake(vec![challenge()]);
    assert_eq!(
        run(&mut frames, Duration::from_secs(1), |_| Ok(
            PreparationCommandBytes {
                payload: &payload,
                signature: &signature
            }
        )),
        Err(PreparationTransportError::Bounds)
    );
    assert_eq!(frames.sent.len(), 1);
    let mut frames = fake(vec![challenge()]);
    let elapsed = frames.elapsed.clone();
    assert_eq!(
        run(&mut frames, Duration::from_millis(10), |_| {
            elapsed.set(Duration::from_millis(11));
            Ok(PreparationCommandBytes {
                payload: &payload[..10],
                signature: &signature,
            })
        }),
        Err(PreparationTransportError::Deadline)
    );
    assert_eq!(frames.sent.len(), 1);
    let mut frames = fake(vec![]);
    assert_eq!(
        run(&mut frames, Duration::ZERO, |_| panic!(
            "zero deadline approved"
        )),
        Err(PreparationTransportError::Deadline)
    );
    assert!(frames.sent.is_empty());
}
#[test]
fn lost_or_wrong_or_late_final_report_is_unknown_without_retry() {
    let payload = [42; 1024];
    let signature = [9; 64];
    let digest = Sha256::digest(payload).into();
    for kind in 0..4 {
        let mut frames = fake(match kind {
            0 => vec![challenge()],
            1 => vec![challenge(), report([0; 32])],
            2 => vec![challenge(), b"{\"phase\":\"started\"}".to_vec()],
            _ => vec![challenge(), report(digest)],
        });
        if kind == 3 {
            frames.receive_delays = [Duration::from_millis(1), Duration::from_secs(2)].into();
        }
        assert_eq!(
            run(&mut frames, Duration::from_secs(1), |_| Ok(
                PreparationCommandBytes {
                    payload: &payload,
                    signature: &signature
                }
            )),
            Err(PreparationTransportError::OutcomeUnknown)
        );
        assert_eq!(
            frames
                .sent
                .iter()
                .filter(|frame| frame.as_slice() == HOST_PREPARATION_REQUEST)
                .count(),
            1
        );
    }
    let mut frames = fake(vec![challenge()]);
    frames.fail_send = Some(1);
    assert_eq!(
        run(&mut frames, Duration::from_secs(1), |_| Ok(
            PreparationCommandBytes {
                payload: &payload,
                signature: &signature
            }
        )),
        Err(PreparationTransportError::OutcomeUnknown)
    );
}
#[test]
fn receiver_lifetime_can_only_shorten_the_original_budget() {
    let mut value: serde_json::Value = serde_json::from_slice(&challenge()).unwrap();
    value["expires_at_millis"] = serde_json::json!(105);
    let mut frames = fake(vec![serde_json::to_vec(&value).unwrap()]);
    let payload = [1; 10];
    let signature = [2; 64];
    assert_eq!(
        run(&mut frames, Duration::from_secs(1), |_| Ok(
            PreparationCommandBytes {
                payload: &payload,
                signature: &signature
            }
        )),
        Err(PreparationTransportError::OutcomeUnknown)
    );
    assert!(frames
        .timeouts
        .iter()
        .skip(2)
        .all(|timeout| *timeout <= Duration::from_millis(5)));
}

#[test]
fn malformed_or_overwide_challenge_and_failed_approval_send_no_document() {
    for mutation in 0..4 {
        let mut value: serde_json::Value = serde_json::from_slice(&challenge()).unwrap();
        match mutation {
            0 => value["maximum_payload_bytes"] = serde_json::json!(16385),
            1 => value["payload_contract"] = serde_json::json!("other-contract"),
            2 => value["expires_at_millis"] = serde_json::json!(100),
            _ => value["expires_at_millis"] = serde_json::json!(10101),
        }
        let mut frames = fake(vec![serde_json::to_vec(&value).unwrap()]);
        assert_eq!(
            run(&mut frames, Duration::from_secs(1), |_| panic!(
                "invalid challenge reached approval"
            )),
            Err(PreparationTransportError::Challenge)
        );
        assert_eq!(frames.sent.len(), 1);
    }
    let mut frames = fake(vec![challenge()]);
    assert_eq!(
        run(&mut frames, Duration::from_secs(1), |_| Err(())),
        Err(PreparationTransportError::ApprovalRefused)
    );
    assert_eq!(frames.sent.len(), 1);
}

#[test]
fn bounded_document_ceiling_is_independent_of_frame_ceiling() {
    let mut value: serde_json::Value = serde_json::from_slice(&challenge()).unwrap();
    value["maximum_payload_bytes"] = serde_json::json!(16384);
    let mut frames = fake(vec![serde_json::to_vec(&value).unwrap()]);
    let mut approved = false;
    assert_eq!(
        run(&mut frames, Duration::from_secs(1), |_| {
            approved = true;
            Err(())
        }),
        Err(PreparationTransportError::ApprovalRefused)
    );
    assert!(approved);
    assert_eq!(frames.sent.len(), 1);
}

#[test]
fn explicit_caller_extent_carries_a_larger_document_without_larger_frames() {
    let payload = vec![42; 17 * 1024];
    let signature = [9; 64];
    let digest = Sha256::digest(&payload).into();
    let mut value: serde_json::Value = serde_json::from_slice(&challenge()).unwrap();
    value["maximum_payload_bytes"] = serde_json::json!(18432);
    let mut frames = fake(vec![serde_json::to_vec(&value).unwrap(), report(digest)]);
    let start = Instant::now();
    let elapsed = frames.elapsed.clone();
    assert_eq!(
        exchange(
            &mut frames,
            Duration::from_secs(1),
            18432,
            |_| {
                Ok(PreparationCommandBytes {
                    payload: &payload,
                    signature: &signature,
                })
            },
            || start + elapsed.get()
        ),
        Ok(PreparationExchange::InputReported(
            ReportedPreparationInput {
                payload_sha256: digest
            }
        ))
    );
    let mut storage = vec![0; 18432];
    let mut assembly = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut storage, 64);
    for chunk in &frames.sent[1..frames.sent.len() - 1] {
        assert!(chunk.len() <= PREPARATION_FRAME_BYTES);
        assembly.push(chunk).unwrap();
    }
    assert_eq!(assembly.completed().unwrap().0, payload.as_slice());
}

#[test]
fn caller_extent_refuses_unfunded_policy_and_receiver_claims_before_approval() {
    for maximum in [0, MAX_PREPARATION_DOCUMENT_BYTES + 1] {
        let mut frames = fake(vec![]);
        assert_eq!(
            exchange(
                &mut frames,
                Duration::from_secs(1),
                maximum,
                |_| panic!("unfunded approval"),
                Instant::now
            ),
            Err(PreparationTransportError::Bounds)
        );
        assert!(frames.sent.is_empty());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&challenge()).unwrap();
    value["maximum_payload_bytes"] = serde_json::json!(18433);
    let mut frames = fake(vec![serde_json::to_vec(&value).unwrap()]);
    assert_eq!(
        exchange(
            &mut frames,
            Duration::from_secs(1),
            18432,
            |_| panic!("oversized receiver approval"),
            Instant::now
        ),
        Err(PreparationTransportError::Challenge)
    );
    assert_eq!(frames.sent.len(), 1);
}
