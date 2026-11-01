#![no_std]
//! Shared kernel Backs for the environmental Source composition.
extern crate alloc;
#[cfg(feature = "source-recording")]
pub mod source_recording;
#[cfg(feature = "appliance")]
pub mod appliance;
pub mod assigned;
#[cfg(feature = "body-image")]
pub mod body_image;
mod canonical_input;
mod canonical_storage;
#[cfg(feature = "construction")]
pub mod construction;
#[cfg(any(
    feature = "evidence-journal",
    feature = "display-session",
    feature = "mask-renderer"
))]
pub mod display_evidence;
#[cfg(feature = "evidence-journal")]
pub mod display_evidence_compact;
#[cfg(feature = "evidence-journal")]
pub mod display_evidence_journal;
#[cfg(feature = "display-mask")]
pub mod display_mask;
#[cfg(feature = "display-output")]
pub mod display_output;
pub mod display_packets;
#[cfg(feature = "display-session")]
pub mod display_session;
pub mod environmental_reading;
#[cfg(feature = "history-exchange")]
pub mod history_exchange;
#[cfg(feature = "mask-kernel")]
pub mod mask_kernel;
#[cfg(feature = "mask-renderer")]
pub mod mask_renderer;
#[cfg(feature = "member-receipts")]
pub mod member_receipts;
pub mod observation;
#[cfg(feature = "face-projection")]
pub mod observed_record;
pub mod publication;
#[cfg(feature = "face-projection")]
pub mod publication_face_custody;
#[cfg(feature = "face-projection")]
pub mod retained_publication_face;
use alloc::{boxed::Box, string::String};
use conduit_computation::{PreparedExpressionBack, StructuredSelectorBack};
use conduit_core::{PortTemporal, StructuredSelector};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};
use conduit_plot::PortableExpressionProgram;

