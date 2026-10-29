//! Private bounded serialization/encoding. The wire contract is the exact
//! decoded canonical Plan; no compressed bitstream is an authority or Plan seal.
#[path = "deflate.rs"]
mod deflate;
#[path = "huffman.rs"]
mod huffman;
#[path = "json.rs"]
mod json;
use alloc::string::String;
use serde::Serialize;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CodecError {
    Capacity,
    Invalid,
}
impl core::fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("Mask Plan stream encoding refused")
    }
}
impl core::error::Error for CodecError {}
impl serde::ser::Error for CodecError {
    fn custom<T: core::fmt::Display>(_: T) -> Self {
        Self::Invalid
    }
}
pub(crate) const WORKSPACE_BYTES: usize = deflate::WORKSPACE_BYTES;
const _: () = assert!(WORKSPACE_BYTES < 30 * 1024);

pub(super) trait JsonSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), CodecError>;
}
impl JsonSink for deflate::Encoder<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), CodecError> {
        deflate::Encoder::write(self, bytes)
    }
}

pub(super) fn encode(
    plan: &impl Serialize,
    maximum_expanded: usize,
    maximum_compressed: usize,
) -> Result<(String, usize), CodecError> {
    encode_with_workspace(plan, maximum_expanded, maximum_compressed, &mut [])
}
pub(super) fn encode_with_workspace(
    plan: &impl Serialize,
    maximum_expanded: usize,
    maximum_compressed: usize,
    workspace: &mut [&mut [u8]],
) -> Result<(String, usize), CodecError> {
    let mut first = deflate::Encoder::counting_with_workspace(maximum_expanded, workspace)?;
    plan.serialize(json::Json {
        encoder: &mut first,
        key: false,
    })?;
    let (histogram, expanded, digest) = first.finish_count()?;
    let tree = huffman::Tree::from_histogram(&histogram);
    drop(histogram);
    let mut encoder = deflate::Encoder::with_tree_and_workspace(
        maximum_expanded,
        maximum_compressed,
        tree,
        workspace,
    )?;
    plan.serialize(json::Json {
        encoder: &mut encoder,
        key: false,
    })?;
    let (encoded, second_expanded, second_digest) = encoder.finish()?;
    if expanded != second_expanded || digest != second_digest {
        return Err(CodecError::Invalid);
    }
    Ok((encoded, expanded))
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
