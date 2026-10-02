//! Exact bounded set of Plots currently intended by one body.
//!
//! An entry is only the existing source/check identity pair. It deliberately
//! does not introduce a `ProgramId` or another semantic object around Plot.

use alloc::vec::Vec;
use conduit_core::{CheckedPlotId, SourceDocumentId};
use serde::{Deserialize, Serialize};

pub const MAX_BODY_PLOTS: usize = 16;
pub const MAX_BODY_PLOT_IDENTITY_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResidentPlot {
    pub source_document_id: SourceDocumentId,
    pub checked_plot_id: CheckedPlotId,
}

impl ResidentPlot {
    pub fn new(source_document_id: SourceDocumentId, checked_plot_id: CheckedPlotId) -> Self {
        Self {
            source_document_id,
            checked_plot_id,
        }
    }

    fn identity_bytes(&self) -> Option<usize> {
        self.source_document_id
            .as_str()
            .len()
            .checked_add(self.checked_plot_id.as_str().len())
    }

    fn valid(&self) -> bool {
        !self.source_document_id.as_str().is_empty()
            && !self.checked_plot_id.as_str().is_empty()
            && self.identity_bytes().is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyWorkset {
    plots: Vec<ResidentPlot>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyWorksetError {
    InvalidPlotIdentity,
    DuplicatePlot,
    PlotAbsent,
    PlotCapacityExhausted,
    IdentityBytesExhausted,
}

impl BodyWorkset {
    pub fn one(plot: ResidentPlot) -> Result<Self, BodyWorksetError> {
        let mut workset = Self::default();
        workset.add(plot)?;
        Ok(workset)
    }

    pub fn from_plots(
        plots: impl IntoIterator<Item = ResidentPlot>,
    ) -> Result<Self, BodyWorksetError> {
        let mut workset = Self::default();
        for plot in plots {
            workset.add(plot)?;
        }
        Ok(workset)
    }

    pub fn plots(&self) -> &[ResidentPlot] {
        &self.plots
    }

    pub fn len(&self) -> usize {
        self.plots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plots.is_empty()
    }

    pub fn identity_bytes(&self) -> usize {
        self.plots
            .iter()
            .map(|plot| {
                plot.identity_bytes()
                    .expect("validated Plot identity bytes")
            })
            .sum()
    }

    pub fn contains(&self, plot: &ResidentPlot) -> bool {
        self.plots.binary_search(plot).is_ok()
    }

    pub fn add(&mut self, plot: ResidentPlot) -> Result<(), BodyWorksetError> {
        self.validate()?;
        if !plot.valid() {
            return Err(BodyWorksetError::InvalidPlotIdentity);
        }
        let position = match self.plots.binary_search(&plot) {
            Ok(_) => return Err(BodyWorksetError::DuplicatePlot),
            Err(position) => position,
        };
        if self.plots.len() >= MAX_BODY_PLOTS {
            return Err(BodyWorksetError::PlotCapacityExhausted);
        }
        let next_bytes = self
            .identity_bytes()
            .checked_add(
                plot.identity_bytes()
                    .ok_or(BodyWorksetError::IdentityBytesExhausted)?,
            )
            .ok_or(BodyWorksetError::IdentityBytesExhausted)?;
        if next_bytes > MAX_BODY_PLOT_IDENTITY_BYTES {
            return Err(BodyWorksetError::IdentityBytesExhausted);
        }
        self.plots.insert(position, plot);
        Ok(())
    }

    pub fn remove(&mut self, plot: &ResidentPlot) -> Result<(), BodyWorksetError> {
        self.validate()?;
        let position = self
            .plots
            .binary_search(plot)
            .map_err(|_| BodyWorksetError::PlotAbsent)?;
        self.plots.remove(position);
        Ok(())
    }

    pub fn validate(&self) -> Result<(), BodyWorksetError> {
        if self.plots.len() > MAX_BODY_PLOTS {
            return Err(BodyWorksetError::PlotCapacityExhausted);
        }
        let mut bytes = 0usize;
        for (index, plot) in self.plots.iter().enumerate() {
            if !plot.valid() {
                return Err(BodyWorksetError::InvalidPlotIdentity);
            }
            if index > 0 && self.plots[index - 1] >= *plot {
                return Err(BodyWorksetError::DuplicatePlot);
            }
            bytes = bytes
                .checked_add(
                    plot.identity_bytes()
                        .ok_or(BodyWorksetError::IdentityBytesExhausted)?,
                )
                .ok_or(BodyWorksetError::IdentityBytesExhausted)?;
        }
        if bytes > MAX_BODY_PLOT_IDENTITY_BYTES {
            return Err(BodyWorksetError::IdentityBytesExhausted);
        }
        Ok(())
    }
}
