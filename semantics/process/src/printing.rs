//! One print attempt's protocol evidence, never a spooler or scheduler.
use crate::*;
use conduit_core::{
    ResourceDereferenceRequirement, ResourceReferenceAccessRefusal, ResourceReferenceBinding,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

pub const PRINT_DOCUMENT_CONTENT_PROFILE: &str = "document/pdf@1";
pub const PRINT_DOCUMENT_ACCESS_CLASS: &str = "conduit.resource/print-document@1";
pub const PRINT_DOCUMENT_AUTHORITY: &str = "conduit.authority/print-document@1";
pub const PRINT_MAXIMUM_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
pub const PRINT_MAXIMUM_DOCUMENT_ITEMS: u64 = 1000;
pub const PRINT_MAXIMUM_WIRE_BYTES: usize = 8192;

impl PrintSupport {
    /// Existing canonical structured Back property, not another print wire.
    pub fn property(
        &self,
    ) -> Result<conduit_core::StructuredConfigurationValue, NativeBindingRefusal> {
        let profile = Self::semantic_type()?
            .profile()
            .expect("checked print profile")
            .value_kind()
            .clone();
        Ok(
            conduit_core::StructuredConfigurationValue::new(profile, self.clone().encode()?)
                .expect("bounded checked print support"),
        )
    }
    pub fn from_property(
        property: &conduit_core::StructuredConfigurationValue,
    ) -> Result<Self, NativeBindingRefusal> {
        Self::decode(property.canonical_value())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintAdmissionRefusal {
    Document(ResourceReferenceAccessRefusal),
    UnsupportedMedia,
    UnsupportedColor,
    UnsupportedSides,
    UnsupportedCopies,
}

/// Validate one selected Back's facts; never choose, rank or reserve a provider.
pub fn admit_print_request(
    request: &PrintRequest,
    support: &PrintSupport,
    binding: &ResourceReferenceBinding,
) -> Result<(), PrintAdmissionRefusal> {
    let attributes = request.attributes();
    if !support.media().get().contains(attributes.media()) {
        return Err(PrintAdmissionRefusal::UnsupportedMedia);
    }
    if !support.colors().get().contains(attributes.color()) {
        return Err(PrintAdmissionRefusal::UnsupportedColor);
    }
    if !support.sides().get().contains(attributes.sides()) {
        return Err(PrintAdmissionRefusal::UnsupportedSides);
    }
    if attributes.copies() > support.maximum_copies() {
        return Err(PrintAdmissionRefusal::UnsupportedCopies);
    }
    ResourceDereferenceRequirement {
        content_profile: PRINT_DOCUMENT_CONTENT_PROFILE.into(),
        access_class: PRINT_DOCUMENT_ACCESS_CLASS.into(),
        authority_contract: PRINT_DOCUMENT_AUTHORITY.into(),
        maximum_bytes: PRINT_MAXIMUM_DOCUMENT_BYTES,
        maximum_items: Some(PRINT_MAXIMUM_DOCUMENT_ITEMS),
    }
    .admit(request.document().get(), binding)
    .map_err(PrintAdmissionRefusal::Document)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintProviderReport {
    Dispatched,
    Acknowledged(PrintIdentity),
    Completed(PrintIdentity),
    RejectedBeforeDispatch,
    FailedAfterAcknowledgement,
    CancellationRequested,
    AcknowledgementUnknown,
    ProviderLost,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintStage {
    Queued,
    Dispatched,
    Acknowledged,
    Terminal,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintProtocolRefusal {
    InvalidTransition,
    ProviderJobIdentity,
}

/// Holds only one finite operation and provider receipt identity, never bytes.
/// Reports describe evidence supplied by the trusted selected provider owner;
/// methods do not send jobs, acquire authority, retry or acknowledge equipment.
pub struct PrintAttempt {
    operation: PrintOperation,
    stage: PrintStage,
    provider_job: Option<PrintIdentity>,
}
impl PrintAttempt {
    pub fn prepare(
        request: &PrintRequest,
        support: &PrintSupport,
        binding: &ResourceReferenceBinding,
        attempt_id: PrintIdentity,
    ) -> Result<Self, PrintAdmissionRefusal> {
        admit_print_request(request, support, binding)?;
        let operation =
            PrintOperation::new(attempt_id, request.job_id().clone()).expect("checked identities");
        Ok(Self {
            operation,
            stage: PrintStage::Queued,
            provider_job: None,
        })
    }
    pub fn operation(&self) -> &PrintOperation {
        &self.operation
    }
    pub fn stage(&self) -> PrintStage {
        self.stage
    }
    pub fn provider_job(&self) -> Option<&PrintIdentity> {
        self.provider_job.as_ref()
    }
    pub fn queued(&self) -> PrintLifecycle {
        PrintLifecycle::queued(
            self.operation.attempt_id().clone(),
            self.operation.job_id().clone(),
        )
        .expect("checked fields")
    }
    pub fn observe(
        &mut self,
        report: PrintProviderReport,
    ) -> Result<PrintLifecycle, PrintProtocolRefusal> {
        use PrintProviderReport as R;
        use PrintStage as S;
        if self.stage == S::Terminal {
            return Err(PrintProtocolRefusal::InvalidTransition);
        }
        let event = match (self.stage, report) {
            (S::Queued, R::Dispatched) => {
                self.stage = S::Dispatched;
                return Ok(PrintLifecycle::dispatched(
                    self.operation.attempt_id().clone(),
                    self.operation.job_id().clone(),
                )
                .expect("checked fields"));
            }
            (S::Dispatched, R::Acknowledged(id)) => {
                let acknowledgement =
                    PrintLifecycle::acknowledged(self.operation.clone(), id.clone())
                        .expect("checked fields");
                self.provider_job = Some(id);
                self.stage = S::Acknowledged;
                return Ok(acknowledgement);
            }
            (S::Acknowledged, R::Completed(id)) => {
                if self.provider_job.as_ref() != Some(&id) {
                    return Err(PrintProtocolRefusal::ProviderJobIdentity);
                }
                PrintLifecycle::completed(self.operation.clone(), id).expect("checked fields")
            }
            (S::Queued, R::RejectedBeforeDispatch) => {
                PrintLifecycle::failed(self.operation.clone(), PrintFailure::RejectedBeforeDispatch)
                    .expect("checked fields")
            }
            (S::Queued, R::ProviderLost) => PrintLifecycle::failed(
                self.operation.clone(),
                PrintFailure::UnavailableBeforeDispatch,
            )
            .expect("checked fields"),
            (S::Queued, R::CancellationRequested) => PrintLifecycle::cancelled(
                self.operation.attempt_id().clone(),
                self.operation.job_id().clone(),
            )
            .expect("checked fields"),
            (S::Acknowledged, R::FailedAfterAcknowledgement) => PrintLifecycle::failed(
                self.operation.clone(),
                PrintFailure::ProviderReportedFailure,
            )
            .expect("checked fields"),
            (S::Dispatched, R::AcknowledgementUnknown) => PrintLifecycle::uncertain(
                self.operation.clone(),
                self.provider_job.clone(),
                PrintUncertainty::AcknowledgementUnknown,
            )
            .expect("checked fields"),
            (S::Dispatched | S::Acknowledged, R::ProviderLost) => PrintLifecycle::uncertain(
                self.operation.clone(),
                self.provider_job.clone(),
                PrintUncertainty::ProviderLostAfterDispatch,
            )
            .expect("checked fields"),
            (S::Dispatched | S::Acknowledged, R::CancellationRequested) => {
                PrintLifecycle::uncertain(
                    self.operation.clone(),
                    self.provider_job.clone(),
                    PrintUncertainty::CancellationAfterDispatch,
                )
                .expect("checked fields")
            }
            _ => return Err(PrintProtocolRefusal::InvalidTransition),
        };
        self.stage = S::Terminal;
        Ok(event)
    }
}
