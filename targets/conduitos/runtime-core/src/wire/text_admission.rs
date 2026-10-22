//! Text admission uses the existing numeric-plan codec as a bounded carrier.
//! Its BackKind fields are fixed sentinels, never used to construct text drivers.
use super::{Malformed, codec::*};
use crate::{
    admission::{Admission as Numeric, BackKind},
    text::{self, Admission},
};
use conduit_kernel::NodeId;
pub fn encode(a: &Admission, bytes: &mut [u8]) -> Result<usize, Malformed> {
    if a.literal_bytes as usize > text::MAX_VALUE_BYTES {
        return Err(Malformed);
    }
    let mut w = Writer { bytes, at: 0 };
    w.raw(b"CTXT")?;
    w.u16(a.literal_node.0)?;
    w.u16(a.upper_node.0)?;
    w.u16(a.face_node.0)?;
    w.u16(a.literal_bytes)?;
    w.raw(&a.literal[..a.literal_bytes as usize])?;
    w.u32(a.sign_bytes)?;
    let mut numeric = Numeric::new(text::NODES as u16, text::CORDS as u16);
    numeric.nodes[..text::NODES].copy_from_slice(&a.nodes);
    numeric.cords[..text::CORDS].copy_from_slice(&a.cords);
    numeric.ranges[..text::ROUTES].copy_from_slice(&a.ranges);
    numeric.targets[..text::CORDS].copy_from_slice(&a.targets);
    numeric.bindings[..text::NODES].copy_from_slice(&a.bindings);
    let at = w.at;
    Ok(at + super::admission::encode(&numeric, &mut w.bytes[at..])?)
}
pub fn decode(bytes: &[u8]) -> Result<Admission, Malformed> {
    let mut r = Reader { bytes, at: 0 };
    if r.raw(4)? != b"CTXT" {
        return Err(Malformed);
    }
    let mut a = Admission::new(NodeId(r.u16()?), NodeId(r.u16()?), NodeId(r.u16()?));
    a.literal_bytes = r.u16()?;
    if a.literal_bytes as usize > text::MAX_VALUE_BYTES {
        return Err(Malformed);
    }
    a.literal[..a.literal_bytes as usize].copy_from_slice(r.raw(a.literal_bytes as usize)?);
    a.sign_bytes = r.u32()?;
    let numeric = super::admission::decode(&r.bytes[r.at..])?;
    if numeric.active_nodes as usize != text::NODES
        || numeric.active_cords as usize != text::CORDS
        || numeric.backs.iter().any(|b| *b != BackKind::Upper)
        || numeric.ranges[text::ROUTES..].iter().any(Option::is_some)
        || numeric.targets[text::CORDS..].iter().any(Option::is_some)
        || numeric.bindings[text::NODES..].iter().any(Option::is_some)
    {
        return Err(Malformed);
    }
    a.nodes.copy_from_slice(&numeric.nodes[..text::NODES]);
    a.cords.copy_from_slice(&numeric.cords[..text::CORDS]);
    a.ranges.copy_from_slice(&numeric.ranges[..text::ROUTES]);
    a.targets.copy_from_slice(&numeric.targets[..text::CORDS]);
    a.bindings.copy_from_slice(&numeric.bindings[..text::NODES]);
    Ok(a)
}
