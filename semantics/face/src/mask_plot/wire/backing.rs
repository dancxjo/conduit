//! Closed workspace for the pinned miniz0.8.9 human-readable Mask codec.
use super::*;
use core::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedMaskJsonWorkspace {
    /// Raw ordinary Plan JSON, counted without calling the Mask codec.
    pub raw_plan_bytes: usize,
    /// Codec-owned peak only. Caller retains input, outer output and any
    /// subsequent Plan/route/wardrobe clones under their separate charges.
    pub codec_bytes: usize,
}
impl PlannedMaskPlot {
    /// Allocation-free raw-Plan preflight for the hosted codec, unavailable on
    /// unqualified no_std paths. Does not validate a Plan or reserve capacity.
    pub fn json_workspace(&self) -> Option<PlannedMaskJsonWorkspace> {
        let raw = crate::bounded_json::count(&self.plan)?;
        if raw == 0 || raw > MAX_EXPANDED_BYTES {
            return None;
        }
        // Pinned miniz_oxide0.8.9 deflate/core.rs and buffer.rs:
        // HuffmanOxide's three arrays; LocalBuf; HashBuffers' dictionary,
        // next and hash boxed slices. All are fixed arrays, with no hidden Vec.
        // CompressorOxide is included conservatively even though its inline
        // state is caller stack, not a separately allocated heap object.
        let huffman = size_of::<[[u16; 288]; 3]>()
            .checked_add(size_of::<[[u16; 288]; 3]>())?
            .checked_add(size_of::<[[u8; 288]; 3]>())?;
        let local = (64usize * 1024).checked_mul(13)?.checked_div(10)?;
        let dictionary = 32768usize.checked_add(258)?;
        let chains = size_of::<[u16; 32768]>().checked_add(size_of::<[u16; 32768]>())?;
        let mut bytes = raw
            .checked_add(huffman)?
            .checked_add(local)?
            .checked_add(dictionary)?
            .checked_add(chains)?
            .checked_add(size_of::<CompressorOxide>())?
            .checked_add(MAX_COMPRESSED_BYTES)?
            .checked_add(MAX_COMPRESSED_BYTES.checked_mul(2)?)?;
        // JsonMask owns one MaskPlot clone, encoding text and Plan identity.
        let mask = &self.mask;
        for text in [
            &mask.plot_identity.source_document_id.0,
            &mask.plot_identity.checked_plot_id.0,
            &mask.plot_identity.expanded_plot_id.0,
            &mask.plot_name,
        ] {
            bytes = bytes.checked_add(text.capacity())?;
        }
        for port in [
            &mask.face_input,
            &mask.interaction_output,
            &mask.show_output,
        ] {
            for text in [&port.front_port_id.0, &port.gear_id.0, &port.gear_port_id.0] {
                bytes = bytes.checked_add(text.capacity())?;
            }
        }
        bytes = bytes
            .checked_add(ENCODING.len())?
            .checked_add(self.plan.plan_id.0.capacity())?;
        Some(PlannedMaskJsonWorkspace {
            raw_plan_bytes: raw,
            codec_bytes: bytes,
        })
    }
}
