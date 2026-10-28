//! Public immutable-epoch offers. Transport access confers no erase authority.
use alloc::vec::Vec;
use core::{cell::RefCell, fmt::Write};
pub enum Reply {
    Pending,
    Offer(Vec<u8>),
    Refused,
    Unavailable,
}
pub type Provider = fn(u8) -> Reply;
static PROVIDER: critical_section::Mutex<RefCell<Option<Provider>>> =
    critical_section::Mutex::new(RefCell::new(None));
pub fn install_provider(provider: Provider) -> Result<(), &'static str> {
    critical_section::with(|cs| {
        let mut current = PROVIDER.borrow(cs).borrow_mut();
        if current.is_some() {
            return Err("observed-custody-provider-already-installed");
        }
        *current = Some(provider);
        Ok(())
    })
}
pub fn available() -> bool {
    critical_section::with(|cs| PROVIDER.borrow(cs).borrow().is_some())
}
pub fn dispatch(slot: u8) -> Reply {
    let provider = critical_section::with(|cs| *PROVIDER.borrow(cs).borrow());
    if slot > 5 {
        return Reply::Refused;
    }
    provider.map_or(Reply::Unavailable, |provider| provider(slot))
}
pub fn render(reply: Reply, response: &mut heapless::String<2048>) -> bool {
    let status = match reply {
        Reply::Offer(bytes) => {
            return crate::body_join::render(crate::body_join::Reply::Receipt(bytes), response)
        }
        Reply::Pending => "202 Accepted",
        Reply::Refused => "409 Conflict",
        Reply::Unavailable => "503 Service Unavailable",
    };
    response.clear();
    write!(response, "HTTP/1.1 {status}\r\nAccess-Control-Allow-Origin: *\r\nRetry-After: 1\r\nCache-Control: no-store\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").is_ok()
}
