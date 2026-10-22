//! Bounded preparation frames; execution uses fixed request/reply arrays.
use super::{BackingLedger, NativeBackingGrant, NativeBackingRefusal, NativeCpuProfile};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
pub const MAX_FRAME: usize = 8192;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    pub profile: NativeCpuProfile,
    pub grant: NativeBackingGrant,
    pub graph: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Prepared {
    Ready(BackingLedger),
    Refused(NativeBackingRefusal),
}
pub fn send<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<(), NativeBackingRefusal> {
    let bytes = serde_json::to_vec(value).map_err(|_| NativeBackingRefusal::Protocol)?;
    if bytes.len() > MAX_FRAME {
        return Err(NativeBackingRefusal::Protocol);
    }
    writer
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|()| writer.write_all(&bytes))
        .and_then(|()| writer.flush())
        .map_err(|_| NativeBackingRefusal::Io)
}
pub fn receive<T: serde::de::DeserializeOwned>(
    reader: &mut impl Read,
) -> Result<T, NativeBackingRefusal> {
    let mut size = [0; 4];
    reader
        .read_exact(&mut size)
        .map_err(|_| NativeBackingRefusal::Io)?;
    let size = u32::from_le_bytes(size) as usize;
    if size > MAX_FRAME {
        return Err(NativeBackingRefusal::Protocol);
    }
    let mut bytes = vec![0; size];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| NativeBackingRefusal::Io)?;
    serde_json::from_slice(&bytes).map_err(|_| NativeBackingRefusal::Protocol)
}
