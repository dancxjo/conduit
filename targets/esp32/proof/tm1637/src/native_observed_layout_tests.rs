//! Layout provenance tests. Existing typed custody vertical separately proves
//! canonical admitted ACK acceptance; the fixed acceptance here is a media fixture.
use super::super::{Custody, ACCEPTANCE, SECTOR, WORK};
use super::*;
use conduit_environment_runtime::display_evidence_journal as journal;
use ed25519_dalek::{Signer, SigningKey};
#[derive(Clone, Debug)]
struct Media {
    bytes: Vec<u8>,
    identity: [u8; 32],
    offset: usize,
    cut: Option<(usize, usize)>,
    writes: usize,
    erases: usize,
}
impl Media {
    fn new() -> Self {
        Self {
            bytes: vec![255; TOTAL],
            identity: [7; 32],
            offset: 0,
            cut: None,
            writes: 0,
            erases: 0,
        }
    }
}
impl WholeEpoch for Media {
    fn identity(&self) -> [u8; 32] {
        self.identity
    }
    fn data_offset(&self) -> usize {
        self.offset
    }
    fn read(&mut self, at: usize, out: &mut [u8]) -> Result<(), Error> {
        out.copy_from_slice(self.bytes.get(at..at + out.len()).ok_or(Error::Region)?);
        Ok(())
    }
    fn program_marker(&mut self, bank: u8, at: usize, bytes: &[u8]) -> Result<(), Error> {
        if bank > 1 || at + bytes.len() > SECTOR {
            return Err(Error::Region);
        }
        let cut = self.cut.filter(|(n, _)| *n == self.writes).map(|(_, n)| n);
        self.writes += 1;
        let at = DATA + usize::from(bank) * SECTOR + at;
        for (old, new) in self.bytes[at..at + cut.unwrap_or(bytes.len())]
            .iter_mut()
            .zip(bytes)
        {
            assert_eq!(*old & new, *new);
            *old &= *new;
        }
        if cut.is_some() {
            Err(Error::Media)
        } else {
            Ok(())
        }
    }
    fn erase_marker(&mut self, bank: u8) -> Result<(), Error> {
        let at = DATA + usize::from(bank) * SECTOR;
        if bank > 1 {
            return Err(Error::Region);
        }
        self.bytes[at..at + SECTOR].fill(255);
        Ok(())
    }
    fn erase_data(&mut self, sector: u8) -> Result<(), Error> {
        let at = usize::from(sector) * SECTOR;
        if at + SECTOR > DATA {
            return Err(Error::Region);
        }
        self.erases += 1;
        self.bytes[at..at + SECTOR].fill(255);
        Ok(())
    }
}
impl Storage for Media {
    fn capacity(&self) -> u32 {
        DATA as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), journal::Refusal> {
        WholeEpoch::read(self, at as usize, out).map_err(|_| journal::Refusal::Media)
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), journal::Refusal> {
        let at = at as usize;
        if at + bytes.len() > DATA {
            return Err(journal::Refusal::Capacity);
        }
        for (old, new) in self.bytes[at..at + bytes.len()].iter_mut().zip(bytes) {
            assert_eq!(*old & new, *new);
            *old &= *new;
        }
        Ok(())
    }
}
fn fixture() -> (Media, Receipt, BodyId, SigningKey) {
    let body: BodyId = serde_json::from_value(serde_json::json!("body/layout-fixture")).unwrap();
    let key = SigningKey::from_bytes(&[8; 32]);
    let mut work = [0; WORK];
    let mut j = Journal::create(Media::new(), &[1; 96], b"layout-fixture", 960, &mut work).unwrap();
    j.append(b"retained canonical-journal payload", &mut work)
        .unwrap();
    let (media, draft) = preflight(j, &body, &key.verifying_key().to_bytes())
        .map_err(|(_, e)| e)
        .unwrap();
    assert_eq!(draft.root(), &key.verifying_key().to_bytes());
    let signature = key.sign(&draft.transcript()).to_bytes();
    (media, draft.signed(signature), body, key)
}
fn accepted(media: &Media) -> [u8; ACCEPTANCE] {
    let mut a = [0; ACCEPTANCE];
    a[..32].copy_from_slice(&media.identity);
    a[32..].fill(9);
    a
}
#[test]
fn actual_journal_preflight_checks_tail_extent_and_pinned_receipt() {
    let (media, receipt, body, key) = fixture();
    receipt.verify(&body, &key.verifying_key(), &media).unwrap();
    let foreign = SigningKey::from_bytes(&[10; 32]);
    assert!(receipt
        .verify(&body, &foreign.verifying_key(), &media)
        .is_err());
    let other: BodyId = serde_json::from_value(serde_json::json!("body/foreign")).unwrap();
    assert!(receipt
        .verify(&other, &key.verifying_key(), &media)
        .is_err());
    receipt.validate(&body, &key.verifying_key()).unwrap();
    let mut other_media = media.clone();
    other_media.identity[0] ^= 1;
    assert!(receipt
        .verify(&body, &key.verifying_key(), &other_media)
        .is_err());
    for at in [8, 12, 16, 20, 24, 56, 88] {
        let mut bytes = *receipt.bytes();
        bytes[at] ^= 1;
        assert!(Receipt::decode(bytes)
            .verify(&body, &key.verifying_key(), &media)
            .is_err());
    }
    for kind in 0..2 {
        let mut raw = Media::new();
        if kind == 0 {
            raw.bytes[DATA + SECTOR] = 0;
        } else {
            raw.offset = DATA;
        }
        let mut work = [0; WORK];
        let j = Journal::create(raw, &[1; 96], b"layout-fixture", 960, &mut work).unwrap();
        assert!(matches!(
            preflight(j, &body, &key.verifying_key().to_bytes()),
            Err((_, Error::Occupied))
        ));
    }
}
#[test]
fn established_layout_never_grants_erasure_or_ignores_committed_corruption() {
    let (mut media, receipt, body, key) = fixture();
    media.bytes[DATA] = b'O';
    let before = media.bytes.clone();
    let mut work = [0; WORK];
    assert!(matches!(
        Custody::open(media.clone(), &mut work, |_| true),
        Err((_, Error::UnknownMetadata))
    ));
    let witness = receipt.verify(&body, &key.verifying_key(), &media).unwrap();
    let controller = Custody::open_established(media, witness, &mut work, |_| true)
        .map_err(|(_, e)| e)
        .unwrap();
    let result = controller.begin(
        &accepted(&Media::new()),
        &mut work,
        |_| false,
        |_, _| Ok(()),
    );
    let (controller, error) = result.err().unwrap();
    assert_eq!(error, Error::Authentication);
    let after = controller.into_storage();
    assert_eq!(after.bytes, before);
    assert_eq!(after.erases, 0);
    for (offset, value) in [
        (DATA + SECTOR, 0),
        (DATA + 448, 0),
        (DATA + 8, 3),
        (DATA + 16, 2),
    ] {
        let mut later = after.clone();
        later.bytes[offset] = value;
        let witness = receipt.verify(&body, &key.verifying_key(), &later).unwrap();
        assert!(matches!(
            Custody::open_established(later, witness, &mut work, |_| true),
            Err((_, Error::UnknownMetadata))
        ));
    }
    let mut corrupt = after;
    corrupt.bytes[DATA + 444..DATA + 448].copy_from_slice(b"done");
    let witness = receipt
        .verify(&body, &key.verifying_key(), &corrupt)
        .unwrap();
    assert!(matches!(
        Custody::open_established(corrupt, witness, &mut work, |_| true),
        Err((_, Error::Corrupt))
    ));
}
#[test]
fn every_first_marker_cut_recovers_only_with_prior_witness_and_fresh_closed_custody() {
    let (original, receipt, body, key) = fixture();
    let acceptance = accepted(&original);
    let verify = |bytes: &[u8; ACCEPTANCE]| bytes == &acceptance;
    let mut cases = Vec::new();
    cases.extend((0..444).map(|n| (0, n)));
    cases.extend((0..=4).map(|n| (1, n)));
    for cut in cases {
        let mut media = original.clone();
        media.cut = Some(cut);
        let witness = receipt.verify(&body, &key.verifying_key(), &media).unwrap();
        let mut work = [0; WORK];
        let controller = Custody::open_established(media, witness, &mut work, verify)
            .map_err(|(_, e)| e)
            .unwrap();
        let failed = controller
            .begin(&acceptance, &mut work, verify, |storage, _| {
                if storage.bytes[..DATA] == original.bytes[..DATA] {
                    Ok(())
                } else {
                    Err(Error::NotClosed)
                }
            })
            .err()
            .expect("injected first marker fault")
            .0;
        let mut media = failed.into_storage();
        assert_eq!(media.erases, 0);
        assert_eq!(media.bytes[..DATA], original.bytes[..DATA]);
        media.cut = None;
        let witness = receipt.verify(&body, &key.verifying_key(), &media).unwrap();
        let controller = Custody::open_established(media, witness, &mut work, verify)
            .map_err(|(_, e)| e)
            .unwrap();
        let erased = match controller.resume(&mut work) {
            Ok(erased) => erased,
            Err((controller, Error::Phase)) => {
                // A witness supplies no closed/ACK proof. Failed fresh closure still preserves data.
                let (controller, error) = controller
                    .begin(&acceptance, &mut work, verify, |_, _| Err(Error::NotClosed))
                    .err()
                    .unwrap();
                assert_eq!(error, Error::NotClosed);
                controller
                    .begin(&acceptance, &mut work, verify, |storage, _| {
                        if storage.bytes[..DATA] == original.bytes[..DATA] {
                            Ok(())
                        } else {
                            Err(Error::NotClosed)
                        }
                    })
                    .map_err(|(_, e)| e)
                    .unwrap()
            }
            Err((_, e)) => panic!("unexpected recovery refusal: {e:?}"),
        };
        assert!(erased.into_storage().bytes[..DATA]
            .iter()
            .all(|b| *b == 255));
    }
}
