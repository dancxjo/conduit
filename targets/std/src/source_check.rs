//! Immutable Source analysis specialized during authorized Host preparation.
//! Play only selects a retained exact typed result through the existing native
//! Host Call. There is no compiler, IO, allocation, or hidden worker in read().
use crate::ImmediateNativeSource;
use conduit_core::{CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal};
use conduit_plot::{
    KindSignature, ProfileCatalog, SourceCheckNativeSchema, SourceCheckProjection,
    SourceCheckRefusal, SourceCheckReply, SourceCheckRequest, StartupCatalog,
};

pub const SOURCE_CHECK_KIND: &str = "language/source-check";
pub const SOURCE_CHECK_CALL: &str = "language/source-check@1";
pub const SOURCE_CHECK_AUTHORITY: &str = "language/source-check-authority@1";
pub const SOURCE_CHECK_RESOURCE: &str = "language/prepared-source-check@1";
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
        if inputs.is_empty()
            || inputs.len() > MAX_PREPARED_SOURCE_CHECKS
            || bounds.request_bytes == 0
            || bounds.report_bytes == 0
            || bounds.request_bytes as usize > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || bounds.report_bytes as usize > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("Source-check preparation bounds refused".into());
        }
        let schema = SourceCheckNativeSchema::prepare()
            .map_err(|e| format!("Source-check schema: {e:?}"))?;
        let mut entries: Vec<Entry> = Vec::with_capacity(inputs.len());
        let mut request_schema = None;
        let mut reply_schema = None;
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
            let (entry, input_type, output_type) = Entry::prepare(&request, &report)?;
            if entries.iter().any(|old| old.request == entry.request) {
                return Err("duplicate immutable Source-check request".into());
            }
            if request_schema
                .as_ref()
                .is_some_and(|old| old != &input_type)
                || reply_schema.as_ref().is_some_and(|old| old != &output_type)
            {
                return Err("prepared Source-check schemas disagree".into());
            }
            request_schema = Some(input_type);
            reply_schema = Some(output_type);
            entries.push(entry);
        }
        let request_schema = request_schema.ok_or("Source-check request schema absent")?;
        let reply_schema = reply_schema.ok_or("Source-check reply schema absent")?;
        let refusal = |reason| -> Result<Box<[u8]>, String> {
            checkpoint()?;
            let bytes = schema
                .encode_reply(reason, bounds.report_bytes as usize)
                .map_err(|e| format!("Source-check refusal: {e:?}"))?;
            let value = conduit_core::validate_canonical_structured_value(&bytes)
                .map_err(|e| format!("refusal shape: {e:?}"))?;
            if value.type_bytes() != reply_schema.as_ref() {
                return Err("refusal schema differs".into());
            }
            Ok(value.value_node().into())
        };
        let refusals = [
            refusal(SourceCheckReply::Refused(
                SourceCheckRefusal::InvalidSnapshot,
            ))?,
            refusal(SourceCheckReply::Refused(SourceCheckRefusal::StaleSource))?,
            refusal(SourceCheckReply::Refused(
                SourceCheckRefusal::UnknownGrammar,
            ))?,
            refusal(SourceCheckReply::Refused(
                SourceCheckRefusal::UnknownContext,
            ))?,
            refusal(SourceCheckReply::Refused(SourceCheckRefusal::EntryBytes))?,
            refusal(SourceCheckReply::NotPrepared)?,
        ];
        let base = core::mem::size_of::<PreparedSourceCheckProvider>()
            + entries.len() * core::mem::size_of::<Entry>()
            + request_schema.len()
            + reply_schema.len()
            + refusals.iter().map(|node| node.len()).sum::<usize>();
        let retained_bytes = entries
            .iter()
            .try_fold(base, |sum, entry| {
                sum.checked_add(entry.request.len())?
                    .checked_add(entry.report.len())
            })
            .ok_or("Source-check backing size overflow")?;
        if retained_bytes > bounds.retained_bytes {
            return Err("Source-check retained backing exceeds admission".into());
        }
        let kind = Kind {
            kind_id: SOURCE_CHECK_KIND.into(),
            kind_contract_revision: SOURCE_CHECK_CALL.into(),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            semantic_laws: vec![],
            inputs: vec![PortDescriptor {
                port_id: "request".into(),
                value_kind: schema
                    .request_kind()
                    .map_err(|e| format!("request Type: {e:?}"))?,
                direction: PortDirection::Input,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            outputs: vec![PortDescriptor {
                port_id: "reply".into(),
                value_kind: schema
                    .reply_kind()
                    .map_err(|e| format!("report Type: {e:?}"))?,
                direction: PortDirection::Output,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: bounds.request_bytes.max(bounds.report_bytes),
            },
        };
        kind.validate()
            .map_err(|e| format!("Source-check Kind: {e:?}"))?;
        checkpoint()?;
        Ok(Self {
            kind,
            provider: PreparedSourceCheckProvider {
                entries: entries.into_boxed_slice(),
                request_schema,
                reply_schema,
                refusals,
                maximum_request: bounds.request_bytes as usize,
            },
            retained_bytes,
            bounds,
        })
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
        let mut next = startup.clone();
        let mut kinds = profiles.clone();
        SourceCheckNativeSchema::prepare()
            .map_err(|e| format!("Source-check schema: {e:?}"))?
            .install_types(&mut next)
            .map_err(|e| format!("Source-check Types: {e:?}"))?;
        next.insert(KindSignature {
            kind: SOURCE_CHECK_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
        next.insert_fore(SOURCE_CHECK_KIND, self.kind.checked_front())
            .map_err(|e| e.to_string())?;
        kinds
            .insert_kind(self.kind.clone())
            .map_err(|e| e.to_string())?;
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
