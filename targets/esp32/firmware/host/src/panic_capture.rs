//! Terminal, bounded ROM-UART fault evidence; never a protocol acknowledgment.
use core::{fmt::{self, Write}, panic::PanicInfo};
struct Report { remaining: usize }
impl Write for Report {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        unsafe extern "C" { fn uart_tx_one_char(byte: u8) -> i32; }
        for byte in text.bytes() {
            if self.remaining == 0 { return Err(fmt::Error); }
            // SAFETY: terminal panic path uses the ESP32 ROM UART0 transmitter.
            unsafe { uart_tx_one_char(byte); }
            self.remaining -= 1;
        }
        Ok(())
    }
}
#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let _ = writeln!(Report { remaining: 512 }, "\nCONDUIT_PANIC@1 heap_used={} {info}", esp_alloc::HEAP.used());
    loop { core::hint::spin_loop(); }
}
