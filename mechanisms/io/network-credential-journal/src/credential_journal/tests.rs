use super::*;
struct Flash {
    bytes: [u8; CAPACITY as usize],
    budget: usize,
}
impl Flash {
    fn erased() -> Self {
        Self {
            bytes: [255; CAPACITY as usize],
            budget: usize::MAX,
        }
    }
}
impl Storage for Flash {
    fn capacity(&self) -> u32 {
        CAPACITY
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        out.copy_from_slice(&self.bytes[at as usize..at as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        // Enforce the central format-transition fence independently of
        // Journal implementation: never overwrite v2 payload at old192
        // while its actual commit remains published.
        let base = (at / SECTOR * SECTOR) as usize;
        if at as usize == base + LEGACY_BODY {
            assert!(
                &self.bytes[base..base + 8] != MAGIC
                    || &self.bytes[base + BODY..base + RECORD] != COMMIT
            );
        }
        for (i, b) in bytes.iter().enumerate() {
            if self.budget == 0 {
                return Err(Refusal::Media);
            }
            self.budget -= 1;
            self.bytes[at as usize + i] &= *b;
        }
        Ok(())
    }
    fn erase_sector(&mut self, at: u32) -> Result<(), Refusal> {
        for i in at..at + SECTOR {
            if self.budget == 0 {
                return Err(Refusal::Media);
            }
            self.budget -= 1;
            self.bytes[i as usize] = 255;
        }
        Ok(())
    }
}
fn open(storage: Flash) -> Journal<Flash> {
    Journal::open(storage).unwrap_or_else(|_| panic!("journal open"))
}
#[test]
fn every_interrupted_successor_recovers_one_complete_committed_revision() {
    let mut base = open(Flash::erased());
    base.commit(b"old-private-input").unwrap();
    for round in 0..2 {
        for budget in 0..=SECTOR as usize + BODY + 12 {
            let mut storage = Flash {
                bytes: base.storage.bytes,
                budget,
            };
            let mut journal = open(storage);
            let _ = journal.commit(b"new-private-input");
            storage = journal.storage;
            storage.budget = usize::MAX;
            let mut recovered = open(storage);
            let mut out = [0; PAYLOAD];
            let (revision, length) = recovered.read(&mut out).unwrap();
            assert!(revision == base.revision || revision == base.revision + 1);
            if revision == base.revision {
                let mut expected = [0; PAYLOAD];
                let (_, n) = base.read(&mut expected).unwrap();
                assert_eq!(&out[..length], &expected[..n]);
            } else {
                assert_eq!(&out[..length], b"new-private-input");
            }
        }
        if round == 0 {
            base.commit(b"second-private-input").unwrap();
        }
    }
}
#[test]
fn blank_corrupt_duplicate_and_undersized_reads_fail_closed() {
    let mut journal = open(Flash::erased());
    assert_eq!(journal.read(&mut [0; PAYLOAD]), Err(Refusal::Empty));
    journal.commit(b"private").unwrap();
    assert_eq!(journal.read(&mut [0; 1]), Err(Refusal::Capacity));
    let mut corrupt = Flash {
        bytes: journal.storage.bytes,
        budget: usize::MAX,
    };
    corrupt.bytes[20] ^= 1;
    assert!(matches!(Journal::open(corrupt), Err((_, Refusal::Corrupt))));
    let first = journal.storage.bytes[..RECORD].to_vec();
    journal.storage.bytes[SECTOR as usize..SECTOR as usize + RECORD].copy_from_slice(&first);
    assert!(matches!(
        Journal::open(journal.storage),
        Err((_, Refusal::Corrupt))
    ));
}
fn seed(storage: &mut Flash, slot: usize, format: Format, revision: u64, wire: &[u8]) {
    assert!(wire.len() <= format.payload());
    let body = format.body();
    let mut record = Secret([255; RECORD]);
    record.0[..8].copy_from_slice(match format {
        Format::Legacy => LEGACY_MAGIC,
        Format::Current => MAGIC,
    });
    record.0[8..16].copy_from_slice(&revision.to_le_bytes());
    record.0[16..18].copy_from_slice(&(wire.len() as u16).to_le_bytes());
    record.0[18..20].fill(0);
    record.0[20..20 + wire.len()].copy_from_slice(wire);
    let checksum = crc(&record.0[..body - 4]);
    record.0[body - 4..body].copy_from_slice(&checksum.to_le_bytes());
    record.0[body..body + 4].copy_from_slice(COMMIT);
    storage.bytes[slot * SECTOR as usize..slot * SECTOR as usize + body + 4]
        .copy_from_slice(&record.0[..body + 4]);
}
#[test]
fn every_byte_cut_legacy_to_v2_and_v2_rotations_preserves_active_and_exact_payload() {
    let mut successor = [0x61; PAYLOAD];
    successor[172..176].copy_from_slice(COMMIT);
    for format in [Format::Legacy, Format::Current] {
        for active in 0..2 {
            let mut base = Flash::erased();
            let mut older = [0x62; PAYLOAD];
            older[172..176].copy_from_slice(COMMIT);
            let older_length = format.payload();
            seed(&mut base, 1 - active, format, 7, &older[..older_length]);
            seed(
                &mut base,
                active,
                format,
                8,
                b"complete-active-private-record",
            );
            let active_start = active * SECTOR as usize;
            for budget in 0..=SECTOR as usize + BODY + 12 {
                let mut journal = open(Flash {
                    bytes: base.bytes,
                    budget,
                });
                let result = journal.commit(&successor);
                assert_eq!(
                    &journal.storage.bytes[active_start..active_start + SECTOR as usize],
                    &base.bytes[active_start..active_start + SECTOR as usize]
                );
                let storage = Flash {
                    bytes: journal.storage.bytes,
                    budget: usize::MAX,
                };
                let mut recovered = open(storage);
                let mut output = Secret([0; RECORD]);
                let (revision, length) = recovered.read(&mut output.0).unwrap();
                assert!(revision == 8 || revision == 9);
                if revision == 8 {
                    assert_eq!(&output.0[..length], b"complete-active-private-record");
                } else {
                    assert_eq!(length, PAYLOAD);
                    assert_eq!(&output.0[..length], &successor);
                }
                if result.is_ok() {
                    assert_eq!(revision, 9);
                }
            }
            // The final fully funded transition must actually publish v2.
            let mut journal = open(base);
            assert_eq!(journal.commit(&successor), Ok(9));
            assert_eq!(
                &journal.storage.bytes
                    [(1 - active) * SECTOR as usize..(1 - active) * SECTOR as usize + 8],
                MAGIC
            );
        }
    }
}
#[test]
fn each_format_strict_crc_padding_lengths_unknown_headers_duplicate_and_exhaustion() {
    for format in [Format::Legacy, Format::Current] {
        let mut base = Flash::erased();
        seed(&mut base, 0, format, 1, b"private");
        let body = format.body();
        for at in [0, 8, 16, 18, 20, 27, body - 4] {
            let mut corrupt = Flash {
                bytes: base.bytes,
                budget: usize::MAX,
            };
            corrupt.bytes[at] ^= 1;
            assert!(
                matches!(Journal::open(corrupt), Err((_, Refusal::Corrupt))),
                "offset {at}"
            );
        }
        // Corruption is still refused when its CRC was recomputed: reserved
        // bytes, padding and extent constraints are independent of CRC.
        for at in [18, 27] {
            let mut corrupt = Flash {
                bytes: base.bytes,
                budget: usize::MAX,
            };
            corrupt.bytes[at] ^= 1;
            let checksum = crc(&corrupt.bytes[..body - 4]);
            corrupt.bytes[body - 4..body].copy_from_slice(&checksum.to_le_bytes());
            assert!(matches!(Journal::open(corrupt), Err((_, Refusal::Corrupt))));
        }
        let mut duplicate = Flash {
            bytes: base.bytes,
            budget: usize::MAX,
        };
        seed(&mut duplicate, 1, format, 1, b"second");
        assert!(matches!(
            Journal::open(duplicate),
            Err((_, Refusal::Corrupt))
        ));
        let mut exhausted = Flash::erased();
        seed(&mut exhausted, 0, format, u64::MAX, b"private");
        let mut journal = open(exhausted);
        let before = journal.storage.bytes;
        assert_eq!(journal.commit(b"new"), Err(Refusal::Exhausted));
        assert_eq!(journal.storage.bytes, before);
        // A torn unknown header with no surviving commit never publishes.
        base.bytes[..8].fill(0);
        base.bytes[body..body + 4].fill(0);
        assert_eq!(open(base).revision(), 0);
    }
    let mut wrong_version = Flash::erased();
    seed(&mut wrong_version, 0, Format::Current, 1, b"private");
    wrong_version.bytes[..8].copy_from_slice(LEGACY_MAGIC);
    assert!(matches!(
        Journal::open(wrong_version),
        Err((_, Refusal::Corrupt))
    ));
    let mut journal = open(Flash::erased());
    assert_eq!(journal.commit(&[1; PAYLOAD + 1]), Err(Refusal::Capacity));
    assert_eq!(journal.commit(&[]), Err(Refusal::Capacity));
    assert_eq!(journal.commit(&[1; PAYLOAD]), Ok(1));
    let mut too_small = [0xa5; PAYLOAD - 1];
    assert_eq!(journal.read(&mut too_small), Err(Refusal::Capacity));
    assert_eq!(too_small, [0xa5; PAYLOAD - 1]);
}
