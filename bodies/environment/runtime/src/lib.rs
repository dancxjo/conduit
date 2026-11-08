#![no_std]
//! Shared kernel Backs for the environmental Source composition.
extern crate alloc;
#[cfg(feature = "appliance")]
pub mod appliance;
mod canonical_input;
mod canonical_storage;
#[cfg(feature = "construction")]
pub mod construction;
#[cfg(feature = "display-mask")]
pub mod display_mask;
pub mod display_packets;
#[cfg(feature = "member-receipts")]
pub mod member_receipts;
pub mod observation;
pub mod publication;
#[cfg(feature = "face-projection")]
pub mod publication_face_custody;
#[cfg(feature = "face-projection")]
pub mod retained_publication_face;
use alloc::{borrow::Cow, boxed::Box, string::String};
use conduit_core::{PortTemporal, StructuredSelector};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};
use conduit_plot::PortableExpressionProgram;
use conduit_semantic_catalog::{PreparedExpressionBack, StructuredSelectorBack};

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
    selector: CaptureSelection,
    maximum_input_bytes: usize,
    input_type: Cow<'static, [u8]>,
    output_type: Cow<'static, [u8]>,
    output: alloc::vec::Vec<u8>,
}
enum CaptureSelection {
    Owned(StructuredSelector),
    Linked(conduit_core::PreparedBorrowedStructuredSelector),
}
impl PreparedCaptureSelector {
    pub fn prepare(
        selector: StructuredSelector,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        let input_type = selector.input_type().canonical_bytes()?;
        if maximum_input_bytes == 0
            || maximum_input_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || input_type.len() >= maximum_input_bytes
        {
            return Err(conduit_core::StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            maximum_input_bytes,
            input_type: Cow::Owned(input_type),
            output_type: Cow::Owned(selector.output_type().canonical_bytes()?),
            selector: CaptureSelection::Owned(selector),
            output: alloc::vec::Vec::with_capacity(maximum_output_bytes),
        })
    }
    /// Check the complete linked selector once, retaining its operation and
    /// immutable schema spans rather than duplicate decoded Type trees.
    pub fn prepare_static(
        canonical: &'static [u8],
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, conduit_core::StructuredSelectorRefusal> {
        let selector = conduit_core::PreparedBorrowedStructuredSelector::new(
            canonical,
            maximum_input_bytes,
            maximum_output_bytes,
        )?;
        Ok(Self {
            maximum_input_bytes,
            input_type: Cow::Borrowed(selector.input_type_bytes()),
            output_type: Cow::Borrowed(selector.output_type_bytes()),
            selector: CaptureSelection::Linked(selector),
            output: alloc::vec::Vec::with_capacity(maximum_output_bytes),
        })
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }
    pub fn select(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_core::StructuredSelectorRefusal> {
        use conduit_core::{
            StructuredCanonicalSelection as S, StructuredSelectorRefusal as R,
            UnmatchedVariantDisposition as D,
        };
        if input.len() > self.maximum_input_bytes {
            return Err(R::MalformedCheckedValue);
        }
        let selected = match &self.selector {
            CaptureSelection::Linked(selector) => selector
                .select_canonical_into(input, &mut self.output)
                .map_err(|error| match error {
                    R::WrongInputType => R::MalformedCheckedValue,
                    other => other,
                })?,
            CaptureSelection::Owned(selector) => {
                let node = input
                    .strip_prefix(self.input_type.as_ref())
                    .ok_or(R::MalformedCheckedValue)?;
                selector
                    .input_type()
                    .validate_canonical_value_node(node)
                    .map_err(|_| R::MalformedCheckedValue)?;
                selector.select_canonical_into(
                    input,
                    &self.input_type,
                    &self.output_type,
                    &mut self.output,
                )?
            }
        };
        match selected {
            S::Matched => Ok(Some(&self.output)),
            S::Unmatched(D::Drop) => Ok(None),
            S::Unmatched(D::Refuse) => Err(R::UnmatchedVariant),
        }
    }
}

pub mod recording;

#[cfg(test)]
mod linked_selector_tests {
    use super::*;
    use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType};

    #[test]
    fn linked_selector_borrows_both_exact_type_prefixes() {
        let leaf = StructuredInfoType::leaf(kind_id("value/boolean")).unwrap();
        let input = StructuredInfoType::record(
            kind_id("test/linked-selector"),
            alloc::vec![StructuredFieldType::new("chosen", leaf).unwrap()],
        )
        .unwrap();
        let selector = StructuredSelector::field(input, "chosen").unwrap();
        let canonical = Box::leak(selector.canonical_bytes().unwrap().into_boxed_slice());
        let linked = PreparedCaptureSelector::prepare_static(canonical, 512, 512).unwrap();
        assert!(matches!(linked.selector, CaptureSelection::Linked(_)));
        assert!(matches!(linked.input_type, Cow::Borrowed(_)));
        assert!(matches!(linked.output_type, Cow::Borrowed(_)));
        assert_eq!(
            linked.input_type.as_ref(),
            selector.input_type().canonical_bytes().unwrap()
        );
        assert_eq!(
            linked.output_type.as_ref(),
            selector.output_type().canonical_bytes().unwrap()
        );
    }
}
