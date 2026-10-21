//! Morse admission uses the existing numeric-plan codec as a bounded carrier.
//! Its BackKind fields are fixed sentinels, never used to construct text drivers.
use super::{Malformed, codec::*};
use crate::{
    admission::{Admission as Numeric, BackKind},
    morse::{self, Admission},
};
use conduit_kernel::NodeId;
pub fn encode(a: &Admission, bytes: &mut [u8]) -> Result<usize, Malformed> {
    if a.literal_bytes as usize > morse::MAX_INPUT_BYTES {
        return Err(Malformed);
    }
    let mut w = Writer { bytes, at: 0 };
    w.raw(b"CMRS")?;
    for node in a.roles {
        w.u16(node.0)?;
    }
    w.u16(a.literal_bytes)?;
    w.raw(&a.literal[..a.literal_bytes as usize])?;
    w.u32(a.sign_bytes)?;
    let mut numeric = Numeric::new(morse::NODES as u16, morse::CORDS as u16);
    numeric.nodes[..morse::NODES].copy_from_slice(&a.nodes);
    numeric.cords[..morse::CORDS].copy_from_slice(&a.cords);
    numeric.ranges[..morse::ROUTES].copy_from_slice(&a.ranges);
    numeric.targets[..morse::CORDS].copy_from_slice(&a.targets);
    numeric.bindings[..morse::NODES].copy_from_slice(&a.bindings);
    let at = w.at;
    Ok(at + super::admission::encode(&numeric, &mut w.bytes[at..])?)
}
pub fn decode(bytes: &[u8]) -> Result<Admission, Malformed> {
    let mut r = Reader { bytes, at: 0 };
    if r.raw(4)? != b"CMRS" {
        return Err(Malformed);
    }
    let mut roles = [NodeId(0); morse::NODES];
    for node in &mut roles {
        *node = NodeId(r.u16()?);
    }
    let mut a = Admission::new(roles);
    a.literal_bytes = r.u16()?;
    if a.literal_bytes as usize > morse::MAX_INPUT_BYTES {
        return Err(Malformed);
    }
    a.literal[..a.literal_bytes as usize].copy_from_slice(r.raw(a.literal_bytes as usize)?);
    a.sign_bytes = r.u32()?;
    let numeric = super::admission::decode(&r.bytes[r.at..])?;
    if numeric.active_nodes as usize != morse::NODES
        || numeric.active_cords as usize != morse::CORDS
        || numeric.backs.iter().any(|b| *b != BackKind::Upper)
        || numeric.ranges[morse::ROUTES..].iter().any(Option::is_some)
        || numeric.targets[morse::CORDS..].iter().any(Option::is_some)
        || numeric.bindings[morse::NODES..].iter().any(Option::is_some)
    {
        return Err(Malformed);
    }
    a.nodes.copy_from_slice(&numeric.nodes[..morse::NODES]);
    a.cords.copy_from_slice(&numeric.cords[..morse::CORDS]);
    a.ranges.copy_from_slice(&numeric.ranges[..morse::ROUTES]);
    a.targets.copy_from_slice(&numeric.targets[..morse::CORDS]);
    a.bindings
        .copy_from_slice(&numeric.bindings[..morse::NODES]);
    Ok(a)
}
