//! Bounded JSON composition using the Host's already admitted serializer.
//! No schema fields are duplicated: both parts use the existing Serialize impls.
use crate::{mask_show_json::MaskShowPrefix, MaskJsonRefusal, Show};
use serde::{Serialize, Serializer};
enum Part<'a> {
    Prefix(MaskShowPrefix<'a>),
    Show(&'a Show),
}
/// Serialize-only view, constructed by the prepared occurrence. A Host can use
/// its existing no-allocation JSON serializer without adding another codec here.
pub struct MaskJsonPart<'a>(Part<'a>);
impl Serialize for MaskJsonPart<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Part::Prefix(value) => value.serialize(serializer),
            Part::Show(value) => value.serialize(serializer),
        }
    }
}
pub(crate) fn encode(
    prefix: MaskShowPrefix<'_>,
    plan: &[u8],
    show: &Show,
    output: &mut [u8],
    mut serialize: impl FnMut(MaskJsonPart<'_>, &mut [u8]) -> Result<usize, MaskJsonRefusal>,
) -> Result<usize, MaskJsonRefusal> {
    let length = serialize(MaskJsonPart(Part::Prefix(prefix)), output)?;
    if length == 0 || length > output.len() || output[length - 1] != b'}' {
        return Err(MaskJsonRefusal::Json);
    }
    let mut at = length - 1;
    for bytes in [&b",\"planned_mask\":"[..], plan, &b",\"show\":"[..]] {
        append(output, &mut at, bytes)?;
    }
    let tail = output.get_mut(at..).ok_or(MaskJsonRefusal::Capacity)?;
    let length = serialize(MaskJsonPart(Part::Show(show)), tail)?;
    if length == 0 || length > tail.len() || tail[length - 1] != b'}' {
        return Err(MaskJsonRefusal::Json);
    }
    at = at.checked_add(length).ok_or(MaskJsonRefusal::Capacity)?;
    append(output, &mut at, b"}")?;
    Ok(at)
}
fn append(output: &mut [u8], at: &mut usize, bytes: &[u8]) -> Result<(), MaskJsonRefusal> {
    let end = at
        .checked_add(bytes.len())
        .ok_or(MaskJsonRefusal::Capacity)?;
    output
        .get_mut(*at..end)
        .ok_or(MaskJsonRefusal::Capacity)?
        .copy_from_slice(bytes);
    *at = end;
    Ok(())
}
