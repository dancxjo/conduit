//! Bounded bootstrap diagnostic bytes through the retained PL011 provider.
use crate::machine::BaseError;

/// Preserve the UART FIFO and append one finite diagnostic envelope.
pub fn append_boot_diagnostic(bytes: &[u8]) -> Result<(), BaseError> {
    if bytes.len() > crate::sign_format::MAX_STRUCTURED_SIGN_BYTES {
        return Err(BaseError::PayloadTooLarge);
    }
    for &byte in bytes {
        let mut ready = false;
        for _ in 0..100_000 {
            if unsafe { super::read32(super::UART_BASE + 0x18) } & (1 << 5) == 0 {
                ready = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !ready {
            return Err(BaseError::Unavailable);
        }
        unsafe { super::write32(super::UART_BASE, u32::from(byte)) };
    }
    Ok(())
}
