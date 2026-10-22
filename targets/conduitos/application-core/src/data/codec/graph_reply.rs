//! Immutable Graph reply serialization, validation, then finite packing turns.
use super::*;
use patchbay_graph::PatchbayGraph;

/// Each packing entry consumes at most this many decoded bytes. The total
/// number of turns is derived from the unchanged admitted decoded ceiling.
pub const PACK_BYTES: usize = 4096;
pub(crate) struct Packing {
    length: usize,
    consumed: usize,
    written: usize,
}
impl Workspace {
    pub(crate) fn serialize_graph(
        &mut self,
        graph: Option<&PatchbayGraph>,
    ) -> Result<usize, Refusal> {
        // The variant and field names preserve Reply::Graph framing without
        // cloning the retained graph or maintaining a second model.
        #[derive(Serialize)]
        enum BorrowedReply<'a> {
            Graph { graph: Option<&'a PatchbayGraph> },
        }
        transport_codec::encode_unvalidated(&BorrowedReply::Graph { graph }, &mut self.json)
    }
    pub(crate) fn validate_graph(&self, length: usize) -> Result<Packing, Refusal> {
        transport_codec::preflight(self.json.get(..length).ok_or(Refusal::Bounds)?)?;
        Ok(Packing {
            length,
            consumed: 0,
            written: HEADER,
        })
    }
    /// Returns true only when the complete existing reply encoding is ready.
    pub(crate) fn pack_graph(
        &mut self,
        state: &mut Packing,
        offset: usize,
        output: &mut Vec<u8>,
    ) -> Result<bool, Refusal> {
        if offset != state.consumed
            || state.consumed >= state.length
            || output.capacity() < MAX_BLOB
        {
            return Err(Refusal::Order);
        }
        let end = (state.consumed + PACK_BYTES).min(state.length);
        let last = end == state.length;
        if state.consumed == 0 {
            output.resize(MAX_BLOB, 0);
            if state.length > MAX_BLOB {
                self.compressor.reset();
                output[..MAGIC.len()].copy_from_slice(MAGIC);
                output[MAGIC.len()..HEADER].copy_from_slice(&(state.length as u32).to_le_bytes());
            }
        }
        if state.length <= MAX_BLOB {
            output[state.consumed..end].copy_from_slice(&self.json[state.consumed..end]);
            state.consumed = end;
            if last {
                output.truncate(state.length);
            }
            return Ok(last);
        }
        let (status, consumed, written) = compress(
            &mut self.compressor,
            &self.json[state.consumed..end],
            &mut output[state.written..],
            if last {
                TDEFLFlush::Finish
            } else {
                TDEFLFlush::None
            },
        );
        if consumed != end - state.consumed
            || (last && status != TDEFLStatus::Done)
            || (!last && status != TDEFLStatus::Okay)
        {
            return Err(Refusal::Capacity);
        }
        state.consumed = end;
        state.written += written;
        if last {
            output.truncate(state.written);
        }
        Ok(last)
    }
}
