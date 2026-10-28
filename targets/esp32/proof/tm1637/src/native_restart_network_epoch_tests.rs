//! Actual network Journal eight-page rotation, with signed protocol fixtures.
//! Canonical member admission is proved separately; no physical device proof.
use super::*;
use conduit_network_join_call::recovery_evidence as wire;
impl wire::Storage for Media {
    fn capacity(&self) -> u32 {
        self.capacity
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), wire::Refusal> {
        if at
            .checked_add(out.len() as u32)
            .is_none_or(|end| end > CAPACITY)
        {
            return Err(wire::Refusal::Geometry);
        }
        out.copy_from_slice(
            &self.data.borrow().bytes[4096 + at as usize..4096 + at as usize + out.len()],
        );
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), wire::Refusal> {
        if at
            .checked_add(bytes.len() as u32)
            .is_none_or(|end| end > CAPACITY)
        {
            return Err(wire::Refusal::Geometry);
        }
        for (old, new) in self.data.borrow_mut().bytes
            [4096 + at as usize..4096 + at as usize + bytes.len()]
            .iter_mut()
            .zip(bytes)
        {
            assert_eq!(*old & new, *new);
            *old &= new;
        }
        Ok(())
    }
}
fn encode(sequence: u64, work: &mut [u8; wire::MAXIMUM_BYTES]) {
    let signs = conduit_kernel::FixedSignLog::<1>::new(core::mem::size_of::<
        conduit_kernel::KernelEvent,
    >() as u32)
    .unwrap();
    wire::encode(
        wire::Scope {
            host: "host/root",
            boot: "boot/network-epoch",
            plan: "plan/network",
            active_play: "play/network",
            provider_generation: sequence,
            play_sequence: sequence,
            configuration_revision: 1,
        },
        signs.events(),
        &signs,
        work,
    )
    .unwrap();
}
fn hash(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(domain);
    for part in parts {
        h.update(part);
    }
    h.finalize().into()
}
#[test]
fn all_eight_closed_pages_and_tail_are_reclaimed_then_ninth_retirement_survives_clean_boot() {
    let media = Media::new();
    media.data.borrow_mut().bytes[4096..36864].fill(255);
    let mut work = [0; wire::MAXIMUM_BYTES];
    let mut journal = wire::Journal::open(media.clone(), &mut work).ok().unwrap();
    for seq in 1..=8 {
        encode(seq, &mut work);
        journal.append(&work).unwrap();
    }
    assert_eq!(journal.committed_count(), 8);
    assert_eq!(journal.remaining_slots(), 0);
    encode(9, &mut work);
    assert_eq!(journal.append(&work), Err(wire::Refusal::Capacity));
    drop(journal);
    // This tail lies outside decoded pages, but belongs to the whole snapshot.
    media.data.borrow_mut().bytes[36863] = 0x17;
    let old = media.data.borrow().bytes[4096..36864].to_vec();
    let (mut store, owner, _, _) = super::super::super::park_tests::fixture();
    store.evidence = Some(owner.unwrap().evidence().clone());
    store.ready = false;
    let key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let mut bytes =
        *payload::network::fixture(&store.evidence.as_ref().unwrap().body_id, &key, 1).bytes();
    bytes[132] = 8;
    bytes[136..168].copy_from_slice(&Sha256::digest(&old));
    let member = ed25519_dalek::SigningKey::from_bytes(&[91; 32]);
    let offer = hash(
        b"conduit.environment/network-retirement-offer@1\0",
        &[
            key.verifying_key().as_bytes(),
            &bytes[24..168],
            &bytes[360..392],
        ],
    );
    bytes[168..200].copy_from_slice(&offer);
    let ack = hash(
        b"conduit.environment/network-retirement-custody@1\0",
        &[&offer, &bytes[200..232]],
    );
    bytes[264..328].copy_from_slice(&member.sign(&ack).to_bytes());
    bytes[328..360].copy_from_slice(&ack);
    let message = hash(
        b"conduit.environment/network-retirement-receipt@1\0",
        &[&bytes[..408]],
    );
    bytes[408..472].copy_from_slice(&key.sign(&message).to_bytes());
    let pending = payload::network::Cursor::decode(bytes).unwrap();
    store
        .journal
        .save_streamed(&store.binding, &[7; 32], |sink| {
            payload::stream_compressed_with_admissions_and_network(
                store.evidence.as_ref().unwrap(),
                &store.archives,
                &[],
                store.admissions.as_ref(),
                Some(&pending),
                sink,
            )
            .map_err(stream_refusal)
        })
        .unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let (r, source) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &store.evidence.as_ref().unwrap().body_id,
        &mut seed.0,
        None,
    )
    .unwrap();
    store.custody = Some(ArchiveCustody {
        proof: r.proof,
        source,
        layouts: r.layouts,
        network: r.network,
        summary: r.summary,
        span: r.span,
        prefix: r.prefix,
        local_count: r.local_count,
    });
    store.archives.clear();
    let blank = blank(&store, media.clone());
    clean_store(&mut store, &blank).unwrap();
    let mut journal = wire::Journal::open(blank.storage, &mut work).ok().unwrap();
    assert_eq!(journal.committed_count(), 0);
    assert_eq!(journal.remaining_slots(), 8);
    encode(9, &mut work);
    journal.append(&work).unwrap();
    assert_eq!(journal.committed_count(), 1);
    assert_eq!(journal.remaining_slots(), 7);
    let writes = media.data.borrow().erases;
    let Opened::Ready(region) = begin_store(&store, journal.into_storage())
        .map_err(|(_, e)| e)
        .unwrap()
    else {
        panic!("Clean must not erase");
    };
    let mut journal = wire::Journal::open(region, &mut work).ok().unwrap();
    assert_eq!(journal.committed_count(), 1);
    journal.read_slot(0, &mut work).unwrap();
    assert_eq!(wire::Page::parse(&work).unwrap().scope().play_sequence, 9);
    assert_eq!(media.data.borrow().erases, writes);
    media.unchanged_neighbors();
}
