#![no_std]

extern crate alloc;

#[allow(dead_code, clippy::too_many_arguments)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    ClockRelation, ClockRelationQuality, ClockRelationQualityEstimated, DataGenerationDigest,
    DataGenerationNamespace, DataGenerationNamespaceRefusal, DataGenerationTextValue,
    DataLoadTextTerminal, DataLoadTextTerminalCode, DataReferenceRefusal, DataSaveTextTerminal,
    DataSaveTextTerminalCode, FullWindowPolicy, FullWindowPolicyCode, MathScalarRefusal,
    MeasurementPlotOverflowPolicy, MeasurementPlotPoint, MeasurementPlotRefusal,
    MeasurementSummaryRefusal, MeasurementThresholdPolicy, MeasurementThresholdRefusal,
    MeasurementThresholdState, MeasurementThresholdStateCode, MeasurementThresholdTransition,
    MeasurementWindowRefusal, NormalizedQuantityRefusal, QuantityMappingRefusal,
    QuantizationPolicy, RangePolicy, SampledSignalRefusal, ScalarComparison,
    ScientificObservationRefusal, SignalContinuity, SignalContinuityClockReset,
    SignalContinuityDiscontinuous, TabularColumnSpec,
    TabularColumnType, TabularOptionalText, TabularPersonRow, TabularPersonRowSlot,
    TabularPersonRowsFour, TabularQueryCompletion, TabularQueryError, TabularQueryResultFour,
    TabularQueryStatus, TabularSchemaFour, TensorAxisRole, TensorAxisRoleOther, TensorElement,
    TensorRefusal,
};

mod data_catalog;
mod data_generation;
mod data_reference;
mod data_terminal;
#[cfg(feature = "kernel-step")]
mod flow_collect_back;
mod measurement_observation_catalog;
mod measurement_plot;
mod measurement_plot_back;
mod measurement_plot_catalog;
mod measurement_profile_wire;
mod measurement_summary;
mod measurement_summary_catalog;
mod measurement_threshold;
mod measurement_threshold_catalog;
mod measurement_threshold_wire;
mod measurement_window;
mod measurement_window_catalog;
mod measurement_wire;
mod sampled_signal;
mod scalar_comparison;
mod scientific_alignment;
mod scientific_corpus;
mod scientific_digest;
mod scientific_observation;
mod tabular;
mod tabular_catalog;
mod tabular_reference;
mod tensor;
mod tensor_catalog;
mod tensor_codec;

pub use data_catalog::*;
pub use data_generation::*;
pub use data_reference::*;
pub use data_terminal::*;
#[cfg(feature = "kernel-step")]
pub use flow_collect_back::{FlowCollectBack, FlowCollectPreparationError};
pub use measurement_observation_catalog::*;
pub use measurement_plot::*;
pub use measurement_plot_back::*;
pub use measurement_plot_catalog::*;
pub use measurement_profile_wire::*;
pub use measurement_summary::*;
pub use measurement_summary_catalog::*;
pub use measurement_threshold::*;
pub use measurement_threshold_catalog::*;
pub use measurement_threshold_wire::*;
pub use measurement_window::*;
pub use measurement_window_catalog::*;
pub use measurement_wire::*;
pub use sampled_signal::*;
pub use scientific_alignment::*;
pub use scientific_corpus::*;
pub use scientific_observation::*;
pub use tabular::*;
pub use tabular_catalog::*;
pub use tabular_reference::*;
pub use tensor::*;
pub use tensor_catalog::*;
