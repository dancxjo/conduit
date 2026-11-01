//! An actual retained History owner under the existing installed Std issuer.
//! This CPU-only entrance does not establish Body admission, recording or NAT.
use conduit_core::{AuthorityGrant, PreparedStructuredValueValidator};
use conduit_environment_runtime::history_exchange::{
    codec::PreparedSignedBatchEncoder, contract::HISTORY_MAXIMUM_BYTES, owner::decode_request,
    HistoryProvider, HistoryReadContract, ReadyHistoryBase, MAXIMUM_SIGNED_BATCH_BYTES,
};
use conduit_std_host::{ImmediateNativeSource, NativeSourceDefinition};

/// Actual affine native owner and definition, prepared for Std registration.
pub type NativeHistoryParts = (NativeSourceDefinition, Box<dyn ImmediateNativeSource>);
/// Preparation refusal preserves the complete original ready owner.
pub type HistoryPreparationRefusal<P> = (ReadyHistoryBase<P>, String);

pub struct PreparedHistorySource<P> {
    base: ReadyHistoryBase<P>,
    validator: PreparedStructuredValueValidator,
    encoder: PreparedSignedBatchEncoder,
    raw: [u8; MAXIMUM_SIGNED_BATCH_BYTES],
    failure: Option<String>,
}
impl<P: HistoryProvider + Send + 'static> PreparedHistorySource<P> {
    /// Prepare an actual ready owner and independently retained current grant.
    /// Refusal returns the complete original ready owner, never a replacement.
    ///
    /// # Safety
    /// P must serve an already owned finite in-memory signed archive with bounded
    /// CPU-only, allocation-free reads: no file/network IO, sleep or unwind.
    /// The returned affine provider may only be transferred into Std installed
    /// execution, never invoked directly. The caller must separately establish
    /// owner/configuration/grant correspondence
    /// at Std installation. Membership/observer receipts are not grants.
    #[allow(clippy::result_large_err)]
    pub unsafe fn prepare(
        base: ReadyHistoryBase<P>,
        grant: AuthorityGrant,
    ) -> Result<NativeHistoryParts, HistoryPreparationRefusal<P>> {
        let prepare = || {
            let contract =
                HistoryReadContract::prepare().map_err(|e| format!("history contract: {e:?}"))?;
            let validator = contract
                .request_validator()
                .map_err(|e| format!("history request: {e:?}"))?;
            let encoder = PreparedSignedBatchEncoder::new(&contract)
                .map_err(|e| format!("history result: {e:?}"))?;
            Ok::<_, String>((contract, validator, encoder))
        };
        let (contract, validator, encoder) = match prepare() {
            Ok(v) => v,
            Err(e) => return Err((base, e)),
        };
        let (identity, attachment, entry) = base.native_registration(&contract);
        if identity.host_id != grant.host_id || identity.boot_id != grant.boot_id {
            return Err((
                base,
                "History native owner differs from independently granted Host/Boot".into(),
            ));
        }
        let definition = NativeSourceDefinition {
            kind: contract.kind().clone(),
            base: entry,
            grant,
            resource_generation: identity.resource_generation_id,
            envelope_id: identity.envelope_id,
            configuration: vec![],
            input_bytes: HISTORY_MAXIMUM_BYTES,
            output_bytes: HISTORY_MAXIMUM_BYTES,
            resource_bytes: attachment.resource_bytes,
        };
        Ok((
            definition,
            Box::new(Self {
                base,
                validator,
                encoder,
                raw: [0; MAXIMUM_SIGNED_BATCH_BYTES],
                failure: Some(
                    "History request/provider/result refused; original evidence retained".into(),
                ),
            }),
        ))
    }
}
// SAFETY: prepare's explicit native CPU contract owns the actual ReadyHistoryBase.
// Std validates independent current registry/grant/artifact/configuration and
// holds its sole BoundBaseOperation lease before dispatching this read.
unsafe impl<P: HistoryProvider + Send + 'static> ImmediateNativeSource
    for PreparedHistorySource<P>
{
    fn read(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, String> {
        if self.validator.validate(input).is_err() {
            return Err(self.failure.take().unwrap_or_default());
        }
        let cursor = match decode_request(input) {
            Ok(cursor) => cursor,
            Err(_) => return Err(self.failure.take().unwrap_or_default()),
        };
        self.raw.fill(0);
        // SAFETY: prepared native correspondence permits only transfer to actual
        // installed Std dispatch, which owns the sole operation lease here.
        let count = match unsafe {
            self.base
                .read_after_native_authorization(cursor, &mut self.raw)
        } {
            Ok(count) if count <= self.raw.len() => count,
            _ => return Err(self.failure.take().unwrap_or_default()),
        };
        let canonical = match self.encoder.encode(&self.raw[..count]) {
            Ok(value) => value,
            Err(_) => return Err(self.failure.take().unwrap_or_default()),
        };
        if canonical.len() > output.len() {
            return Err(self.failure.take().unwrap_or_default());
        }
        let target = &mut output[..canonical.len()];
        target.copy_from_slice(canonical);
        Ok(canonical.len())
    }
}

/// Affine refusal from preparation or actual Std Host admission. Both variants
/// retain the original native owner; neither reports a Playing History Source.
#[allow(clippy::large_enum_variant)]
pub enum HistoryInstallationRefusal<P> {
    Preparation(ReadyHistoryBase<P>, String),
    Host(
        NativeSourceDefinition,
        Box<dyn ImmediateNativeSource>,
        String,
    ),
}
/// Install an actual ready CPU archive through Std's existing registry, finite
/// ledger and sole selected-operation issuer. This does not plan or start Play.
///
/// # Safety
/// The same native correspondence, independent current grant and bounded CPU/no
/// IO contract documented by `PreparedHistorySource::prepare` must hold. The
/// grant must come from actual installed Host authorization, never a recorder
/// receipt or a peer-provided Plan. Root retains returned owners on refusal.
#[allow(clippy::result_large_err)]
pub unsafe fn install_cpu_history_source<P: HistoryProvider + Send + 'static>(
    host: &mut conduit_std_host::StdHost,
    base: ReadyHistoryBase<P>,
    grant: AuthorityGrant,
) -> Result<(), HistoryInstallationRefusal<P>> {
    // SAFETY: Caller supplies the actual CPU archive and independent authority.
    let (definition, provider) = unsafe { PreparedHistorySource::prepare(base, grant) }
        .map_err(|(owner, error)| HistoryInstallationRefusal::Preparation(owner, error))?;
    // SAFETY: Corresponding owner and independently retained definition above;
    // actual Std registration/selection checks remain the sole admission path.
    unsafe { host.install_immediate_native_source(definition, provider) }.map_err(
        |(definition, owner, error)| HistoryInstallationRefusal::Host(definition, owner, error),
    )
}
