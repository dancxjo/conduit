//! Stable exclusively owned allocations loaned once to the TLS connection.
//! Raw ownership avoids moving/retagging a Box while its allocation is borrowed.
use crate::{RECEIVE_BYTES, Refusal, TRANSMIT_BYTES, buffer};
use alloc::vec::Vec;
use core::{ptr::NonNull, slice};
struct Allocation {
    pointer: NonNull<u8>,
    length: usize,
    capacity: usize,
}
impl Allocation {
    fn new(bytes: &[u8]) -> Result<Self, Refusal> {
        let mut value = buffer(bytes.len())?;
        value.copy_from_slice(bytes);
        let result = Self {
            pointer: NonNull::new(value.as_mut_ptr()).ok_or(Refusal::Capacity)?,
            length: value.len(),
            capacity: value.capacity(),
        };
        core::mem::forget(value);
        Ok(result)
    }
    fn zeroed(length: usize) -> Result<Self, Refusal> {
        let mut value = buffer(length)?;
        let result = Self {
            pointer: NonNull::new(value.as_mut_ptr()).ok_or(Refusal::Capacity)?,
            length,
            capacity: value.capacity(),
        };
        core::mem::forget(value);
        Ok(result)
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        // SAFETY: Resources is dropped only after all loans/futures/connection
        // are dropped. This unique raw owner then erases and frees its allocation.
        unsafe {
            for index in 0..self.capacity {
                self.pointer.as_ptr().add(index).write_volatile(0);
            }
            drop(Vec::from_raw_parts(
                self.pointer.as_ptr(),
                self.length,
                self.capacity,
            ));
        }
    }
}
pub(crate) struct Resources {
    rx: Allocation,
    tx: Allocation,
    host: Allocation,
    cert: Allocation,
    loaned: bool,
}
pub(crate) struct Loans {
    pub rx: &'static mut [u8],
    pub tx: &'static mut [u8],
    pub host: &'static str,
    pub cert: &'static [u8],
}
impl Resources {
    pub fn new(host: &str, cert: &[u8]) -> Result<Self, Refusal> {
        Ok(Self {
            rx: Allocation::zeroed(RECEIVE_BYTES)?,
            tx: Allocation::zeroed(TRANSMIT_BYTES)?,
            host: Allocation::new(host.as_bytes())?,
            cert: Allocation::new(cert)?,
            loaned: false,
        })
    }
    /// # Safety
    /// Caller owns this Resources until every returned loan is dead. Loans may
    /// only enter the unique Session connection or its one pinned owning future.
    /// Session Drop must destroy that future, then the connection, before Self.
    /// No allocation is read/reborrowed/mutated/freed through this raw owner while
    /// loaned. Moving raw owner metadata never moves or retags the allocations.
    pub unsafe fn loan_once(&mut self) -> Result<Loans, Refusal> {
        if self.loaned {
            return Err(Refusal::Order);
        }
        self.loaned = true;
        // SAFETY: disjoint live allocations, initialized above, exclusive loan;
        // host UTF-8 was checked before allocation and cannot change afterwards.
        unsafe {
            Ok(Loans {
                rx: slice::from_raw_parts_mut(self.rx.pointer.as_ptr(), self.rx.length),
                tx: slice::from_raw_parts_mut(self.tx.pointer.as_ptr(), self.tx.length),
                host: core::str::from_utf8_unchecked(slice::from_raw_parts(
                    self.host.pointer.as_ptr(),
                    self.host.length,
                )),
                cert: slice::from_raw_parts(self.cert.pointer.as_ptr(), self.cert.length),
            })
        }
    }
}
