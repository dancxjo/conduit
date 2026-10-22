//! Per-call canonical cache deltas. Never authority, a cache, or a progress phase.
use conduit_plot::PreparationCacheStats;

pub(super) fn publish(
    plot: super::NativePlot,
    before: PreparationCacheStats,
    after: PreparationCacheStats,
    sources: usize,
    expansions: usize,
    fallback: bool,
    accepted: bool,
) {
    #[cfg(all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    ))]
    guest::publish(plot, before, after, sources, expansions, fallback, accepted);
    #[cfg(not(all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    )))]
    let _ = (plot, before, after, sources, expansions, fallback, accepted);
}

#[cfg(any(
    test,
    all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    )
))]
fn delta(before: PreparationCacheStats, after: PreparationCacheStats) -> Option<[u64; 7]> {
    let fields = |s: PreparationCacheStats| {
        [
            s.checks,
            s.check_hits,
            s.expansions,
            s.expansion_hits,
            s.source_evictions,
            s.expansion_evictions,
            s.unretained_checks,
        ]
    };
    let mut result = [0; 7];
    for ((out, before), after) in result.iter_mut().zip(fields(before)).zip(fields(after)) {
        if after == u64::MAX {
            return None;
        }
        *out = after.checked_sub(before)?;
    }
    Some(result)
}

#[cfg(any(
    test,
    all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    )
))]
mod admission {
    use core::sync::atomic::{AtomicU32, Ordering};
    pub(super) const LIMIT: u32 = 512;
    pub(super) struct Observer(AtomicU32);
    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum Admission {
        Record(u32),
        Refusal,
        Silent,
    }
    impl Observer {
        pub(super) const fn new() -> Self {
            Self(AtomicU32::new(0))
        }
        pub(super) fn admit(&self) -> Admission {
            match self
                .0
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                    (next <= LIMIT).then(|| next + 1)
                }) {
                Ok(sequence) if sequence < LIMIT => Admission::Record(sequence),
                Ok(_) => Admission::Refusal,
                Err(_) => Admission::Silent,
            }
        }
        // First failure freezes only telemetry. Never consulted by preparation.
        pub(super) fn freeze(&self) -> bool {
            self.0.swap(LIMIT + 1, Ordering::Relaxed) <= LIMIT
        }
    }
}

#[cfg(all(
    target_arch = "x86_64",
    target_os = "none",
    feature = "native-latency-observation"
))]
mod guest {
    use super::admission::{Admission, Observer};
    use super::*;
    use core::fmt::{self, Write};
    use sha2::{Digest, Sha256};
    static OBSERVER: Observer = Observer::new();
    pub(super) fn publish(
        plot: super::super::NativePlot,
        before: PreparationCacheStats,
        after: PreparationCacheStats,
        sources: usize,
        expansions: usize,
        fallback: bool,
        accepted: bool,
    ) {
        let sequence = match OBSERVER.admit() {
            Admission::Record(sequence) => sequence,
            Admission::Refusal => {
                write_refusal("observation-bound");
                return;
            }
            Admission::Silent => return,
        };
        let Some(stats) = delta(before, after) else {
            refuse("counter-bound");
            return;
        };
        let zero = cfg!(feature = "native-cache-zero-retention");
        let mode = if zero { "zero-retention" } else { "retained" };
        let limits = if zero {
            [0, 0, 0]
        } else {
            [
                super::super::super::NATIVE_CANDIDATE_CAPACITY,
                1,
                super::super::SOURCE_BYTES,
            ]
        };
        let mut line = Line {
            bytes: [0; 768],
            length: 0,
        };
        let result = (|| {
            write!(
                line,
                "CONDUIT_NATIVE_CACHE {{\"schema\":\"conduit.conduitos/native-cache@1\",\"scope\":\"native-root-boot\",\"sequence\":{sequence},\"mode\":\"{mode}\",\"limits\":[{},{},{}],\"plot\":\"{}\",\"source_sha256\":\"",
                limits[0],
                limits[1],
                limits[2],
                plot.name()
            )?;
            for byte in Sha256::digest(plot.source().as_bytes()) {
                write!(line, "{byte:02x}")?;
            }
            writeln!(
                line,
                "\",\"path\":\"{}\",\"accepted\":{accepted},\"delta\":[{},{},{},{},{},{},{}],\"retained\":[{sources},{expansions}]}}",
                if fallback {
                    "private-contention"
                } else {
                    "shared"
                },
                stats[0],
                stats[1],
                stats[2],
                stats[3],
                stats[4],
                stats[5],
                stats[6]
            )
        })();
        if result.is_ok() {
            crate::arch::early_write(&line.bytes[..line.length]);
        } else {
            refuse("record-bound");
        }
    }
    fn refuse(reason: &str) {
        if OBSERVER.freeze() {
            write_refusal(reason);
        }
    }
    fn write_refusal(reason: &str) {
        crate::arch::early_write(b"CONDUIT_NATIVE_CACHE_REFUSAL ");
        crate::arch::early_write(reason.as_bytes());
        crate::arch::early_write(b"\n");
    }
    struct Line {
        bytes: [u8; 768],
        length: usize,
    }
    impl Write for Line {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
            self.bytes
                .get_mut(self.length..end)
                .ok_or(fmt::Error)?
                .copy_from_slice(text.as_bytes());
            self.length = end;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_sequence_admission_has_one_terminal_refusal_after_512_records() {
        use admission::{Admission, LIMIT, Observer};
        let observer = Observer::new();
        for sequence in 0..LIMIT {
            assert_eq!(observer.admit(), Admission::Record(sequence));
        }
        assert_eq!(observer.admit(), Admission::Refusal);
        assert_eq!(observer.admit(), Admission::Silent);
        assert_eq!(observer.admit(), Admission::Silent);
        assert!(!observer.freeze());
    }
    #[test]
    fn counter_failure_and_early_refusal_freeze_only_local_observer() {
        use admission::{Admission, Observer};
        for counter_failure in [false, true] {
            let observer = Observer::new();
            assert_eq!(observer.admit(), Admission::Record(0));
            if counter_failure {
                let saturated = PreparationCacheStats {
                    checks: u64::MAX,
                    ..PreparationCacheStats::default()
                };
                assert!(delta(PreparationCacheStats::default(), saturated).is_none());
            }
            assert!(observer.freeze());
            assert!(!observer.freeze());
            assert_eq!(observer.admit(), Admission::Silent);
            assert_eq!(Observer::new().admit(), Admission::Record(0));
        }
    }
    #[test]
    fn deltas_refuse_saturation_and_reset_instead_of_hiding_activity() {
        let initial = PreparationCacheStats::default();
        let mut next = initial;
        next.checks = 1;
        assert_eq!(delta(initial, next).unwrap(), [1, 0, 0, 0, 0, 0, 0]);
        assert!(delta(next, initial).is_none());
        next.expansion_hits = u64::MAX;
        assert!(delta(initial, next).is_none());
    }
}
