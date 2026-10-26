//! Bounded observations of existing domain counters; never an admission input.
use crate::protected_region::DomainCost;
use core::fmt::Write;
pub(super) struct InputCost {
    before: DomainCost,
    writes: u8,
    max_write_rtc: u64,
}
fn rtc(before: DomainCost, after: DomainCost) -> u64 {
    after
        .interrupt_entries
        .saturating_sub(before.interrupt_entries)
        .saturating_sub(
            after
                .source_timer_interrupts
                .saturating_sub(before.source_timer_interrupts),
        )
}
impl InputCost {
    pub fn new(before: DomainCost) -> Self {
        Self {
            before,
            writes: 0,
            max_write_rtc: 0,
        }
    }
    pub fn write(&mut self, before: DomainCost, after: DomainCost) {
        self.writes += 1; // At most 128KiB / 16KiB = eight existing writes.
        self.max_write_rtc = self.max_write_rtc.max(rtc(before, after));
    }
    pub fn finish(self, bytes: u32, commit_before: DomainCost, after: DomainCost) {
        let mut line = crate::sign_format::FixedText::new();
        if writeln!(line,
            "CONDUIT_CRECHE_INPUT_PHASE_COST bytes={} writes={} max_write_rtc={} commit_rtc={} entries={} rtc={} source_timer={}",
            bytes,self.writes,self.max_write_rtc,rtc(commit_before,after),
            after.entries.saturating_sub(self.before.entries),rtc(self.before,after),
            after.source_timer_interrupts.saturating_sub(self.before.source_timer_interrupts),
        ).is_ok() {crate::arch::early_write(line.as_bytes());}
    }
}
