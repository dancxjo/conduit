//! Fixed-capacity wall observations of existing installed std phase markers.
use conduit_std_host::{KernelTimingPhase, TimerAdapter};
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct PhaseObserver {
    origin: Option<Instant>,
    markers: [Option<(KernelTimingPhase, u64)>; 3],
    count: usize,
    end: Option<u64>,
    begin_counts: Option<crate::allocation::Counts>,
    marker_counts: [Option<crate::allocation::Counts>; 3],
    end_counts: Option<crate::allocation::Counts>,
}
#[derive(Debug, Serialize)]
pub(super) struct PhaseWall {
    marker_offsets_ns: [u64; 3],
    call_wall_ns: u64,
    before_preparation_ns: u64,
    back_preparation_ns: u64,
    play_ns: u64,
    after_play_ns: u64,
    /// before preparation, Back preparation, Play, then after Play; absent when
    /// allocator measurement is inactive. Play includes std output adaptation.
    allocator_intervals: Option<[crate::allocation::Interval; 4]>,
}
impl PhaseObserver {
    pub(super) fn begin(&mut self) {
        self.origin = Some(Instant::now());
        self.begin_counts = crate::allocation::snapshot();
    }
    pub(super) fn end(&mut self) {
        self.end = self.origin.map(|start| crate::timing::ns(start.elapsed()));
        self.end_counts = crate::allocation::snapshot();
    }
    pub(super) fn report(&self) -> Result<PhaseWall, &'static str> {
        use KernelTimingPhase::*;
        if self.count != 3 {
            return Err("expected exactly three installed phase markers");
        }
        let mut offsets = [0; 3];
        for (index, expected) in [PreparationStarted, PlayStarted, PlayFinished]
            .into_iter()
            .enumerate()
        {
            let (phase, offset) = self.markers[index].ok_or("missing installed phase marker")?;
            if phase != expected {
                return Err("installed phase markers are out of order");
            }
            offsets[index] = offset;
        }
        let mut report = partition(offsets, self.end.ok_or("call end absent")?)?;
        report.allocator_intervals = (|| {
            let begin = self.begin_counts.as_ref()?;
            let [Some(preparation), Some(start), Some(finish)] = &self.marker_counts else {
                return None;
            };
            let end = self.end_counts.as_ref()?;
            Some([
                begin.interval_to(preparation)?,
                preparation.interval_to(start)?,
                start.interval_to(finish)?,
                finish.interval_to(end)?,
            ])
        })();
        Ok(report)
    }
}
fn partition(offsets: [u64; 3], end: u64) -> Result<PhaseWall, &'static str> {
    let [preparation, start, finish] = offsets;
    if preparation > start || start > finish || finish > end {
        return Err("phase wall observations are not monotonic");
    }
    Ok(PhaseWall {
        marker_offsets_ns: offsets,
        call_wall_ns: end,
        before_preparation_ns: preparation,
        back_preparation_ns: start - preparation,
        play_ns: finish - start,
        after_play_ns: end - finish,
        allocator_intervals: None,
    })
}
impl TimerAdapter for PhaseObserver {
    fn wait(&mut self, _: Duration) {}
    fn observe_kernel_phase(&mut self, phase: KernelTimingPhase) {
        if let (Some(origin), Some(marker)) = (self.origin, self.markers.get_mut(self.count)) {
            *marker = Some((phase, crate::timing::ns(origin.elapsed())));
            self.marker_counts[self.count] = crate::allocation::snapshot();
        }
        self.count = self.count.saturating_add(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phases_partition_the_call_and_refuse_nonmonotonic_samples() {
        let report = partition([2, 5, 12], 17).unwrap();
        assert_eq!(
            (
                report.before_preparation_ns,
                report.back_preparation_ns,
                report.play_ns,
                report.after_play_ns
            ),
            (2, 3, 7, 5)
        );
        for (offsets, end) in [([5, 2, 12], 17), ([2, 12, 5], 17), ([2, 5, 18], 17)] {
            assert!(partition(offsets, end).is_err());
        }
    }
    #[test]
    fn missing_duplicate_reordered_and_extra_markers_refuse() {
        use KernelTimingPhase::*;
        for phases in [
            vec![PreparationStarted, PlayStarted],
            vec![PreparationStarted, PlayStarted, PlayStarted],
            vec![PlayStarted, PreparationStarted, PlayFinished],
            vec![PreparationStarted, PlayStarted, PlayFinished, PlayFinished],
        ] {
            let mut observer = PhaseObserver::default();
            observer.begin();
            for phase in phases {
                observer.observe_kernel_phase(phase);
            }
            observer.end();
            assert!(observer.report().is_err());
        }
        let mut observer = PhaseObserver::default();
        observer.begin();
        for phase in [PreparationStarted, PlayStarted, PlayFinished] {
            observer.observe_kernel_phase(phase);
        }
        assert!(observer.report().is_err());
        observer.end();
        assert!(observer.report().is_ok());
    }
}
