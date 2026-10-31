#![no_std]
//! Read-only semantic projection of the existing bounded recording reader.
//! Content validity is not membership, storage authority or a commit receipt.
extern crate alloc;
mod contribution;
pub use contribution::{
    HistoryFaceRefusal, HistoryFaceWindow, PreparedHistoryFace, HISTORY_SUBJECT,
};

mod browser;
mod graph;
mod retained;
pub use retained::RetainedEvidenceGraph;
mod native;
pub use graph::{GraphRefusal, GraphSample, HistoryGraph, MAX_GRAPH_SAMPLES};

pub use native::{NativeHistoryShow, NativeHistoryTextMeasure};

pub mod source_series;
mod source_series_face;

mod source_series_native;
pub use source_series_native::{SourceSeriesStyle, SourceSeriesStyleResolver};
