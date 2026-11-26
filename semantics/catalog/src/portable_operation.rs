//! Explicit checked computation preparation, independent of any application's
//! workset or catalog defaults. Host Call servicing and admission stay upstream.
use crate::{PreparedCanonicalSelector, PreparedExpressionBack, StructuredSelectorBack};
use alloc::{boxed::Box, vec, vec::Vec};
use conduit_core as c;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortableOperationRefusal {
    Unsupported,
    Initialization,
}
pub enum PortableOperationBack {
    Expression(Box<PreparedExpressionBack>),
    Selector(StructuredSelectorBack),
}
pub struct PreparedPortableOperation {
    pub kind: c::Kind,
    pub back: PortableOperationBack,
    pub selector: Option<PreparedCanonicalSelector>,
    pub host_calls: Vec<c::HostCallRequirement>,
}
impl PreparedPortableOperation {
    /// Allocate preparation storage for one explicitly supplied checked gear.
    /// The returned contract must still be matched against its checked Fore by
    /// the admitting owner. No offer, authority, placement or Play is created.
    pub fn prepare(
        gear: &conduit_plot::CheckedGear,
        temporal: c::PortTemporal,
        maximum_input_bytes: u32,
        maximum_output_bytes: u32,
    ) -> Result<Self, PortableOperationRefusal> {
        let value = |key| {
            gear.configuration
                .iter()
                .find(|entry| entry.key == key)
                .map(|entry| &entry.value)
        };
        if let Some(c::ConfigurationValue::Text(encoded)) = value("program") {
            let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
                .map_err(init)?;
            return Ok(Self {
                kind: crate::pure_expression_contract(&program, temporal).map_err(init)?,
                back: PortableOperationBack::Expression(Box::new(
                    PreparedExpressionBack::prepare_with_temporal(&program, temporal)
                        .map_err(init)?,
                )),
                selector: None,
                host_calls: vec![],
            });
        }
        if let Some(c::ConfigurationValue::Text(encoded)) = value("selector") {
            let selector = c::StructuredSelector::from_canonical_hex(encoded).map_err(init)?;
            let kind = crate::structured_selector_contract(&selector, temporal);
            let selector = PreparedCanonicalSelector::prepare(
                selector,
                maximum_input_bytes as usize,
                maximum_output_bytes as usize,
            )
            .map_err(init)?;
            let host_calls = vec![c::HostCallRequirement {
                contract_id: "conduit.host/structured-selector@1".into(),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes,
                maximum_output_bytes,
            }];
            return Ok(Self {
                kind,
                back: PortableOperationBack::Selector(StructuredSelectorBack::new(
                    maximum_input_bytes,
                )),
                selector: Some(selector),
                host_calls,
            });
        }
        Err(PortableOperationRefusal::Unsupported)
    }
}
fn init<T>(_: T) -> PortableOperationRefusal {
    PortableOperationRefusal::Initialization
}
