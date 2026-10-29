//! Full Clean checkpoint NOR fault cuts while actual Blank keeps region custody.
use super::*;
use crate::native_body_checkpoint::Storage as CheckpointStorage;
#[derive(Clone)]
struct BodyMedia(Rc<RefCell<BodyFlash>>);
struct BodyFlash {
    bytes: Vec<u8>,
    operations: usize,
    cut: Option<(usize, u8)>,
}
impl BodyFlash {
    fn fault(&mut self) -> Option<u8> {
        self.operations += 1;
        self.cut
            .filter(|(step, _)| *step == self.operations)
            .map(|(_, mode)| mode)
    }
}
impl CheckpointStorage for BodyMedia {
    fn capacity(&self) -> u32 {
        65536
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        let mut f = self.0.borrow_mut();
        if f.fault().is_some() {
            return Err(Refusal::Storage);
        }
        out.copy_from_slice(&f.bytes[at as usize..at as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        let mut f = self.0.borrow_mut();
        let fault = f.fault();
        let n = match fault {
            Some(0) => 0,
            Some(1) => (bytes.len() / 2) & !3,
            _ => bytes.len(),
        };
        for (old, new) in f.bytes[at as usize..at as usize + n]
            .iter_mut()
            .zip(&bytes[..n])
        {
            assert_eq!(*old & new, *new);
            *old &= new;
        }
        if fault.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
    fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
        let mut f = self.0.borrow_mut();
        let fault = f.fault();
        let n = match fault {
            Some(0) => 0,
            Some(1) => 4096,
            _ => 32768,
        };
        f.bytes[bank as usize * 32768..bank as usize * 32768 + n].fill(255);
        if fault.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
}
fn load(journal: &mut Checkpoint<BodyMedia>, store: &Store) -> ArchiveCustody {
    let body = &store.evidence.as_ref().unwrap().body_id;
    let root = ed25519_dalek::SigningKey::from_bytes(&[7; 32]).verifying_key();
    let mut seed = PrivateSeed([0; 32]);
    let (mut resident, source) = journal
        .load_streamed_with_snapshot(&store.binding, &mut seed.0, |reader| {
            payload::decode_resident_boxed_with_prefix(
                |out| reader.read(out).map_err(|_| ()),
                body,
                root.as_bytes(),
                None,
            )
            .map_err(|_| Refusal::Corrupt)
        })
        .unwrap()
        .unwrap();
    assert_eq!(seed.0, [7; 32]);
    assert_eq!(
        resident.retained.evidence,
        *store.evidence.as_ref().unwrap()
    );
    assert_eq!(resident.retained.admissions, store.admissions);
    payload::authenticate_receipts(&mut resident, &source).unwrap();
    ArchiveCustody {
        proof: resident.proof,
        source,
        layouts: resident.layouts,
        network: resident.network,
        summary: resident.summary,
        span: resident.span,
        prefix: resident.prefix,
        local_count: resident.local_count,
    }
}
fn prepare<'a>(
    previous: &'a ArchiveCustody,
    store: &'a Store,
    next: &'a payload::network::Cursor,
) -> payload::rewrite::PreparedRewrite<'a> {
    let root = ed25519_dalek::SigningKey::from_bytes(&[7; 32]).verifying_key();
    payload::rewrite::PreparedRewrite::prepare_clean_network(
        previous.proof.as_ref().unwrap(),
        &previous.source,
        store.evidence.as_ref().unwrap(),
        store.admissions.as_ref().unwrap(),
        &previous.summary,
        previous.span,
        previous.prefix.as_ref(),
        previous.local_count,
        root.as_bytes(),
        &previous.layouts,
        previous.network.as_ref().unwrap(),
        next,
    )
    .unwrap()
}
#[test]
fn every_clean_checkpoint_cut_keeps_blank_owner_and_reopens_pending_or_clean() {
    let mut store = cold();
    let media = Media::new();
    let blank = blank(&store, media.clone());
    let encoded = store
        .journal
        .read_snapshot(&store.custody.as_ref().unwrap().source, |r| {
            let mut bytes = Vec::new();
            let mut work = [0; 128];
            loop {
                let n = r.read(&mut work)?;
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&work[..n]);
            }
            Ok(bytes)
        })
        .unwrap();
    let body = BodyMedia(Rc::new(RefCell::new(BodyFlash {
        bytes: vec![255; 65536],
        operations: 0,
        cut: None,
    })));
    let mut journal = Checkpoint::open(body.clone()).unwrap();
    journal.save(&store.binding, &[7; 32], &encoded).unwrap();
    let original = body.0.borrow().bytes.clone();
    let previous = load(&mut journal, &store);
    let next = clean_cursor(
        previous.network.as_ref().unwrap(),
        &ed25519_dalek::SigningKey::from_bytes(&[7; 32]),
    )
    .unwrap();
    let prepared = prepare(&previous, &store, &next);
    body.0.borrow_mut().operations = 0;
    let checked = super::super::super::network_transition::write_prepared(
        &mut journal,
        &store.binding,
        &[7; 32],
        &previous,
        &prepared,
    )
    .unwrap();
    assert_eq!(checked.source.generation(), 2);
    let operations = body.0.borrow().operations;
    for mode in 0..3 {
        for cut in 1..=operations {
            let body = BodyMedia(Rc::new(RefCell::new(BodyFlash {
                bytes: original.clone(),
                operations: 0,
                cut: None,
            })));
            let mut journal = Checkpoint::open(body.clone()).unwrap();
            let previous = load(&mut journal, &store);
            let prepared = prepare(&previous, &store, &next);
            {
                let mut f = body.0.borrow_mut();
                f.operations = 0;
                f.cut = Some((cut, mode));
            }
            assert!(
                super::super::super::network_transition::write_prepared(
                    &mut journal,
                    &store.binding,
                    &[7; 32],
                    &previous,
                    &prepared,
                )
                .is_err(),
                "Clean cut {cut}/{mode}"
            );
            assert_eq!(&body.0.borrow().bytes[..32768], &original[..32768]);
            assert!(media.data.borrow().bytes[4096..36864]
                .iter()
                .all(|b| *b == 255));
            media.unchanged_neighbors();
            body.0.borrow_mut().cut = None;
            let mut reopened = Checkpoint::open(body).unwrap();
            let surviving = load(&mut reopened, &store);
            match reopened.generation().unwrap() {
                1 => assert_eq!(
                    surviving.network.as_ref().unwrap().bytes(),
                    previous.network.as_ref().unwrap().bytes()
                ),
                2 => assert_eq!(surviving.network.as_ref().unwrap().bytes(), next.bytes()),
                _ => panic!("unexpected generation"),
            }
        }
    }
    // The actual original Store still owns Pending; only Blank can advance it.
    clean_store(&mut store, &blank).unwrap();
    println!(
        "Clean checkpoint:{} operation cuts with region retained",
        operations * 3
    );
}
