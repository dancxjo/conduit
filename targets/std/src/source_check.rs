//! Immutable Source analysis specialized during authorized Host preparation.
//! Play only selects a retained exact typed result through the existing native
//! Host Call. There is no compiler, IO, allocation, or hidden worker in read().
use crate::ImmediateNativeSource;
use conduit_core::{CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal};
use conduit_plot::{
    KindSignature, ProfileCatalog, SourceCheckNativeSchema, SourceCheckProjection,
    SourceCheckRefusal, SourceCheckReply, SourceCheckRequest, StartupCatalog,
};

pub use conduit_std_offers::{
    SOURCE_CHECK_AUTHORITY, SOURCE_CHECK_CALL, SOURCE_CHECK_KIND, SOURCE_CHECK_RESOURCE,
};
pub const MAX_PREPARED_SOURCE_CHECKS: usize = 4;

pub struct SourceCheckPreparation<'a> {
    /// Independently authorized immutable Source/context, never reconstructed
    /// from an incoming snapshot name or copied report.
    pub owner: &'a mut SourceCheckProjection,
    pub request: &'a SourceCheckRequest,
}
#[derive(Clone, Copy)]
pub struct SourceCheckPreparationBounds {
    pub request_bytes: u32,
    pub report_bytes: u32,
    /// Retained Rust payload and inline provider bytes only; excludes allocator
    /// bookkeeping and the caller's compiler,
    /// context, catalogue and preparation workspace, which need separate custody.
    pub retained_bytes: usize,
}
mod assembly;
#[cfg(target_os = "linux")]
pub mod process;
mod requests;
use requests::Entry;
/// One copy of each schema plus finite canonical value nodes. No compiler AST,
/// cache or per-refusal duplicate schema is retained by the runtime provider.
pub struct PreparedSourceCheckProvider {
    entries: Box<[Entry]>,
    request_schema: Box<[u8]>,
    reply_schema: Box<[u8]>,
    refusals: [Box<[u8]>; 6],
    maximum_request: usize,
}
pub struct PreparedSourceChecker {
    kind: Kind,
    provider: PreparedSourceCheckProvider,
    retained_bytes: usize,
    bounds: SourceCheckPreparationBounds,
}
impl PreparedSourceChecker {
    /// Trusted preparation must already possess the Source grants and admit the
    /// compiler's complete workspace/work envelope. This pure specialization
    /// creates no Host, Base, grant, Ready offer, Plan or Play. The ordinary
    /// installed native Source entrance independently checks those facts.
    pub fn prepare(
        inputs: &mut [SourceCheckPreparation<'_>],
        bounds: SourceCheckPreparationBounds,
    ) -> Result<Self, String> {
        Self::prepare_inner(inputs, bounds, None)
    }
    /// Cooperative preparation cancellation, independent of Play cancellation.
    /// Checks bracket allocating compiler/schema stages; they do not interrupt
    /// one Rust compiler call or establish memory/CPU containment.
    pub fn prepare_controlled(
        inputs: &mut [SourceCheckPreparation<'_>],
        bounds: SourceCheckPreparationBounds,
        control: &crate::RunControl,
    ) -> Result<Self, String> {
        Self::prepare_inner(inputs, bounds, Some(control))
    }
    fn prepare_inner(
        inputs: &mut [SourceCheckPreparation<'_>],
        bounds: SourceCheckPreparationBounds,
        control: Option<&crate::RunControl>,
    ) -> Result<Self, String> {
        let checkpoint = || {
            if control.is_some_and(crate::RunControl::stop_requested) {
                Err("Source-check preparation cancelled".to_string())
            } else {
                Ok(())
            }
        };
        checkpoint()?;
        Self::validate_bounds(inputs.len(), bounds)?;
        let schema = SourceCheckNativeSchema::prepare()
            .map_err(|e| format!("Source-check schema: {e:?}"))?;
        let mut builder = requests::Builder::new(inputs.len());
        for input in inputs {
            checkpoint()?;
            let request = schema
                .encode_request(input.request, bounds.request_bytes as usize)
                .map_err(|e| format!("Source-check request: {e:?}"))?;
            checkpoint()?;
            let report = input
                .owner
                .project(input.request)
                .map_err(|e| format!("Source-check preparation: {e:?}"))?;
            checkpoint()?;
            let report = schema
                .encode_reply(
                    SourceCheckReply::Checked(&report),
                    bounds.report_bytes as usize,
                )
                .map_err(|e| format!("Source-check report: {e:?}"))?;
            builder.push(&request, &report)?;
        }
        Self::assemble(&schema, builder, bounds, checkpoint)
    }
    fn validate_bounds(count: usize, bounds: SourceCheckPreparationBounds) -> Result<(), String> {
        if count == 0
            || count > MAX_PREPARED_SOURCE_CHECKS
            || bounds.request_bytes == 0
            || bounds.report_bytes == 0
            || bounds.request_bytes as usize > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || bounds.report_bytes as usize > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("Source-check preparation bounds refused".into());
        }
        Ok(())
    }
    pub fn kind(&self) -> &Kind {
        &self.kind
    }
    pub fn bounds(&self) -> SourceCheckPreparationBounds {
        self.bounds
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Authoring eligibility only. Hosts without the independently installed
    /// provider still cannot place or execute the operation.
    pub fn install_authoring(
        &self,
        startup: &mut StartupCatalog,
        profiles: &mut ProfileCatalog,
    ) -> Result<(), String> {
        let schema = SourceCheckNativeSchema::prepare()
            .map_err(|e| format!("Source-check schema: {e:?}"))?;
        Self::install_kind_authoring(startup, profiles, &schema, self.kind.clone())
    }
    /// Register checked semantic declarations without preparing a compiler,
    /// Source owner, provider, resource offer or authority grant. These finite
    /// byte ceilings describe authoring eligibility; actual Host admission is
    /// independent and may be smaller.
    pub fn install_authoring_declaration(
        startup: &mut StartupCatalog,
        profiles: &mut ProfileCatalog,
        request_bytes: u32,
        report_bytes: u32,
    ) -> Result<(), String> {
        let schema = SourceCheckNativeSchema::prepare()
            .map_err(|e| format!("Source-check schema: {e:?}"))?;
        let kind = Self::kind_for_bounds(
            &schema,
            SourceCheckPreparationBounds {
                request_bytes,
                report_bytes,
                retained_bytes: 0,
            },
        )?;
        Self::install_kind_authoring(startup, profiles, &schema, kind)
    }
    fn install_kind_authoring(
        startup: &mut StartupCatalog,
        profiles: &mut ProfileCatalog,
        schema: &SourceCheckNativeSchema,
        kind: Kind,
    ) -> Result<(), String> {
        let mut next = startup.clone();
        let mut kinds = profiles.clone();
        schema
            .install_types(&mut next)
            .map_err(|e| format!("Source-check Types: {e:?}"))?;
        next.insert(KindSignature {
            kind: SOURCE_CHECK_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
        next.insert_fore(SOURCE_CHECK_KIND, kind.checked_front())
            .map_err(|e| e.to_string())?;
        kinds.insert_kind(kind).map_err(|e| e.to_string())?;
        *startup = next;
        *profiles = kinds;
        Ok(())
    }
    pub fn into_provider(self) -> PreparedSourceCheckProvider {
        self.provider
    }
}
// SAFETY: all compiler work and Native admission finished in prepare(). This
// provider owns only bounded immutable packets. Reads compare/copy those bytes,
// never allocate (even on refusal), access IO, invoke callbacks, or retain loans.
unsafe impl ImmediateNativeSource for PreparedSourceCheckProvider {
    fn read(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, String> {
        let node = requests::select(self, input);
        let length = self
            .reply_schema
            .len()
            .checked_add(node.len())
            .ok_or_else(String::new)?;
        if output.len() < length {
            return Err(String::new());
        }
        output[..self.reply_schema.len()].copy_from_slice(&self.reply_schema);
        output[self.reply_schema.len()..length].copy_from_slice(node);
        Ok(length)
    }
}
