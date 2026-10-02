#![no_std]

extern crate alloc;

#[allow(dead_code, clippy::too_many_arguments)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    AlignedTrainingView, CalibrationTransform, ClockRelation, ClockRelationQuality,
    ClockRelationQualityEstimated, ConcatenatedSignal, CoordinateAxisName, CoordinateFrame,
    DataGenerationDigest, DataGenerationNamespace, DataGenerationNamespaceRefusal,
    DataGenerationRefusal, DataGenerationTextValue, DataLoadTextTerminal, DataLoadTextTerminalForm,
    DataReferenceRefusal, DataSaveTextTerminal, DataSaveTextTerminalForm, DatasetDescriptor,
    DatasetExampleIdentity, DatasetExamplePage, DatasetSplitMembership, FileCopyOutcome,
    FileCopyResult, FullWindowPolicy, FullWindowPolicyForm, MathScalarRefusal,
    MeasurementHysteresisProfile, MeasurementPlotOverflowPolicy, MeasurementPlotPoint,
    MeasurementPlotProfile, MeasurementPlotRefusal, MeasurementPlotSeries, MeasurementRange,
    MeasurementSample, MeasurementSummary, MeasurementSummaryRefusal, MeasurementThresholdDecision,
    MeasurementThresholdPolicy, MeasurementThresholdRefusal, MeasurementThresholdState,
    MeasurementThresholdStateForm, MeasurementThresholdTransition, MeasurementWindowProfile,
    MeasurementWindowRefusal, MissingDataMask, NormalizedQuantityRefusal, ObservationProvenance,
    ObservationProvenanceDerived, ObservationProvenanceMeasured, ObservationSet, ObservationValue,
    ObservationValueSampledSignal, ObservationValueTensor, QuantityMappingRefusal,
    QuantizationPolicy, RangePolicy, SampledSignal, SampledSignalRefusal, ScalarComparison,
    ScientificAlignmentRefusal, ScientificAlignmentRefusalObservation, ScientificCorpusRefusal,
    ScientificCorpusRefusalObservation, ScientificObservation, ScientificObservationIdentity,
    ScientificObservationRefusal, SignalCadence, SignalCadenceIrregular, SignalCadenceRegular,
    SignalContinuity, SignalContinuityClockReset, SignalContinuityDiscontinuous, SignalIdentity,
    SignalStart, SignalStartInstant, SignalStartSampleIndex, SignalSummary, SignalWindow,
    TabularColumnSpec, TabularColumnType, TabularOptionalText, TabularPersonRow,
    TabularPersonRowSlot, TabularPersonRowsFour, TabularQueryCompletion, TabularQueryError,
    TabularQueryOutcomeFour, TabularQueryResultFour, TabularQueryStatus, TabularSchemaFour,
    TensorAxis, TensorAxisRole, TensorAxisRoleOther, TensorBacking, TensorElement, TensorRefusal,
    TensorResourceIdentity, TensorSummary, TensorValue,
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
