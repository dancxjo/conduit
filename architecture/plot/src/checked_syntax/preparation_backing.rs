//! Retention only. Unsupported syntax shapes fail closed instead of estimating.
use super::*;
use crate::preparation_backing::{Backing, sum};
impl CheckedSyntaxDocument {
    /// Conservative reachable heap capacities, excluding inline Self/allocator
    /// overhead. Shared backing is charged fully per alias, not borrowed from
    /// another owner. This observation supplies no compiler factory peak.
    pub fn owned_preparation_backing_bytes(&self) -> Option<usize> {
        if !self.glyph_notations.is_empty() || !self.type_forms.is_empty() {
            return None;
        }
        sum([
            self.native_law_custody.backing(),
            self.physical.owned_preparation_backing_bytes(),
            self.glyph_values.backing(),
            self.source_document_id.backing(),
            self.native_types.backing(),
            self.retained_native_types.backing(),
            Some(
                self.type_forms
                    .capacity()
                    .checked_mul(core::mem::size_of::<CheckedTypeForm>())?,
            ),
            Some(
                self.glyph_notations
                    .capacity()
                    .checked_mul(core::mem::size_of::<crate::CheckedGlyphNotation>())?,
            ),
            self.plots.backing(),
            self.source_sugar_expansions.backing(),
            self.structured_types.backing(),
            self.exact_initial_info.backing(),
        ])
    }
}
impl Backing for CheckedSyntaxDocument {
    fn backing(&self) -> Option<usize> {
        self.owned_preparation_backing_bytes()
    }
}
impl Backing for u8 {
    fn backing(&self) -> Option<usize> {
        Some(0)
    }
}
impl Backing for CanonicalStartupValue {
    fn backing(&self) -> Option<usize> {
        match self {
            Self::Literal(v) | Self::PlotParameter(v) => v.backing(),
            Self::PoolReference(_) => None,
            Self::Structured(v) => v.backing(),
            Self::Quantity(_) | Self::Unit(_) | Self::TemperatureDifference(_) => None,
        }
    }
}
impl Backing for CheckedStartupBinding {
    fn backing(&self) -> Option<usize> {
        sum([
            self.name.backing(),
            self.value_type.backing(),
            self.value.backing(),
        ])
    }
}
impl Backing for CheckedStartupParameter {
    fn backing(&self) -> Option<usize> {
        sum([
            self.name.backing(),
            self.value_type.backing(),
            self.default.backing(),
        ])
    }
}
impl Backing for CheckedNativeType {
    fn backing(&self) -> Option<usize> {
        sum([
            self.name.backing(),
            self.identity.backing(),
            self.value_type.backing(),
            self.value_contracts.backing(),
            self.invariants.backing(),
        ])
    }
}
impl Backing for NativeTypeValueContract {
    fn backing(&self) -> Option<usize> {
        sum([self.representation_path.backing(), self.contract.backing()])
    }
}
impl Backing for CheckedCanonicalPlot {
    fn backing(&self) -> Option<usize> {
        if !self.pools.is_empty() {
            return None;
        }
        sum([
            self.glyph_values.backing(),
            self.checked_plot_id.backing(),
            self.name.backing(),
            self.startup_parameters.backing(),
            self.runtime_ports.backing(),
            self.runtime_front.backing(),
            self.shorthand.backing(),
            self.local_values.backing(),
            Some(
                self.pools
                    .capacity()
                    .checked_mul(core::mem::size_of::<CheckedPoolDeclaration>())?,
            ),
            self.gears.backing(),
            self.cords.backing(),
        ])
    }
}
impl Backing for CheckedCanonicalGear {
    fn backing(&self) -> Option<usize> {
        if self.activation.is_some() || self.retained.is_some() {
            return None;
        }
        sum([
            self.name.backing(),
            self.kind.backing(),
            self.startup_parameters.backing(),
            self.startup_bindings.backing(),
        ])
    }
}
impl Backing for CheckedCanonicalCord {
    fn backing(&self) -> Option<usize> {
        self.stages.backing()
    }
}
impl Backing for CheckedCordStage {
    fn backing(&self) -> Option<usize> {
        match self {
            Self::Reference(v) => v.backing(),
            Self::InlineGear(v) => v.backing(),
            Self::RelationalGear {
                operands,
                gear,
                input_ports,
                output_port,
            } => sum([
                operands.backing(),
                gear.backing(),
                input_ports.backing(),
                output_port.backing(),
            ]),
            Self::TerminalProjection { endpoint, .. } => endpoint.backing(),
            Self::Cancellation { gear, .. } => gear.backing(),
            Self::Literal { value, .. } => value.backing(),
            Self::When { .. } | Self::PureExpression { .. } | Self::StructuredSelector { .. } => {
                None
            }
        }
    }
}
impl Backing for crate::RuntimePort {
    fn backing(&self) -> Option<usize> {
        if !self.refinements.is_empty() {
            return None;
        }
        sum([
            self.name.text.backing(),
            self.value_type.text.backing(),
            Some(
                self.refinements
                    .capacity()
                    .checked_mul(core::mem::size_of::<crate::ValueRefinement>())?,
            ),
        ])
    }
}
impl Backing for SourceSugarExpansion {
    fn backing(&self) -> Option<usize> {
        sum([
            self.plot.backing(),
            self.checked_plot_id.backing(),
            self.authored.backing(),
            self.ordinary_kind.backing(),
            self.input_ports.backing(),
            self.output_ports.backing(),
            self.operand_bindings.backing(),
            self.canonical_replacement.backing(),
        ])
    }
}
impl Backing for SourceSugarOperandBinding {
    fn backing(&self) -> Option<usize> {
        sum([self.source.backing(), self.input_port.backing()])
    }
}

#[cfg(test)]
mod tests;
