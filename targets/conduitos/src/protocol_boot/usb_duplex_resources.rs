//! Root-selected bounded duplex preparation envelopes, before advertisement.
//! The exclusive Boot arena backs heap retention; separately rooted static DMA
//! backs each endpoint. Native admission checks actual retention against these
//! fixed envelopes before Play. Kernel/Body allocation remains separately charged.
const ENDPOINT_BYTES: u64 = 84 * 1024;
const ROOT_DMA_BYTES: u64 = 8192;

pub(super) fn pending_heap_bytes() -> usize {
    (2 * (ENDPOINT_BYTES - ROOT_DMA_BYTES)) as usize
}

pub(super) fn admit(
    arena_capacity: usize,
    arena_live: usize,
    dma_bytes: [u64; 2],
) -> Result<[u64; 2], &'static str> {
    // A changed Root DMA layout requires a reviewed native retention charge.
    if dma_bytes != [ROOT_DMA_BYTES; 2] {
        return Err("protocol-usb-duplex-dma-envelope-refused");
    }
    let heap_budget = 2 * (ENDPOINT_BYTES - ROOT_DMA_BYTES);
    let available = arena_capacity
        .checked_sub(arena_live)
        .ok_or("protocol-usb-duplex-arena-envelope-refused")?;
    if (available as u64) < heap_budget {
        return Err("protocol-usb-duplex-arena-envelope-refused");
    }
    Ok([ENDPOINT_BYTES; 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_root_backing_is_required_before_both_envelopes_are_admitted() {
        let needed = (2 * (ENDPOINT_BYTES - ROOT_DMA_BYTES)) as usize;
        assert_eq!(admit(needed + 32, 32, [8192; 2]), Ok([86016; 2]));
        assert!(admit(needed + 31, 32, [8192; 2]).is_err());
        assert!(admit(0, 0, [8192; 2]).is_err());
        assert!(admit(32, 33, [8192; 2]).is_err());
        assert!(admit(usize::MAX, 0, [8192, 4096]).is_err());
    }
}
