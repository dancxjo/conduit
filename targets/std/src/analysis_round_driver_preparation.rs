//! Explicit hosted preparation helper for actual init/commit/rebase Source execution.
//! This allocating helper is not the cooperative round poll or a Current authority.
use super::*;
use conduit_plot::rust_binding::{
    PreparedNativeAdmission, PreparedNativeBounds, PreparedNativeProgress,
};
#[derive(Clone, Copy)]
pub enum AuxiliarySource {
    Initialize,
    InitialEnvelope,
    CommitFrontier,
    CommitBeam,
    Rebase,
    RebasedEnvelope,
}
impl AuxiliarySource {
    fn entry(self) -> &'static str {
        match self {
            Self::Initialize => "language-parser-initialize",
            Self::InitialEnvelope => "language/analysis-initial-envelope",
            Self::CommitFrontier => "language-parser-commit-frontier",
            Self::CommitBeam => "language-parser-beam-commit",
            Self::Rebase => "language-parser-revision-rebase",
            Self::RebasedEnvelope => "language/analysis-rebased-envelope",
        }
    }
}
pub fn required_auxiliary_source_offers(
    document: &CheckedSyntaxDocument,
    source: AuxiliarySource,
) -> Result<Vec<CapabilityOffer>, DriverRefusal> {
    plan::offers(document, &[source.entry()])
}
pub struct PreparedAuxiliarySource<'d> {
    operation: Operation,
    input_laws: PreparedNativeAdmission<'d>,
    closed: bool,
}
impl<'d> PreparedAuxiliarySource<'d> {
    pub fn prepare(
        document: &'d CheckedSyntaxDocument,
        host: &HostAdvertisement,
        source: AuxiliarySource,
    ) -> Result<Self, DriverRefusal> {
        for offer in required_auxiliary_source_offers(document, source)? {
            if !host.capabilities.contains(&offer) {
                return Err(DriverRefusal::PreparationDetail {
                    stage: "missing-auxiliary-offer",
                    detail: format!(
                        "required={:?} matching_id={}",
                        offer.capability_id,
                        host.capabilities
                            .iter()
                            .any(|actual| actual.capability_id == offer.capability_id)
                    ),
                });
            }
        }
        let definition = plan::definition(document, host, source.entry())?;
        let kind = &definition.boundary.input_fronts[0].external_port.value_kind;
        // A payload profile identity is not the authored Native schema identity.
        // Resolve the actual Source input declaration, then verify its Fore profile.
        let input = document
            .plots
            .iter()
            .find(|plot| plot.name == source.entry())
            .and_then(|plot| {
                plot.runtime_ports.iter().find(|port| {
                    port.direction == conduit_plot::syntax::RuntimePortDirection::Input
                })
            })
            .ok_or(DriverRefusal::Preparation)?;
        let native = document
            .native_types
            .iter()
            .find(|native| native.name == input.value_type.text)
            .ok_or(DriverRefusal::Preparation)?;
        let profile =
            native
                .value_type
                .profile()
                .map_err(|error| DriverRefusal::PreparationDetail {
                    stage: "native-input-profile",
                    detail: format!("{error:?}"),
                })?;
        if profile.value_kind() != kind {
            return Err(DriverRefusal::PreparationDetail {
                stage: "native-input-profile",
                detail: "Source input differs from actual Fore".into(),
            });
        }
        let input_laws = PreparedNativeAdmission::new(
            document,
            &native.identity,
            PreparedNativeBounds {
                canonical_bytes: TRANSPORT,
                scopes: 4096,
                invariants: 4096,
            },
        )
        .map_err(|error| DriverRefusal::PreparationDetail {
            stage: "native-input-admission",
            detail: format!("{error:?}"),
        })?;
        let operation =
            Operation::prepare(source.entry(), definition, DriverBounds::default().signs)?;
        Ok(Self {
            operation,
            input_laws,
            closed: false,
        })
    }
    /// Bounded caller-selected preparation work allowance; timeout never retries.
    /// The returned bytes are raw actual Source output, not an admitted target Native.
    pub fn execute_for_preparation(
        &mut self,
        input: &[u8],
        maximum_polls: usize,
    ) -> Result<Vec<u8>, DriverRefusal> {
        if self.closed || maximum_polls == 0 {
            return Err(DriverRefusal::Closed);
        }
        self.closed = true;
        self.input_laws
            .begin(input)
            .map_err(|error| DriverRefusal::PreparationDetail {
                stage: "native-input-admission",
                detail: format!("{error:?}"),
            })?;
        loop {
            if self
                .input_laws
                .step()
                .map_err(|_| DriverRefusal::Preparation)?
                == PreparedNativeProgress::Admitted
            {
                break;
            }
        }
        self.operation.submit(input)?;
        for _ in 0..maximum_polls {
            if let Some(output) = self.operation.poll()? {
                if output.len() > TRANSPORT {
                    return Err(DriverRefusal::SourceTransport);
                }
                let output = output.to_vec(); // explicit hosted preparation allocation
                self.operation.acknowledge()?;
                return Ok(output);
            }
        }
        Err(DriverRefusal::Pressure)
    }
    pub fn input_admission(&self) -> &PreparedNativeAdmission<'d> {
        &self.input_laws
    }
    pub fn retained_actual_output(&self) -> &[u8] {
        self.operation.retained_output()
    }
    pub fn definition(&self) -> &KernelCompositeDefinition {
        self.operation.kernel.definition()
    }
    pub fn signs(
        &self,
    ) -> std::collections::BTreeMap<conduit_core::HostId, Vec<conduit_kernel::KernelEvent>> {
        self.operation.kernel.signs()
    }
    pub fn last_kernel_error(&self) -> Option<&KernelCompositeError> {
        self.operation.error.as_ref()
    }
}
