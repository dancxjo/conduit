//! Root-side native timing observations, separate from Plot meaning and Signs.
//!
//! Counter ticks have no inferred frequency. Serial publication is outside each
//! interval, but enclosing intervals include nested observation publication.
//! These records describe instrumented execution, not an uninstrumented baseline.

#[derive(Clone, Copy)]
pub enum Stage {
    BootEntry,
    FirstFramebufferCommit,
    PlotBuild,
    PlanSeal,
    WorksetPrepare,
    KernelPrepare,
    PlayStart,
    PlayInput,
    PointerFeedback,
}
impl Stage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BootEntry => "boot-entry",
            Self::FirstFramebufferCommit => "first-framebuffer-commit",
            Self::PlotBuild => "plot-build",
            Self::PlanSeal => "plan-seal",
            Self::WorksetPrepare => "workset-prepare",
            Self::KernelPrepare => "kernel-prepare",
            Self::PlayStart => "play-start",
            Self::PlayInput => "play-input",
            Self::PointerFeedback => "pointer-feedback",
        }
    }
}

#[cfg(all(
    target_arch = "x86_64",
    target_os = "none",
    feature = "native-latency-observation"
))]
#[path = "native_observation/counter.rs"]
mod counter;

pub struct Interval {
    #[cfg(all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    ))]
    stage: Stage,
    #[cfg(all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    ))]
    start: u64,
}
impl Interval {
    #[inline]
    pub fn begin(stage: Stage) -> Self {
        #[cfg(all(
            target_arch = "x86_64",
            target_os = "none",
            feature = "native-latency-observation"
        ))]
        {
            Self {
                stage,
                start: counter::read(),
            }
        }
        #[cfg(not(all(
            target_arch = "x86_64",
            target_os = "none",
            feature = "native-latency-observation"
        )))]
        {
            let _ = stage;
            Self {}
        }
    }
}
impl Drop for Interval {
    fn drop(&mut self) {
        #[cfg(all(
            target_arch = "x86_64",
            target_os = "none",
            feature = "native-latency-observation"
        ))]
        counter::publish(self.stage, self.start, counter::read());
    }
}

#[inline]
pub fn point(stage: Stage) {
    #[cfg(all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    ))]
    {
        let tick = counter::read();
        counter::publish(stage, tick, tick);
    }
    #[cfg(not(all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    )))]
    let _ = stage;
}

/// Raw observation clock for existing identity-bearing receipts. Publication
/// and serialization costs follow this sample; no conversion to elapsed time.
pub fn counter_ticks() -> Option<u64> {
    #[cfg(all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    ))]
    {
        Some(counter::read())
    }
    #[cfg(not(all(
        target_arch = "x86_64",
        target_os = "none",
        feature = "native-latency-observation"
    )))]
    {
        None
    }
}
