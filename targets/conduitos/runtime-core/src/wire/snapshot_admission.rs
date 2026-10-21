//! Bounded numeric snapshot Fore admission, without Face/Plan serialization.
use super::{Malformed, codec::*};
use crate::snapshot::Admission;
use conduit_kernel::{CordId, RemoteEndpointId};
pub fn encode(a: &Admission, bytes: &mut [u8]) -> Result<usize, Malformed> {
    let mut w = Writer { bytes, at: 0 };
    w.raw(b"CSNP")?;
    w.u16(a.input_endpoint.0)?;
    w.u16(a.input_cord.0)?;
    w.u16(a.output_endpoint.0)?;
    w.u16(a.output_cord.0)?;
    let at = w.at;
    Ok(at + super::admission::encode(&a.numeric, &mut w.bytes[at..])?)
}
pub fn decode(bytes: &[u8]) -> Result<Admission, Malformed> {
    let mut r = Reader { bytes, at: 0 };
    if r.raw(4)? != b"CSNP" {
        return Err(Malformed);
    }
    let input_endpoint = RemoteEndpointId(r.u16()?);
    let input_cord = CordId(r.u16()?);
    let output_endpoint = RemoteEndpointId(r.u16()?);
    let output_cord = CordId(r.u16()?);
    let numeric = super::admission::decode(&r.bytes[r.at..])?;
    Ok(Admission {
        numeric,
        input_endpoint,
        input_cord,
        output_endpoint,
        output_cord,
    })
}
