//! Synthetic carriage proof; these fixtures carry no signing authority.
use super::*;
use std::{cell::Cell, collections::VecDeque, rc::Rc};

struct Fake {
    incoming: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
    timeouts: Vec<Duration>,
    elapsed: Rc<Cell<Duration>>,
    fail_send: Option<usize>,
    receive_delay: Duration,
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
        self.elapsed.set(self.elapsed.get() + self.receive_delay);
        let bytes = self.incoming.pop_front().ok_or(())?;
        let target = out.get_mut(..bytes.len()).ok_or(())?;
        target.copy_from_slice(&bytes);
        Ok(target)
    }
}
fn chunks(bytes: &[u8], frame_bytes: usize) -> Vec<Vec<u8>> {
    let digest = Sha256::digest(bytes).into();
    let mut frame = vec![0; frame_bytes];
    let mut offset = 0;
    let mut result = vec![];
    while offset < bytes.len() {
        let (n, next) =
            conduit_wire::encode_bounded_document_chunk(&mut frame, bytes, offset, &digest)
                .unwrap();
        result.push(frame[..n].to_vec());
        offset = next;
    }
    result
}
fn fake(incoming: Vec<Vec<u8>>) -> Fake {
    Fake {
        incoming: incoming.into(),
        sent: vec![],
        timeouts: vec![],
        elapsed: Rc::new(Cell::new(Duration::ZERO)),
        fail_send: None,
        receive_delay: Duration::from_millis(1),
    }
}
fn run(
    frames: &mut Fake,
    timeout: Duration,
) -> Result<UnverifiedContinuationPrefix, ContinuationInstallTransportError> {
    let start = Instant::now();
    let elapsed = frames.elapsed.clone();
    exchange(frames, timeout, &[42; PREFIX_BYTES], || {
        start + elapsed.get()
    })
}
#[test]
fn exact_owner_candidate_and_unverified_selected_prefix_share_one_deadline() {
    let mut selected = [42; PREFIX_BYTES];
    selected[ARTIFACT_BYTES..].fill(9);
    let mut frames = fake(chunks(&selected, FRAME_BYTES));
    assert_eq!(
        run(&mut frames, Duration::from_secs(99)).unwrap().prefix,
        selected
    );
    assert_eq!(frames.sent[0], INSTALL_REQUEST);
    assert!(frames.sent.len() <= MAX_CHUNKS + 1);
    let mut storage = [0; PREFIX_BYTES];
    let mut assembly = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut storage, MAX_CHUNKS);
    for frame in &frames.sent[1..] {
        assert!(frame.len() <= FRAME_BYTES);
        assembly.push(frame).unwrap();
    }
    let (bytes, digest) = assembly.completed().unwrap();
    assert_eq!(bytes, &[42; PREFIX_BYTES]);
    assert_eq!(Sha256::digest(bytes).as_slice(), digest);
    assert!(frames
        .timeouts
        .iter()
        .all(|timeout| *timeout <= Duration::from_secs(10)));
    assert!(frames.timeouts.windows(2).all(|pair| pair[1] < pair[0]));
}
#[test]
fn malformed_reordered_truncated_oversize_or_changed_intent_is_unknown() {
    for kind in 0..9 {
        let mut incoming = chunks(&[42; PREFIX_BYTES], FRAME_BYTES);
        match kind {
            0 => incoming[0][0] ^= 1,
            1 => incoming.swap(0, 1),
            2 => {
                incoming.pop();
            }
            3 => incoming = chunks(&[42; PREFIX_BYTES + 1], FRAME_BYTES),
            4 => incoming[0][conduit_wire::BOUNDED_DOCUMENT_HEADER_BYTES] ^= 1,
            5 => {
                let mut changed = [42; PREFIX_BYTES];
                changed[ARTIFACT_BYTES - 1] ^= 1;
                incoming = chunks(&changed, FRAME_BYTES);
            }
            6 => incoming = chunks(&[42; PREFIX_BYTES - 1], FRAME_BYTES),
            7 => incoming = chunks(&[42; PREFIX_BYTES], FRAME_BYTES + 1),
            _ => incoming[1][12] ^= 1,
        }
        let mut frames = fake(incoming);
        assert_eq!(
            run(&mut frames, Duration::from_secs(1)),
            Err(ContinuationInstallTransportError::OutcomeUnknown),
            "kind {kind}"
        );
        assert_eq!(
            frames
                .sent
                .iter()
                .filter(|b| b.as_slice() == INSTALL_REQUEST)
                .count(),
            1
        );
    }
}
#[test]
fn original_deadline_includes_sends_and_late_response() {
    let mut frames = fake(vec![]);
    assert_eq!(
        run(&mut frames, Duration::ZERO),
        Err(ContinuationInstallTransportError::Deadline)
    );
    assert!(frames.sent.is_empty());
    let mut frames = fake(vec![]);
    assert_eq!(
        run(&mut frames, Duration::from_millis(2)),
        Err(ContinuationInstallTransportError::OutcomeUnknown)
    );
    assert_eq!(frames.sent.len(), 2);
    let mut frames = fake(chunks(&[42; PREFIX_BYTES], FRAME_BYTES));
    frames.receive_delay = Duration::from_secs(11);
    assert_eq!(
        run(&mut frames, Duration::from_secs(99)),
        Err(ContinuationInstallTransportError::OutcomeUnknown)
    );
    assert_eq!(
        frames.incoming.len(),
        chunks(&[42; PREFIX_BYTES], FRAME_BYTES).len() - 1
    );
}
#[test]
fn partial_failed_sends_and_excessive_chunk_count_never_retry() {
    for failed in [0, 1, 5] {
        let mut frames = fake(vec![]);
        frames.fail_send = Some(failed);
        assert_eq!(
            run(&mut frames, Duration::from_secs(1)),
            Err(ContinuationInstallTransportError::OutcomeUnknown)
        );
        assert_eq!(frames.sent.len(), failed);
    }
    let mut frames = fake(chunks(
        &[42; PREFIX_BYTES],
        conduit_wire::BOUNDED_DOCUMENT_HEADER_BYTES + 1,
    ));
    assert_eq!(
        run(&mut frames, Duration::from_secs(1)),
        Err(ContinuationInstallTransportError::OutcomeUnknown)
    );
    assert_eq!(frames.incoming.len(), PREFIX_BYTES - MAX_CHUNKS);
}
