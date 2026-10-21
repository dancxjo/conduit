//! Checked meaning of the exact plots offered at native birth.
use super::WorksetRefusal;
use conduit_body::ResidentPlot;
use conduit_plot::ExpandedCanonicalPlot;

/// Finite native product profile. Capacity is a reviewed deployment choice,
/// not a claim that a body conceptually consists of these particular Plots.
pub const NATIVE_PLOT_CAPACITY: usize = 4;
/// Known Source candidates may outnumber this profile's installed workset.
pub const NATIVE_CANDIDATE_CAPACITY: usize = NATIVE_PLOT_CAPACITY + 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePlotProfile {
    pub id: &'static str,
    pub capacity: usize,
    installed: [NativePlot; NATIVE_PLOT_CAPACITY],
}

impl NativePlotProfile {
    pub const fn installed(&self) -> &[NativePlot] {
        &self.installed
    }

    pub fn contains(&self, plot: &ResidentPlot) -> Result<bool, WorksetRefusal> {
        for installed in self.installed {
            if &resident(installed)? == plot {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePlot {
    KeyboardCanvas,
    MemoryLantern,
    Tour,
    Patchbay,
    Hello,
}

impl NativePlot {
    pub const fn name(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => "conduitos-keyboard-upper",
            Self::MemoryLantern => "memory_lantern",
            Self::Tour => "tour",
            Self::Patchbay => "patchbay",
            Self::Hello => "hello",
        }
    }
    pub const fn title(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => "Keyboard canvas",
            Self::MemoryLantern => "Text editor",
            Self::Tour => "Tutorial",
            Self::Patchbay => "Patchbay",
            Self::Hello => "Hello, world.",
        }
    }
    pub const fn source(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => crate::keyboard_text_plan::PLOT_SOURCE,
            Self::MemoryLantern => {
                include_str!("../../../../plots/memory-lantern/keyboard-only.conduit")
            }
            Self::Tour => include_str!("../../../../plots/tour/main.conduit"),
            Self::Patchbay => include_str!("../../../../plots/patchbay/main.conduit"),
            Self::Hello => include_str!("../../../../plots/hello/main.conduit"),
        }
    }
}

pub const fn profile() -> NativePlotProfile {
    NativePlotProfile {
        id: "conduitos/native-installed-plots@1",
        capacity: NATIVE_PLOT_CAPACITY,
        installed: [
            NativePlot::KeyboardCanvas,
            NativePlot::MemoryLantern,
            NativePlot::Tour,
            NativePlot::Patchbay,
        ],
    }
}

pub const fn inventory() -> [NativePlot; NATIVE_PLOT_CAPACITY] {
    profile().installed
}

/// Reviewed Source visibility does not grant native execution. Hello is real
/// checked Source, but is outside this finite installed profile and is refused.
pub const fn candidate_inventory() -> [NativePlot; NATIVE_CANDIDATE_CAPACITY] {
    let installed = inventory();
    [
        installed[0],
        installed[1],
        installed[2],
        installed[3],
        NativePlot::Hello,
    ]
}

pub fn checked(plot: NativePlot) -> Result<ExpandedCanonicalPlot, WorksetRefusal> {
    super::preparation::with_plot(plot, Clone::clone)
}

pub fn resident(plot: NativePlot) -> Result<ResidentPlot, WorksetRefusal> {
    super::preparation::with_plot(plot, |checked| {
        ResidentPlot::new(
            checked.source_document_id.clone(),
            checked.checked_plot_id.clone(),
        )
    })
}

pub fn resolve(identity: &ResidentPlot) -> Result<NativePlot, WorksetRefusal> {
    for plot in profile().installed {
        if &resident(plot)? == identity {
            return Ok(plot);
        }
    }
    Err(WorksetRefusal::UnknownPlot)
}
