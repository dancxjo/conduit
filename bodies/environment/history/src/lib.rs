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
mod native;
pub use graph::{GraphRefusal, GraphSample, HistoryGraph, MAX_GRAPH_SAMPLES};

pub use native::{HistoryLookSceneRefusal, NativeHistoryShow};

mod committed;
pub use committed::{
    CommittedHistoryRefusal, PreparedCommittedHistoryFace, MAXIMUM_COMMITTED_HISTORY_GENERATIONS,
};

mod temperature_look;
pub use temperature_look::HistoryTemperaturePresentation;
