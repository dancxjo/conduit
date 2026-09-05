//! Admitted browser media lifecycle. Browser APIs remain outside this module.

use conduit_core::{AuthorityGrantId, BoundedResourceRef, HostOperationId, KindId, PlanId};
use conduit_human::{
    plan_media_acquisition, select_acquired_media, AcquiredMediaResource,
    ImageObservationReference, ImageObservationRefusal, MediaAcquisitionAuthority,
    MediaAcquisitionOffer, MediaAcquisitionPlan, MediaAcquisitionRequest, MediaAcquisitionResult,
    MediaPlanningRefusal, MediaUseRequirement, SelectedMediaResource,
};

mod abi;
mod offers;

pub use offers::{
    acquired_camera_source_offer, browser_camera_frame_sink_offer,
    browser_media_acquisition_offers, BROWSER_MEDIA_ARTIFACT, BROWSER_MEDIA_PROFILE,
};

pub const MAXIMUM_BROWSER_MEDIA_VALUE_BYTES: usize = 64 * 1024;
pub const MAXIMUM_BROWSER_MEDIA_RESULT_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserMediaPhase {
    OfferAvailable,
    AcquisitionPlanned(MediaAcquisitionPlan),
    AcquisitionPlaying(MediaAcquisitionPlan),
    ResourceTruth(AcquiredMediaResource),
    UsePlanned {
        plan_id: PlanId,
        resource: AcquiredMediaResource,
        selection: SelectedMediaResource,
    },
    UsePlaying {
        plan_id: PlanId,
        resource: AcquiredMediaResource,
        selection: SelectedMediaResource,
    },
    Terminal(BrowserMediaTerminal),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BrowserMediaTerminal {
    AcquisitionDenied,
    AcquisitionDismissed,
    AcquisitionCancelled,
    NoMatchingDevice,
    UnsupportedConstraints,
    CapacityExhausted,
    MalformedCompletion,
    AcquisitionClosed,
    MediaCancelled,
    DeviceLost,
    TrackEnded,
    MediaClosed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BrowserMediaRefusal {
    WrongPhase,
    Planning(MediaPlanningRefusal),
    CompletionOperationMismatch,
    ResultTooLarge,
    MalformedCompletion,
    ValueTooLarge,
    Pressure,
    ImageObservation(ImageObservationRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserMediaSession {
    phase: BrowserMediaPhase,
    expected_operation: Option<HostOperationId>,
    retained_bytes: usize,
    observed_values: u32,
}

impl BrowserMediaSession {
    pub const fn new() -> Self {
        Self {
            phase: BrowserMediaPhase::OfferAvailable,
            expected_operation: None,
            retained_bytes: 0,
            observed_values: 0,
        }
    }

    pub fn phase(&self) -> &BrowserMediaPhase {
        &self.phase
    }

    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub const fn observed_values(&self) -> u32 {
        self.observed_values
    }

    pub fn seal_acquisition(
        &mut self,
        plan_id: PlanId,
        offer: &MediaAcquisitionOffer,
        authority: Option<&MediaAcquisitionAuthority>,
        request: MediaAcquisitionRequest,
    ) -> Result<(), BrowserMediaRefusal> {
        if !matches!(self.phase, BrowserMediaPhase::OfferAvailable) {
            return Err(BrowserMediaRefusal::WrongPhase);
        }
        let operation = request.operation_id.clone();
        let plan = plan_media_acquisition(plan_id, offer, authority, request, 0)
            .map_err(BrowserMediaRefusal::Planning)?;
        self.expected_operation = Some(operation);
        self.phase = BrowserMediaPhase::AcquisitionPlanned(plan);
        Ok(())
    }

    pub fn start_acquisition(&mut self) -> Result<(), BrowserMediaRefusal> {
        let BrowserMediaPhase::AcquisitionPlanned(plan) = &self.phase else {
            return Err(BrowserMediaRefusal::WrongPhase);
        };
        self.phase = BrowserMediaPhase::AcquisitionPlaying(plan.clone());
        Ok(())
    }

    pub fn complete_acquisition(
        &mut self,
        operation: &HostOperationId,
        encoded_result_bytes: usize,
        result: MediaAcquisitionResult,
    ) -> Result<(), BrowserMediaRefusal> {
        if !matches!(self.phase, BrowserMediaPhase::AcquisitionPlaying(_)) {
            return Err(BrowserMediaRefusal::WrongPhase);
        }
        if self.expected_operation.as_ref() != Some(operation) {
            return Err(BrowserMediaRefusal::CompletionOperationMismatch);
        }
        if encoded_result_bytes == 0 || encoded_result_bytes > MAXIMUM_BROWSER_MEDIA_RESULT_BYTES {
            self.phase = BrowserMediaPhase::Terminal(BrowserMediaTerminal::MalformedCompletion);
            return Err(BrowserMediaRefusal::ResultTooLarge);
        }
        self.expected_operation = None;
        self.phase = match result {
            MediaAcquisitionResult::Acquired(resource) => {
                BrowserMediaPhase::ResourceTruth(resource)
            }
            MediaAcquisitionResult::Denied => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::AcquisitionDenied)
            }
            MediaAcquisitionResult::Dismissed => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::AcquisitionDismissed)
            }
            MediaAcquisitionResult::Cancelled => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::AcquisitionCancelled)
            }
            MediaAcquisitionResult::NoMatchingDevice => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::NoMatchingDevice)
            }
            MediaAcquisitionResult::UnsupportedConstraints => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::UnsupportedConstraints)
            }
            MediaAcquisitionResult::CapacityExhausted => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::CapacityExhausted)
            }
            MediaAcquisitionResult::Closed => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::AcquisitionClosed)
            }
        };
        Ok(())
    }

    pub fn seal_use(
        &mut self,
        plan_id: PlanId,
        requirement: &MediaUseRequirement,
        authority: Option<&AuthorityGrantId>,
    ) -> Result<(), BrowserMediaRefusal> {
        let BrowserMediaPhase::ResourceTruth(resource) = &self.phase else {
            return Err(BrowserMediaRefusal::WrongPhase);
        };
        let selection = select_acquired_media(requirement, resource, authority)
            .map_err(BrowserMediaRefusal::Planning)?;
        self.phase = BrowserMediaPhase::UsePlanned {
            plan_id,
            resource: resource.clone(),
            selection,
        };
        Ok(())
    }

    pub fn start_use(&mut self) -> Result<(), BrowserMediaRefusal> {
        let BrowserMediaPhase::UsePlanned {
            plan_id,
            resource,
            selection,
        } = &self.phase
        else {
            return Err(BrowserMediaRefusal::WrongPhase);
        };
        self.phase = BrowserMediaPhase::UsePlaying {
            plan_id: plan_id.clone(),
            resource: resource.clone(),
            selection: selection.clone(),
        };
        Ok(())
    }

    pub fn admit_value(&mut self, bytes: usize) -> Result<(), BrowserMediaRefusal> {
        let BrowserMediaPhase::UsePlaying { selection, .. } = &self.phase else {
            return Err(BrowserMediaRefusal::WrongPhase);
        };
        if bytes == 0
            || bytes > MAXIMUM_BROWSER_MEDIA_VALUE_BYTES
            || bytes > selection.flow_bounds.maximum_value_bytes as usize
        {
            return Err(BrowserMediaRefusal::ValueTooLarge);
        }
        if self.retained_bytes != 0 || bytes > selection.flow_bounds.maximum_queue_bytes as usize {
            return Err(BrowserMediaRefusal::Pressure);
        }
        self.retained_bytes = bytes;
        self.observed_values += 1;
        Ok(())
    }

    /// Admit one already-materialized image value from the selected camera
    /// flow. The browser adapter owns byte capture and resource realization;
    /// this boundary validates only exact portable observation truth.
    pub fn admit_image_observation(
        &mut self,
        content: BoundedResourceRef,
        width: u16,
        height: u16,
        expected_profile: &KindId,
    ) -> Result<ImageObservationReference, BrowserMediaRefusal> {
        let bytes = usize::try_from(content.extent.bytes)
            .map_err(|_| BrowserMediaRefusal::ValueTooLarge)?;
        let observation = ImageObservationReference::new(content, width, height, expected_profile)
            .map_err(BrowserMediaRefusal::ImageObservation)?;
        self.admit_value(bytes)?;
        Ok(observation)
    }

    pub fn release_value(&mut self) -> Result<(), BrowserMediaRefusal> {
        if self.retained_bytes == 0 {
            return Err(BrowserMediaRefusal::WrongPhase);
        }
        self.retained_bytes = 0;
        Ok(())
    }

    pub fn device_lost(&mut self) -> Result<(), BrowserMediaRefusal> {
        if !matches!(self.phase, BrowserMediaPhase::UsePlaying { .. }) {
            return Err(BrowserMediaRefusal::WrongPhase);
        }
        self.retained_bytes = 0;
        self.phase = BrowserMediaPhase::Terminal(BrowserMediaTerminal::DeviceLost);
        Ok(())
    }

    pub fn track_ended(&mut self) -> Result<(), BrowserMediaRefusal> {
        if !matches!(self.phase, BrowserMediaPhase::UsePlaying { .. }) {
            return Err(BrowserMediaRefusal::WrongPhase);
        }
        self.retained_bytes = 0;
        self.phase = BrowserMediaPhase::Terminal(BrowserMediaTerminal::TrackEnded);
        Ok(())
    }

    pub fn close_media(&mut self) -> Result<(), BrowserMediaRefusal> {
        if !matches!(self.phase, BrowserMediaPhase::UsePlaying { .. }) {
            return Err(BrowserMediaRefusal::WrongPhase);
        }
        self.retained_bytes = 0;
        self.phase = BrowserMediaPhase::Terminal(BrowserMediaTerminal::MediaClosed);
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), BrowserMediaRefusal> {
        self.retained_bytes = 0;
        self.expected_operation = None;
        self.phase = match self.phase {
            BrowserMediaPhase::AcquisitionPlanned(_) | BrowserMediaPhase::AcquisitionPlaying(_) => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::AcquisitionCancelled)
            }
            BrowserMediaPhase::UsePlanned { .. } | BrowserMediaPhase::UsePlaying { .. } => {
                BrowserMediaPhase::Terminal(BrowserMediaTerminal::MediaCancelled)
            }
            _ => return Err(BrowserMediaRefusal::WrongPhase),
        };
        Ok(())
    }
}

impl Default for BrowserMediaSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
