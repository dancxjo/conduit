//! Root-only bounded progress. These observations are not lifecycle Signs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    XhciSetup,
    UsbEnumeration,
    MachineSetup,
    IdentityGeneration,
    DisplaySetup,
    InputSetup,
    InputWait,
    HostPreparation,
    NetworkSetup,
    FaceConstruction,
    MaskPreparation,
    FirstShow,
    Scanout,
    PlotBuild,
    CatalogInstall,
    SourceParse,
    SourceCheck,
    SourceExpand,
    IdentityResolve,
    WorksetPlanning,
    WorksetLowering,
    KernelPrepare,
}
impl Phase {
    pub const ALL: [Self; 22] = [
        Self::XhciSetup,
        Self::UsbEnumeration,
        Self::MachineSetup,
        Self::IdentityGeneration,
        Self::DisplaySetup,
        Self::InputSetup,
        Self::InputWait,
        Self::HostPreparation,
        Self::NetworkSetup,
        Self::FaceConstruction,
        Self::MaskPreparation,
        Self::FirstShow,
        Self::Scanout,
        Self::PlotBuild,
        Self::CatalogInstall,
        Self::SourceParse,
        Self::SourceCheck,
        Self::SourceExpand,
        Self::IdentityResolve,
        Self::WorksetPlanning,
        Self::WorksetLowering,
        Self::KernelPrepare,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::XhciSetup => "xhci-setup",
            Self::UsbEnumeration => "usb-enumeration",
            Self::MachineSetup => "machine-setup",
            Self::IdentityGeneration => "identity-generation",
            Self::DisplaySetup => "display-setup",
            Self::InputSetup => "input-setup",
            Self::InputWait => "input-wait",
            Self::HostPreparation => "host-preparation",
            Self::NetworkSetup => "network-setup",
            Self::FaceConstruction => "face-construction",
            Self::MaskPreparation => "mask-preparation",
            Self::FirstShow => "first-show",
            Self::Scanout => "scanout",
            Self::PlotBuild => "plot-build",
            Self::CatalogInstall => "catalog-install",
            Self::SourceParse => "source-parse",
            Self::SourceCheck => "source-check",
            Self::SourceExpand => "source-expand",
            Self::IdentityResolve => "identity-resolve",
            Self::WorksetPlanning => "workset-planning",
            Self::WorksetLowering => "workset-lowering",
            Self::KernelPrepare => "kernel-prepare",
        }
    }
}

#[cfg(any(test, all(target_arch = "x86_64", target_os = "none")))]
mod state;
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use state::{Event, Tracker};
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
static TRACKER: Tracker = Tracker::new();

/// One synchronous Root scope. Other CPU domains must not publish this stream.
/// A dropped scope is explicitly incomplete, never an inferred success.
pub struct Progress {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    token: Option<state::Token>,
}
impl Progress {
    pub fn begin(phase: Phase, context: &'static str) -> Self {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            match TRACKER.begin(phase, context, super::raw_counter::read()) {
                Ok(Some((token, record))) => {
                    publish(Ok(Some(record)));
                    Self { token: Some(token) }
                }
                Ok(None) => Self { token: None },
                Err(reason) => {
                    publish(Err(reason));
                    Self { token: None }
                }
            }
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        {
            let _ = (phase, context);
            Self {}
        }
    }
    pub fn measure<T>(phase: Phase, context: &'static str, operation: impl FnOnce() -> T) -> T {
        let span = Self::begin(phase, context);
        let result = operation();
        span.finish();
        result
    }
    pub fn result<T, E>(
        phase: Phase,
        context: &'static str,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let span = Self::begin(phase, context);
        let result = operation();
        if result.is_ok() {
            span.finish();
        } else {
            span.refuse();
        }
        result
    }
    pub fn finish(mut self) {
        self.close(false, false);
    }
    pub fn refuse(mut self) {
        self.close(true, false);
    }
    fn close(&mut self, refused: bool, abandoned: bool) {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        if let Some(token) = self.token.take() {
            let event = if abandoned {
                Event::Abandoned
            } else if refused {
                Event::Refused
            } else {
                Event::Finished
            };
            publish(TRACKER.close(token, event, super::raw_counter::read()));
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        let _ = (refused, abandoned);
    }
}
impl Drop for Progress {
    fn drop(&mut self) {
        self.close(false, true);
    }
}

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
fn publish(result: Result<Option<state::Record>, &'static str>) {
    match result {
        Ok(Some(record)) => {
            let mut line = state::Line::new();
            if record.write(&mut line).is_ok() {
                crate::arch::early_write(line.bytes());
            } else if let Err(reason) = TRACKER.fail::<state::Record>("record-bound") {
                publish(Err(reason));
            }
        }
        Ok(None) => {}
        Err(reason) => {
            crate::arch::early_write(b"CONDUIT_NATIVE_PROGRESS_REFUSAL ");
            crate::arch::early_write(reason.as_bytes());
            crate::arch::early_write(b"\n");
        }
    }
}
