//! Checked meaning of the exact plots offered at native birth.
use super::WorksetRefusal;
use conduit_body::ResidentPlot;
use conduit_plot::{ExpandedCanonicalPlot, ProfileCatalog, StartupCatalog};

/// Finite native product profile. Capacity is a reviewed deployment choice,
/// not a claim that a body conceptually consists of these particular Plots.
pub const NATIVE_PLOT_CAPACITY: usize = 4;

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
}

impl NativePlot {
    pub const fn name(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => "conduitos-keyboard-upper",
            Self::MemoryLantern => "memory_lantern",
            Self::Tour => "tour",
            Self::Patchbay => "patchbay",
        }
    }
    pub const fn title(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => "Keyboard canvas",
            Self::MemoryLantern => "Memory Lantern",
            Self::Tour => "Tour",
            Self::Patchbay => "Patchbay",
        }
    }
    pub const fn source(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => crate::keyboard_text_plan::PLOT_SOURCE,
            Self::MemoryLantern => include_str!("../../../../plots/memory-lantern/main.conduit"),
            Self::Tour => include_str!("../../../../plots/tour/main.conduit"),
            Self::Patchbay => include_str!("../../../../plots/patchbay/main.conduit"),
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

pub fn checked(plot: NativePlot) -> Result<ExpandedCanonicalPlot, WorksetRefusal> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    let syntax = conduit_plot::parse_syntax_document(plot.source());
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_plot::expand_canonical_plot(&checked, plot.name(), &profile)
        .map_err(|_| WorksetRefusal::Catalog)
}

pub fn resident(plot: NativePlot) -> Result<ResidentPlot, WorksetRefusal> {
    let checked = checked(plot)?;
    Ok(ResidentPlot::new(
        checked.source_document_id,
        checked.checked_plot_id,
    ))
}

pub fn resolve(identity: &ResidentPlot) -> Result<NativePlot, WorksetRefusal> {
    for plot in profile().installed {
        if &resident(plot)? == identity {
            return Ok(plot);
        }
    }
    Err(WorksetRefusal::UnknownPlot)
}
