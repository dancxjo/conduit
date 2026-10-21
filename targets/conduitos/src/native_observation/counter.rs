//! Fixed-storage serial publication. Root is the only observation producer.
use super::Stage;
use core::{
    fmt::{self, Write},
    sync::atomic::{AtomicU32, Ordering},
};

// A bounded observation family. Further events emit one explicit exhaustion
// record; consumers must reject that family instead of truncating silently.
const LIMIT: u32 = 2048;
static NEXT: AtomicU32 = AtomicU32::new(0);

pub(super) use super::raw_counter::read;

pub(super) fn publish(stage: Stage, start: u64, end: u64) {
    let sequence = match NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
        (next <= LIMIT).then(|| next + 1)
    }) {
        Ok(sequence) => sequence,
        Err(_) => return,
    };
    if sequence == LIMIT {
        crate::arch::early_write(b"CONDUIT_NATIVE_TIMING_REFUSAL observation-bound\n");
        return;
    }
    let mut line = Line {
        bytes: [0; 320],
        length: 0,
    };
    let result = writeln!(
        line,
        "CONDUIT_NATIVE_TIMING {{\"schema\":\"conduit.conduitos/native-timing@1\",\"sequence\":{},\"stage\":\"{}\",\"clock\":\"x86-tsc-lfence\",\"frequency_hz\":null,\"start_ticks\":{},\"end_ticks\":{},\"counter_reversed\":{}}}",
        sequence,
        stage.as_str(),
        start,
        end,
        end < start
    );
    if result.is_ok() {
        crate::arch::early_write(&line.bytes[..line.length]);
    } else {
        crate::arch::early_write(b"CONDUIT_NATIVE_TIMING_REFUSAL record-bound\n");
    }
}
struct Line {
    bytes: [u8; 320],
    length: usize,
}
impl Write for Line {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.length..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
