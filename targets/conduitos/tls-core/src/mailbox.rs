//! Async IO yields to Root once for each correlated read/write/flush grant.
use crate::{MAX_BLOB, TRANSMIT_BYTES, buffer};
use alloc::{rc::Rc, vec::Vec};
use core::{cell::RefCell, future::poll_fn, task::Poll};
use embedded_io::{ErrorKind, ErrorType};
#[derive(Clone)]
pub(crate) struct Mailbox(pub Rc<RefCell<Bus>>);
pub(crate) enum Grant {
    Read {
        event: u64,
        maximum: usize,
        data: Option<Vec<u8>>,
    },
    Write {
        event: u64,
        data: Vec<u8>,
        read: usize,
        acked: bool,
    },
    Flush {
        event: u64,
        acked: bool,
    },
}
pub(crate) struct Bus {
    pub operation: u64,
    pub next_event: u64,
    pub grant: Option<Grant>,
}
impl Bus {
    pub fn event(&mut self) -> Result<u64, ErrorKind> {
        let result = self.next_event;
        self.next_event = self.next_event.checked_add(1).ok_or(ErrorKind::Other)?;
        Ok(result)
    }
}
impl Default for Mailbox {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(Bus {
            operation: 0,
            next_event: 1,
            grant: None,
        })))
    }
}
impl ErrorType for Mailbox {
    type Error = ErrorKind;
}
impl embedded_io_async::Read for Mailbox {
    async fn read(&mut self, out: &mut [u8]) -> Result<usize, Self::Error> {
        if out.is_empty() {
            return Ok(0);
        }
        poll_fn(|_| {
            let mut bus = self.0.borrow_mut();
            match &mut bus.grant {
                Some(Grant::Read { data: Some(_), .. }) => {
                    let Some(Grant::Read {
                        data: Some(data), ..
                    }) = bus.grant.take()
                    else {
                        return Poll::Ready(Err(ErrorKind::Other));
                    };
                    if data.len() > out.len() {
                        return Poll::Ready(Err(ErrorKind::InvalidData));
                    }
                    out[..data.len()].copy_from_slice(&data);
                    Poll::Ready(Ok(data.len()))
                }
                Some(Grant::Read { data: None, .. }) => Poll::Pending,
                None => {
                    let event = match bus.event() {
                        Ok(event) => event,
                        Err(error) => return Poll::Ready(Err(error)),
                    };
                    bus.grant = Some(Grant::Read {
                        event,
                        maximum: out.len().min(MAX_BLOB),
                        data: None,
                    });
                    Poll::Pending
                }
                _ => Poll::Ready(Err(ErrorKind::InvalidData)),
            }
        })
        .await
    }
}
impl embedded_io_async::Write for Mailbox {
    async fn write(&mut self, input: &[u8]) -> Result<usize, Self::Error> {
        if input.is_empty() {
            return Ok(0);
        }
        poll_fn(|_| {
            let mut bus = self.0.borrow_mut();
            match &mut bus.grant {
                Some(Grant::Write { acked: true, .. }) => {
                    let Some(Grant::Write { data, .. }) = bus.grant.take() else {
                        return Poll::Ready(Err(ErrorKind::Other));
                    };
                    Poll::Ready(Ok(data.len()))
                }
                Some(Grant::Write { acked: false, .. }) => Poll::Pending,
                None => {
                    let event = match bus.event() {
                        Ok(event) => event,
                        Err(error) => return Poll::Ready(Err(error)),
                    };
                    let count = input.len().min(TRANSMIT_BYTES);
                    let mut data = match buffer(count) {
                        Ok(data) => data,
                        Err(_) => return Poll::Ready(Err(ErrorKind::OutOfMemory)),
                    };
                    data.copy_from_slice(&input[..count]);
                    bus.grant = Some(Grant::Write {
                        event,
                        data,
                        read: 0,
                        acked: false,
                    });
                    Poll::Pending
                }
                _ => Poll::Ready(Err(ErrorKind::InvalidData)),
            }
        })
        .await
    }
    async fn flush(&mut self) -> Result<(), Self::Error> {
        poll_fn(|_| {
            let mut bus = self.0.borrow_mut();
            match &mut bus.grant {
                Some(Grant::Flush { acked: true, .. }) => {
                    bus.grant = None;
                    Poll::Ready(Ok(()))
                }
                Some(Grant::Flush { acked: false, .. }) => Poll::Pending,
                None => {
                    let event = match bus.event() {
                        Ok(event) => event,
                        Err(error) => return Poll::Ready(Err(error)),
                    };
                    bus.grant = Some(Grant::Flush {
                        event,
                        acked: false,
                    });
                    Poll::Pending
                }
                _ => Poll::Ready(Err(ErrorKind::InvalidData)),
            }
        })
        .await
    }
}
