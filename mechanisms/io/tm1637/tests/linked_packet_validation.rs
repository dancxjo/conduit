//! Actual driver codec versus its previous owned-schema validation contract.
#![cfg(feature = "host-call")]
use conduit_core as c;
use conduit_tm1637_wire::{Packet, WriteContract, MAXIMUM_BYTES};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "../src/codec.rs"]
mod codec;
#[path = "support/packet.rs"]
mod packet;

#[test]
fn linked_schema_keeps_wire_and_refusal_parity_without_retained_heap() {
    let contract = WriteContract::prepare().unwrap();
    let (owned, old_memory) = allocation_probe::observe(|| {
        c::PreparedStructuredValueValidator::new(contract.packets_type(), MAXIMUM_BYTES as usize)
            .unwrap()
    });
    let (decoder, new_memory) =
        allocation_probe::observe(|| codec::PreparedPackets::prepare(&contract).unwrap());
    assert!(old_memory.retained_bytes() > 0);
    assert_eq!(new_memory.retained_bytes(), 0);
    let encoded = packet::encoded();
    let mut variants: Vec<Vec<u8>> = (0..encoded.len())
        .map(|length| encoded[..length].to_vec())
        .collect();
    let mut trailing = encoded.clone();
    trailing.push(0);
    variants.push(trailing);
    let mut foreign = encoded.clone();
    foreign[0] ^= 1;
    variants.push(foreign);
    // The final U8 leaf now has a well-framed but invalid two-byte payload.
    let mut bad_leaf = encoded.clone();
    let length = bad_leaf.len();
    assert_eq!(&bad_leaf[length - 6..length - 1], &[0, 1, 0, 0, 0]);
    bad_leaf[length - 5] = 2;
    bad_leaf.push(0);
    variants.push(bad_leaf);
    variants.push(vec![0; MAXIMUM_BYTES as usize + 1]);
    let expected: Vec<_> = variants
        .iter()
        .map(|bytes| owned.validate(bytes).unwrap_err())
        .collect();
    let (_, play) = allocation_probe::observe(|| {
        for _ in 0..4 {
            for (bytes, refusal) in variants.iter().zip(&expected) {
                assert_eq!(
                    decoder.decode(bytes).err(),
                    Some(codec::PacketRefusal::Canonical(refusal.clone()))
                );
            }
            let decoded = decoder.decode(&encoded).unwrap();
            let packets = decoded.packets();
            assert_eq!(packets[0].bytes, &[0x40]);
            assert_eq!(packets[1].bytes, &[0xc0, 0x7d, 0x87, 0x7d, 0x71, 0, 0]);
            assert_eq!(packets[2].bytes, &[0x8b]);
        }
    });
    assert_eq!((play.allocations, play.reallocations), (0, 0));
    eprintln!(
        "packet_validator_retained_before={} after={} inline_bytes={}",
        old_memory.retained_bytes(),
        new_memory.retained_bytes(),
        core::mem::size_of_val(&decoder)
    );
}