/// The ordinary production Backs retain their lifecycle and commit ownership.
/// Targets service their exact admitted Host Calls; this is no scheduler.
pub enum CaptureBack {
    Expression(Box<PreparedExpressionBack>),
    Selector(StructuredSelectorBack),
    Pulse(conduit_kernel::scheduler::HostCallBack),
    Observation(Box<observation::ObservationBack>),
}
impl CaptureBack {
    pub fn observation(
        contract: &observation::ObservationContract,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        Ok(Self::Observation(Box::new(
            observation::ObservationBack::prepare(contract)?,
        )))
    }
    pub fn expression(
        program: &PortableExpressionProgram,
        temporal: PortTemporal,
    ) -> Result<Self, String> {
        Ok(Self::Expression(Box::new(
            PreparedExpressionBack::prepare_with_temporal(program, temporal)?,
        )))
    }
    /// # Safety
    /// Complete independently authenticated checked Make origin and exact
    /// Source/configuration binding are required; digests alone are not proof.
    pub unsafe fn expression_from_linked_make(
        material: &'static [u8],
        program_digest: [u8; 32],
        bounds: (u32, u32),
        temporal: PortTemporal,
    ) -> Result<Self, String> {
        // SAFETY: this wrapper preserves the caller's installed-Make contract.
        Ok(Self::Expression(Box::new(unsafe {
            PreparedExpressionBack::from_linked_make(material, program_digest, bounds, temporal)
        }?)))
    }
    pub fn selector(maximum_input_bytes: u32) -> Self {
        Self::Selector(StructuredSelectorBack::new(maximum_input_bytes))
    }
    pub fn pulse(maximum_input_bytes: u32) -> Self {
        Self::Pulse(conduit_kernel::scheduler::HostCallBack::new(
            maximum_input_bytes,
        ))
    }
}
impl<const P: usize> StepBack<P> for CaptureBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Expression(back) => back.step(io, input),
            Self::Selector(back) => back.step(io, input),
            Self::Pulse(back) => back.step(io, input),
            Self::Observation(back) => back.step(io, input),
        }
    }
    fn compact_canonical_output(&self) -> Option<(PortId, conduit_kernel::CompactCanonicalLayout)> {
        match self {
            Self::Observation(back) => {
                <observation::ObservationBack as StepBack<P>>::compact_canonical_output(back)
            }
            _ => None,
        }
    }
    fn prepared_compact_canonical_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Observation(back) => {
                <observation::ObservationBack as StepBack<P>>::prepared_compact_canonical_output(
                    back, port,
                )
            }
            _ => None,
        }
    }
    fn encodes_prepared_output(&self, port: PortId) -> bool {
        match self {
            Self::Observation(back) => {
                <observation::ObservationBack as StepBack<P>>::encodes_prepared_output(back, port)
            }
            _ => false,
        }
    }
    fn encode_prepared_output(
        &self,
        port: PortId,
        output: &mut [u8],
    ) -> Result<usize, conduit_kernel::StorageError> {
        match self {
            Self::Observation(back) => {
                <observation::ObservationBack as StepBack<P>>::encode_prepared_output(
                    back, port, output,
                )
            }
            _ => {
                let bytes = <Self as StepBack<P>>::prepared_output(self, port)
                    .ok_or(conduit_kernel::StorageError::RepresentationMismatch)?;
                output
                    .get_mut(..bytes.len())
                    .ok_or(conduit_kernel::StorageError::ValueTooLarge)?
                    .copy_from_slice(bytes);
                Ok(bytes.len())
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Expression(back) => {
                <PreparedExpressionBack as StepBack<P>>::prepared_output(back, port)
            }
            Self::Observation(back) => {
                <observation::ObservationBack as StepBack<P>>::prepared_output(back, port)
            }
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        if let Self::Expression(back) = self {
            <PreparedExpressionBack as StepBack<P>>::step_committed(back)
        }
        if let Self::Observation(back) = self {
            <observation::ObservationBack as StepBack<P>>::step_committed(back);
        }
        if let Self::Selector(back) = self {
            <StructuredSelectorBack as StepBack<P>>::step_committed(back);
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Observation(back) => <observation::ObservationBack as StepBack<P>>::cancel(back),
            Self::Expression(back) => <PreparedExpressionBack as StepBack<P>>::cancel(back),
            Self::Selector(back) => <StructuredSelectorBack as StepBack<P>>::cancel(back),
            Self::Pulse(back) => {
                <conduit_kernel::scheduler::HostCallBack as StepBack<P>>::cancel(back)
            }
        }
    }
}

/// Prepared pure selection invoked by the selector Back's existing Host Call.
/// The output envelope is reserved before Play; unmatched drops return None.
pub struct PreparedCaptureSelector {
    selector: conduit_core::PreparedCanonicalStructuredSelector,
}
impl PreparedCaptureSelector {
    pub fn prepare(
        selector: StructuredSelector,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        // Reserve the complete checked output Type domain, capped by the
        // independently admitted output envelope. Unknown Type bounds retain
        // the original envelope rather than narrowing selector compatibility.
        let output_bytes = canonical_storage::bytes(selector.output_type(), maximum_output_bytes)
            .unwrap_or(maximum_output_bytes);
        Ok(Self {
            selector: conduit_core::PreparedCanonicalStructuredSelector::prepare(
                &selector,
                maximum_input_bytes,
                output_bytes,
            )?,
        })
    }
    /// Keep exact immutable checked Type encodings in linked storage. The Core
    /// constructor compares them against the ordinarily decoded selector.
    pub fn with_static_schema(
        selector: StructuredSelector,
        input_schema: &'static [u8],
        output_schema: &'static [u8],
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        let output_bytes = canonical_storage::bytes(selector.output_type(), maximum_output_bytes)
            .unwrap_or(maximum_output_bytes);
        Ok(Self {
            selector: conduit_core::PreparedCanonicalStructuredSelector::with_static_schema(
                &selector,
                input_schema,
                output_schema,
                maximum_input_bytes,
                output_bytes,
            )?,
        })
    }
    pub fn select(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_core::StructuredSelectorRefusal> {
        self.selector.select(input)
    }
}

pub mod recording;

pub mod environmental_publication;

#[cfg(feature = "face-projection")]
pub mod environmental_publication_face;

#[cfg(any(feature = "local-planning", feature = "environmental-source"))]
pub mod environmental_source;
