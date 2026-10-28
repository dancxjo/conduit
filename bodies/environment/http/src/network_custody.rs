//! Read-only transport for Root-endorsed recovery snapshots. No request carries
//! Body scope, signing keys, an erase command, or admitted custody authority.
pub use crate::observed_custody::{render, Reply};
use core::cell::RefCell;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Offer,
    Chunk { snapshot: [u8; 32], offset: u32 },
}
// Keep this rare control payload out of every retained HTTP task frame.
// One exact fallible allocation belongs only to its active request.
pub(crate) fn owned(request: Request) -> Result<alloc::boxed::Box<Request>, ()> {
    let layout = core::alloc::Layout::new::<Request>();
    let raw = unsafe { alloc::alloc::alloc(layout) }.cast::<Request>();
    if raw.is_null() {
        return Err(());
    }
    // SAFETY: exact Request layout, checked nonnull, fully initialized before Box.
    unsafe {
        raw.write(request);
        Ok(alloc::boxed::Box::from_raw(raw))
    }
}
pub type Provider = fn(Request) -> Reply;
static PROVIDER: critical_section::Mutex<RefCell<Option<Provider>>> =
    critical_section::Mutex::new(RefCell::new(None));
pub fn available() -> bool {
    critical_section::with(|cs| PROVIDER.borrow(cs).borrow().is_some())
}
pub fn install_provider(provider: Provider) -> Result<(), &'static str> {
    critical_section::with(|cs| {
        let mut slot = PROVIDER.borrow(cs).borrow_mut();
        if slot.is_some() {
            return Err("network-custody-provider-already-installed");
        }
        *slot = Some(provider);
        Ok(())
    })
}
pub fn dispatch(request: Request) -> Reply {
    if let Request::Chunk { offset, .. } = request {
        if offset >= 32768 || offset % 512 != 0 {
            return Reply::Refused;
        }
    }
    let provider = critical_section::with(|cs| *PROVIDER.borrow(cs).borrow());
    provider.map_or(Reply::Unavailable, |provider| provider(request))
}
/// Exact bounded GET grammar. The snapshot pin prevents mixing chunks from
/// different leases; offsets identify fixed512-byte chunks of the full region.
pub(crate) fn parse(line: &str) -> Result<Option<Request>, ()> {
    if !line.starts_with("GET /network/custody/") {
        return Ok(None);
    }
    let (target, version) = line
        .strip_prefix("GET ")
        .ok_or(())?
        .split_once(' ')
        .ok_or(())?;
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return Err(());
    }
    if target == "/network/custody/offer" {
        return Ok(Some(Request::Offer));
    }
    let tail = target
        .strip_prefix("/network/custody/chunk?snapshot=")
        .ok_or(())?;
    let (hex, number) = tail.split_once("&offset=").ok_or(())?;
    if hex.len() != 64
        || number.is_empty()
        || number.len() > 5
        || !number.bytes().all(|b| b.is_ascii_digit())
        || (number.len() > 1 && number.starts_with('0'))
    {
        return Err(());
    }
    let mut snapshot = [0; 32];
    fn digit(b: u8) -> Result<u8, ()> {
        match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(()),
        }
    }
    for (out, pair) in snapshot.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        *out = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    let offset = number.parse::<u32>().map_err(|_| ())?;
    if offset >= 32768 || offset % 512 != 0 {
        return Err(());
    }
    Ok(Some(Request::Chunk { snapshot, offset }))
}
