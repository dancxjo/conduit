//! Read-only full Face export requests. Claims here carry no admission authority.
pub use crate::observed_custody::{render, Reply};
use core::cell::RefCell;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Snapshot {
        nonce: [u8; 32],
    },
    Chunk {
        digest: [u8; 32],
        // Wire scalar backing keeps this queued request four-byte aligned;
        // a native u64 would raise alignment of every HTTP exchange future.
        revision_le: [u8; 8],
        bytes: u32,
        offset: u32,
    },
}
type Provider = fn(Request) -> Reply;
static PROVIDER: critical_section::Mutex<RefCell<Option<Provider>>> =
    critical_section::Mutex::new(RefCell::new(None));
pub fn install_provider(provider: Provider) -> Result<(), &'static str> {
    critical_section::with(|cs| {
        let mut slot = PROVIDER.borrow(cs).borrow_mut();
        if slot.is_some() {
            return Err("face-export-already-installed");
        }
        *slot = Some(provider);
        Ok(())
    })
}
pub fn available() -> bool {
    critical_section::with(|cs| PROVIDER.borrow(cs).borrow().is_some())
}
pub fn dispatch(request: Request) -> Reply {
    match request {
        Request::Snapshot { nonce } if nonce == [0; 32] => return Reply::Refused,
        Request::Chunk { bytes, offset, .. }
            if bytes == 0 || bytes > 8192 || offset >= bytes || offset % 512 != 0 =>
        {
            return Reply::Refused
        }
        _ => {}
    }
    let provider = critical_section::with(|cs| *PROVIDER.borrow(cs).borrow());
    provider.map_or(Reply::Unavailable, |provider| provider(request))
}
fn hex(text: &str) -> Result<[u8; 32], ()> {
    if text.len() != 64 {
        return Err(());
    }
    let mut out = [0; 32];
    for (byte, pair) in out.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        let digit = |b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(()),
        };
        *byte = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(out)
}
fn number(text: &str) -> Result<u64, ()> {
    if text.is_empty()
        || text.len() > 20
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(());
    }
    text.parse().map_err(|_| ())
}
pub(crate) fn parse(line: &str) -> Result<Option<Request>, ()> {
    if !line.starts_with("GET /face/export/") {
        return Ok(None);
    }
    let (path, version) = line
        .strip_prefix("GET ")
        .ok_or(())?
        .split_once(' ')
        .ok_or(())?;
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return Err(());
    }
    if let Some(nonce) = path.strip_prefix("/face/export/snapshot?nonce=") {
        let nonce = hex(nonce)?;
        if nonce == [0; 32] {
            return Err(());
        }
        return Ok(Some(Request::Snapshot { nonce }));
    }
    let tail = path.strip_prefix("/face/export/chunk?digest=").ok_or(())?;
    let (digest, tail) = tail.split_once("&revision=").ok_or(())?;
    let (revision, tail) = tail.split_once("&bytes=").ok_or(())?;
    let (bytes, offset) = tail.split_once("&offset=").ok_or(())?;
    let (revision, bytes, offset) = (number(revision)?, number(bytes)?, number(offset)?);
    if bytes == 0 || bytes > 8192 || offset >= bytes || offset % 512 != 0 {
        return Err(());
    }
    Ok(Some(Request::Chunk {
        digest: hex(digest)?,
        revision_le: revision.to_le_bytes(),
        bytes: bytes as u32,
        offset: offset as u32,
    }))
}
#[cfg(test)]
#[path = "face_export_tests.rs"]
mod tests;
