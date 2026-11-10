//! Common immutable runtime construction for local and bounded-process analysis.
use super::*;
impl PreparedSourceChecker {
    /// Semantic/byte-profile construction only, before initialized availability.
    /// This creates no Source, provider, grant or resource reservation.
    pub fn kind_for_bounds(
        schema: &SourceCheckNativeSchema,
        bounds: SourceCheckPreparationBounds,
    ) -> Result<Kind, String> {
        Self::validate_bounds(1, bounds)?;
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
        Ok(kind)
    }
    pub(super) fn assemble(
        schema: &SourceCheckNativeSchema,
        builder: requests::Builder,
        bounds: SourceCheckPreparationBounds,
        checkpoint: impl Fn() -> Result<(), String>,
    ) -> Result<Self, String> {
        let requests::Builder {
            entries,
            request_schema,
            reply_schema,
        } = builder;
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
        let kind = Self::kind_for_bounds(schema, bounds)?;
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
}
